//! Optional volatile exports for the read-only tftpd-hpa sidecar.
//! HTTP authentication is never bypassed. TFTP uses explicit source-IP admission.
use crate::{
    AppState,
    routes::{provisioning, provisioning_admin},
};
use axum::http::{HeaderMap, HeaderValue, header};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use talkops_core::{
    phones::{self, MediaKind},
    tenant::TenantId,
};
use talkops_provisioning::vendors;
use uuid::Uuid;

const MARKER: &str = "TalkOps Cisco TFTP export v1\n";
const OWNED: &[&str] = &[".owner", "active", "next", "previous"];
/// Wideband ringtone limits: 16 kHz, at most 10 s (160800 samples).
const RWB_MIN_SAMPLES: usize = 240;
const RWB_MAX_SAMPLES: usize = 160_800;
/// Ringtone bytes exported per phone; the sidecar volume is a small tmpfs.
const RING_EXPORT_LIMIT: usize = 8 * 1024 * 1024;

/// Exported files per client IP. Ringtones share one buffer across phones
/// and are written once, then hard-linked.
type Exports = BTreeMap<String, BTreeMap<String, Arc<Vec<u8>>>>;

/// Export root plus a lock: the timer, saves and resyncs all swap the same
/// `next`/`active` directories, so refreshes must not overlap.
#[derive(Debug, Clone)]
pub struct Exporter {
    root: Arc<PathBuf>,
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl Exporter {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root: Arc::new(root),
            lock: Arc::default(),
        }
    }
}

#[derive(Debug, Default)]
pub struct ExportResult {
    pub phones: usize,
    pub files: usize,
    pub omitted: usize,
}

async fn permissions(path: &Path, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).await?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

