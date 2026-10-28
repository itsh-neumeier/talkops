//! Optional volatile exports for the read-only tftpd-hpa sidecar.
//! HTTP authentication is never bypassed. TFTP uses explicit source-IP admission.
use crate::{
    AppState,
    routes::{provisioning, provisioning_admin},
};
use axum::http::{HeaderMap, HeaderValue, header};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};
use talkops_core::{phones, tenant::TenantId};
use talkops_provisioning::vendors;

const MARKER: &str = "TalkOps Cisco TFTP export v1\n";
const OWNED: &[&str] = &[".owner", "active", "next", "previous"];

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

async fn install(
    root: &Path,
    files: &BTreeMap<String, BTreeMap<String, String>>,
) -> anyhow::Result<()> {
    prepare(root).await?;
    remove_owned(root, "next").await?;
    tokio::fs::create_dir(root.join("next")).await?;
    permissions(&root.join("next"), 0o700).await?;
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
            tokio::fs::write(&path, body).await?;
            permissions(&path, 0o600).await?;
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

async fn collect(
    state: &AppState,
) -> anyhow::Result<(BTreeMap<String, BTreeMap<String, String>>, ExportResult)> {
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
                    artifacts.insert(name, body);
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
        stats.phones += 1;
        stats.files += artifacts.len();
        output.insert(ip, artifacts);
    }
    Ok((output, stats))
}

pub async fn spawn(state: AppState, root: PathBuf) -> anyhow::Result<()> {
    // A restart removes the served generation before any database operation.
    prepare(&root).await?;
    remove_owned(&root, "active").await?;
    let stats = refresh(&state, &root).await?;
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
            match refresh(&state, &root).await {
                Ok(stats) if stats.omitted > 0 => tracing::warn!(
                    omitted = stats.omitted,
                    "Cisco TFTP devices omitted: check unique client IP, SIP host and profile settings"
                ),
                Ok(_) => {}
                Err(_) => {
                    tracing::error!("Cisco TFTP refresh failed; check export root and sidecar")
                }
            }
        }
    });
    Ok(())
}