/// Reject directory links before any recursive operation. This root is owned
/// exclusively by this exporter, identified by a fixed marker and entry set.
async fn prepare(root: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(root.is_absolute(), "Cisco TFTP root must be absolute");
    for ancestor in root.ancestors() {
        match tokio::fs::symlink_metadata(ancestor).await {
            Ok(meta) => anyhow::ensure!(
                !meta.file_type().is_symlink() && meta.is_dir(),
                "Cisco TFTP root cannot contain directory links"
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    tokio::fs::create_dir_all(root).await?;
    let marker = root.join(".owner");
    let mut entries = tokio::fs::read_dir(root).await?;
    let mut nonempty = false;
    while let Some(entry) = entries.next_entry().await? {
        nonempty = true;
        let name = entry.file_name();
        anyhow::ensure!(
            OWNED.iter().any(|allowed| name == *allowed),
            "Cisco TFTP root contains unmanaged files"
        );
        anyhow::ensure!(
            !entry.file_type().await?.is_symlink(),
            "Cisco TFTP root contains links"
        );
    }
    if nonempty {
        anyhow::ensure!(
            tokio::fs::read_to_string(&marker).await? == MARKER,
            "Cisco TFTP root ownership marker is invalid"
        );
    } else {
        tokio::fs::write(&marker, MARKER).await?;
        permissions(&marker, 0o600).await?;
    }
    permissions(root, 0o700).await?;
    Ok(())
}

async fn remove_owned(root: &Path, name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        ["active", "next", "previous"].contains(&name),
        "invalid export directory"
    );
    let target = root.join(name);
    match tokio::fs::symlink_metadata(&target).await {
        Ok(meta) => {
            anyhow::ensure!(
                meta.is_dir() && !meta.file_type().is_symlink(),
                "export directory is not a real directory"
            );
            tokio::fs::remove_dir_all(target).await?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

async fn install(root: &Path, files: &Exports) -> anyhow::Result<()> {
    prepare(root).await?;
    remove_owned(root, "next").await?;
    tokio::fs::create_dir(root.join("next")).await?;
    permissions(&root.join("next"), 0o700).await?;
    // Keyed by buffer address: the same ringtone buffer is linked, not copied.
    let mut written: HashMap<usize, PathBuf> = HashMap::new();
    for (ip, artifacts) in files {
        anyhow::ensure!(vendors::cisco_tftp_ip(ip).is_some(), "invalid export IP");
        let directory = root.join("next").join(ip);
        tokio::fs::create_dir(&directory).await?;
        permissions(&directory, 0o700).await?;
        for (name, body) in artifacts {
            anyhow::ensure!(
                vendors::valid_path(name) && !name.contains('/'),
                "invalid export filename"
            );
            let path = directory.join(name);
            let key = Arc::as_ptr(body) as usize;
            let linked = match written.get(&key) {
                Some(first) => tokio::fs::hard_link(first, &path).await.is_ok(),
                None => false,
            };
            if !linked {
                tokio::fs::write(&path, body.as_slice()).await?;
                permissions(&path, 0o600).await?;
                written.insert(key, path);
            }
        }
    }
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs()
        + 90;
    let lease = root.join("next/.lease");
    tokio::fs::write(&lease, expiry.to_string()).await?;
    permissions(&lease, 0o600).await?;
    remove_owned(root, "previous").await?;
    match tokio::fs::rename(root.join("active"), root.join("previous")).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    tokio::fs::rename(root.join("next"), root.join("active")).await?;
    remove_owned(root, "previous").await?;
    Ok(())
}

/// Revoke old exports if database access or generation fails. Never retain an
/// unbounded stale plaintext snapshot after a failed refresh.
pub async fn refresh(state: &AppState, root: &Path) -> anyhow::Result<ExportResult> {
    let result = tokio::time::timeout(Duration::from_secs(20), collect(state))
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("Cisco TFTP generation timed out")));
    match result {
        Ok((files, stats)) => {
            if let Err(error) = install(root, &files).await {
                // Remove the served generation even if staging failed.
                prepare(root).await?;
                remove_owned(root, "active").await?;
                return Err(error);
            }
            Ok(stats)
        }
        Err(error) => {
            install(root, &BTreeMap::new()).await?;
            Err(error)
        }
    }
}

/// The `userLocale` name in a rendered SEP file, such as `German_Germany`.
fn user_locale(sep: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(sep).ok()?;
    let block = &text[text.find("<userLocale>")?..];
    let start = block.find("<name>")? + "<name>".len();
    let name = &block[start..start + block[start..].find("</name>")?];
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphabetic() || c == '_'))
        .then(|| name.to_owned())
}

/// The files of an uploaded Cisco locale, loaded once per refresh. The phone
/// requests them from the TFTP root, for example `/sp-sip.jar`.
async fn locale_files(
    state: &AppState,
    name: &str,
    cache: &mut HashMap<String, BTreeMap<String, Arc<Vec<u8>>>>,
) -> BTreeMap<String, Arc<Vec<u8>>> {
    if !cache.contains_key(name) {
        let files = match crate::cisco_locale::files(&state.provisioning_dir, name).await {
            Ok(files) => files
                .into_iter()
                .filter(|(file, _)| vendors::valid_path(file) && !file.contains('/'))
                .map(|(file, body)| (file, Arc::new(body)))
                .collect(),
            Err(_) => {
                tracing::warn!(locale = name, "Cisco locale files are missing");
                BTreeMap::new()
            }
        };
        cache.insert(name.to_owned(), files);
    }
    cache[name].clone()
}

/// Converts a TalkOps ringtone (WAV, 8 kHz, mono, 16 bit) to a Cisco wideband
/// `.rwb` file: raw signed 16-bit little-endian PCM, mono, 16 kHz, no header.
/// Linear interpolation doubles the rate. Longer tones are cut to 10 s.
pub fn wav_to_rwb(wav: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut reader = hound::WavReader::new(std::io::Cursor::new(wav))?;
    let spec = reader.spec();
    anyhow::ensure!(
        spec.sample_rate == 8000
            && spec.channels == 1
            && spec.bits_per_sample == 16
            && spec.sample_format == hound::SampleFormat::Int,
        "ringtone must be 8 kHz mono 16-bit PCM"
    );
    let input = reader
        .samples::<i16>()
        .take(RWB_MAX_SAMPLES / 2 + 1)
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = Vec::with_capacity(input.len() * 2);
    for (i, sample) in input.iter().enumerate() {
        output.push(*sample);
        let next = input.get(i + 1).copied().unwrap_or(*sample);
        output.push(((i32::from(*sample) + i32::from(next)) / 2) as i16);
    }
    output.truncate(RWB_MAX_SAMPLES);
    anyhow::ensure!(output.len() >= RWB_MIN_SAMPLES, "ringtone is too short");
    Ok(output.iter().flat_map(|s| s.to_le_bytes()).collect())
}

/// Ring list and ringtone files for a phone. The phone's selected ringtone is
/// first, because the device configuration cannot select a custom tone; the
/// user selects it on the phone. Without a selected ringtone nothing is added.
async fn ringtones(
    state: &AppState,
    phone: &phones::Phone,
    media: &[phones::PhoneMedia],
    cache: &mut HashMap<Uuid, Option<Arc<Vec<u8>>>>,
) -> BTreeMap<String, Arc<Vec<u8>>> {
    let mut files = BTreeMap::new();
    let Some(selected) = phone.ringtone_id else {
        return files;
    };
    let ordered = media
        .iter()
        .filter(|m| m.id == selected)
        .chain(media.iter().filter(|m| m.id != selected));
    let mut rings = Vec::new();
    let mut bytes = 0;
    for m in ordered {
        if rings.len() >= vendors::CISCO_RING_LIMIT {
            break;
        }
        let name = vendors::cisco_ring_filename(&m.id.simple().to_string());
        if files.contains_key(&name) {
            continue;
        }
        let rwb = match cache.get(&m.id) {
            Some(rwb) => rwb.clone(),
            None => {
                let path = provisioning::media_path(state, m.id, &m.filename);
                let rwb = match tokio::fs::read(&path).await.map(|wav| wav_to_rwb(&wav)) {
                    Ok(Ok(rwb)) => Some(Arc::new(rwb)),
                    _ => None,
                };
                cache.insert(m.id, rwb.clone());
                rwb
            }
        };
        let Some(rwb) = rwb else {
            continue;
        };
        if bytes + rwb.len() > RING_EXPORT_LIMIT {
            break;
        }
        bytes += rwb.len();
        rings.push((m.name.clone(), name.clone()));
        files.insert(name, rwb);
    }
    if !rings.is_empty() {
        files.insert(
            "Ringlist-wb.xml".into(),
            Arc::new(vendors::cisco_ringlist(&rings).into_bytes()),
        );
    }
    files
}

async fn collect(state: &AppState) -> anyhow::Result<(Exports, ExportResult)> {
    let tenant = TenantId::DEFAULT;
    let phones = phones::list(&state.db, tenant).await?;
    let options = provisioning_admin::options(state, tenant)
        .await
        .map_err(|_| anyhow::anyhow!("cannot load provisioning options"))?;
    let mut candidates: BTreeMap<String, Vec<_>> = BTreeMap::new();
    let mut stats = ExportResult::default();
    for phone in phones {
        if !vendors::cisco_enterprise(&phone.model) {
            continue;
        }
        let ip = phone
            .provisioning
            .get("vendor_settings")
            .and_then(|s| s.get("cisco_enterprise_tftp_ip"))
            .and_then(|s| s.as_str())
            .unwrap_or("");
        if ip.is_empty() {
            continue;
        }
        let Some(ip) = vendors::cisco_tftp_ip(ip) else {
            stats.omitted += 1;
            continue;
        };
        candidates.entry(ip.to_string()).or_default().push(phone);
    }
    anyhow::ensure!(candidates.len() <= 100, "too many Cisco TFTP devices");
    let media: Vec<_> = phones::list_media(&state.db, tenant)
        .await?
        .into_iter()
        .filter(|m| m.kind == MediaKind::Ringtone)
        .collect();
    let mut cache = HashMap::new();
    let mut locales = HashMap::new();
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("tftp.invalid"));
    let mut output = BTreeMap::new();
    for (ip, phones) in candidates {
        if phones.len() != 1 {
            stats.omitted += phones.len();
            continue;
        }
        let phone = &phones[0];
        if talkops_provisioning::normalize_mac(&phone.mac).as_deref() != Some(phone.mac.as_str()) {
            stats.omitted += 1;
            continue;
        }
        let sip_host = phone
            .provisioning
            .get("sip_host")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        if sip_host.is_empty() && options.sip_host.is_empty() {
            stats.omitted += 1;
            continue;
        }
        let profile = vendors::profile(&phone.model)
            .ok_or_else(|| anyhow::anyhow!("missing Enterprise profile"))?;
        let mut artifacts = BTreeMap::new();
        let mut valid = true;
        for artifact in &profile.artifacts {
            let name = vendors::delivery_filename(profile, artifact, &phone.mac);
            if name != format!("SEP{}.cnf.xml", phone.mac.to_ascii_uppercase())
                && name != "dialplan.xml"
            {
                valid = false;
                break;
            }
            match provisioning::render_phone_artifact(
                state,
                tenant,
                phone,
                &headers,
                &artifact.name,
            )
            .await
            {
                Ok(body) => {
                    artifacts.insert(name, Arc::new(body.into_bytes()));
                }
                // Do not log render errors or template context: these can contain credentials.
                Err(_) => {
                    valid = false;
                    break;
                }
            }
        }
        if !valid || artifacts.len() != 2 {
            stats.omitted += 1;
            continue;
        }
        artifacts.extend(ringtones(state, phone, &media, &mut cache).await);
        let sep = format!("SEP{}.cnf.xml", phone.mac.to_ascii_uppercase());
        if let Some(name) = artifacts.get(&sep).and_then(|b| user_locale(b)) {
            artifacts.extend(locale_files(state, &name, &mut locales).await);
        }
        stats.phones += 1;
        stats.files += artifacts.len();
        output.insert(ip, artifacts);
    }
    Ok((output, stats))
}

/// Refreshes the export now, if enabled. A resync awaits this so the phone
/// never fetches a file that is older than the last save.
pub async fn refresh_now(state: &AppState) -> bool {
    let Some(exporter) = &state.cisco_tftp else {
        return false;
    };
    let _guard = exporter.lock.lock().await;
    match refresh(state, &exporter.root).await {
        Ok(stats) => {
            if stats.omitted > 0 {
                tracing::warn!(
                    omitted = stats.omitted,
                    "Cisco TFTP devices omitted: check unique client IP, SIP host and profile settings"
                );
            }
            true
        }
        Err(_) => {
            tracing::error!("Cisco TFTP refresh failed; check export root and sidecar");
            false
        }
    }
}

/// Starts a refresh in the background after a change that affects exports.
pub fn schedule(state: &AppState) {
    if state.cisco_tftp.is_some() {
        let state = state.clone();
        tokio::spawn(async move { refresh_now(&state).await });
    }
}

pub async fn spawn(state: AppState) -> anyhow::Result<()> {
    let exporter = state
        .cisco_tftp
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Cisco TFTP export is not configured"))?;
    let root = exporter.root.as_path();
    // A restart removes the served generation before any database operation.
    prepare(root).await?;
    remove_owned(root, "active").await?;
    let stats = {
        let _guard = exporter.lock.lock().await;
        refresh(&state, root).await?
    };
    tracing::info!(
        phones = stats.phones,
        files = stats.files,
        omitted = stats.omitted,
        "optional Cisco Enterprise TFTP export enabled; trusted LAN access required"
    );
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(30));
        timer.tick().await;
        loop {
            timer.tick().await;
            refresh_now(&state).await;
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(samples: &[i16]) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut out = std::io::Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(&mut out, spec).unwrap();
        for s in samples {
            writer.write_sample(*s).unwrap();
        }
        writer.finalize().unwrap();
        out.into_inner()
    }

    #[test]
    fn converts_wav_to_wideband_raw() {
        let samples: Vec<i16> = (0..200).map(|i| (i * 10) as i16).collect();
        let rwb = wav_to_rwb(&wav(&samples)).unwrap();
        assert_eq!(rwb.len(), 400 * 2, "rate doubles, no header");
        let pcm: Vec<i16> = rwb
            .chunks(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        assert_eq!(&pcm[..4], &[0, 5, 10, 15]);
        // 20 s at 8 kHz is cut to 10 s at 16 kHz.
        let long = vec![1000i16; 160_000];
        assert_eq!(wav_to_rwb(&wav(&long)).unwrap().len(), RWB_MAX_SAMPLES * 2);
        assert!(
            wav_to_rwb(&wav(&[1; 100])).is_err(),
            "shorter than 240 samples"
        );
        assert!(wav_to_rwb(b"not a wav").is_err());
    }

    #[tokio::test]
    async fn install_links_shared_files_per_address() {
        let dir = std::env::temp_dir().join(format!("talkops-tftp-test-{}", Uuid::new_v4()));
        let ring = Arc::new(vec![1u8, 2, 3, 4]);
        let mut files = Exports::new();
        for ip in ["10.0.0.5", "10.0.0.6"] {
            let mut artifacts = BTreeMap::new();
            artifacts.insert("dialplan.xml".into(), Arc::new(ip.as_bytes().to_vec()));
            artifacts.insert("talkops-0123abcd.rwb".into(), ring.clone());
            files.insert(ip.into(), artifacts);
        }
        install(&dir, &files).await.unwrap();
        for ip in ["10.0.0.5", "10.0.0.6"] {
            let base = dir.join("active").join(ip);
            assert_eq!(
                tokio::fs::read(base.join("talkops-0123abcd.rwb"))
                    .await
                    .unwrap(),
                [1, 2, 3, 4]
            );
            assert_eq!(
                tokio::fs::read(base.join("dialplan.xml")).await.unwrap(),
                ip.as_bytes()
            );
        }
        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }
}
