//! Validated, bounded archive staging and deployment configuration recovery.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path};

use age::secrecy::{ExposeSecret, SecretString};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use talkops_core::crypto::SecretBox;

use crate::backup::{BackupConfig, BackupError, Manifest, Result};

pub fn password(value: Option<&str>, file: Option<&Path>) -> Result<Option<SecretString>> {
    let value = if let Some(file) = file.filter(|p| !p.as_os_str().is_empty()) {
        let mut text = String::new();
        File::open(file)?.take(4097).read_to_string(&mut text)?;
        if text.len() > 4096 {
            return Err(BackupError::Invalid(
                "backup password file exceeds 4 KiB".into(),
            ));
        }
        text.trim_end_matches(['\r', '\n']).to_owned()
    } else {
        value.unwrap_or("").to_owned()
    };
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() < 12 {
        return Err(BackupError::Invalid(
            "use a separate backup password of at least 12 characters".into(),
        ));
    }
    Ok(Some(value.into()))
}

pub const CONFIG_LIMIT: u64 = 1 << 20;
const MAX_ENTRIES: usize = 100_000;
pub const IMPORT_FILE: &str = ".deployment.enc";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct FileInfo {
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct DeploymentInfo {
    pub source: String,
    pub captured_at: DateTime<Utc>,
    pub complete: bool,
    pub missing: Vec<String>,
    pub files: Vec<String>,
    pub project_name: String,
    pub profiles: String,
    pub postgres_version: String,
}

/// Explicit source files, including secrets. Never log or serialize into audit details.
#[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct DeploymentInput {
    pub files: BTreeMap<String, String>,
    #[serde(default = "default_project")]
    pub project_name: String,
    #[serde(default)]
    pub profiles: String,
}
fn default_project() -> String {
    "talkops".into()
}

pub fn configuration_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 120
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && !name.starts_with("..")
        && (name == ".env"
            || name.ends_with(".env")
            || (name.starts_with("docker-compose")
                && (name.ends_with(".yml") || name.ends_with(".yaml"))))
}

pub fn validate_configuration(input: &DeploymentInput) -> Result<()> {
    if input.files.len() > 32 || input.files.keys().any(|n| !configuration_name(n)) {
        return Err(BackupError::Invalid(
            "select .env, *.env and docker-compose*.yml/yaml files only".into(),
        ));
    }
    if input.files.values().map(|s| s.len() as u64).sum::<u64>() > CONFIG_LIMIT {
        return Err(BackupError::Invalid(
            "deployment configuration exceeds 1 MiB".into(),
        ));
    }
    if input.project_name.is_empty()
        || !input
            .project_name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
    {
        return Err(BackupError::Invalid("invalid Compose project name".into()));
    }
    Ok(())
}

pub fn env_value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.trim().trim_start_matches("export ").split_once('='))
        .filter(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().trim_matches(['\'', '"']).to_owned())
        .next_back()
}

pub fn deployment_info(
    input: &DeploymentInput,
    source: &str,
    captured_at: DateTime<Utc>,
) -> DeploymentInfo {
    let mut missing = Vec::new();
    if !input.files.contains_key("docker-compose.yml")
        && !input.files.contains_key("docker-compose.yaml")
    {
        missing.push("docker-compose.yml".into());
    }
    let env = input.files.get(".env").map(String::as_str).unwrap_or("");
    for key in [
        "POSTGRES_PASSWORD",
        "TALKOPS_SECRET_KEY",
        "TALKOPS_ESL_PASSWORD",
        "TALKOPS_XMLCURL_PASSWORD",
    ] {
        if env_value(env, key).is_none_or(|v| v.is_empty() || v.contains('$')) {
            missing.push(key.into());
        }
    }
    if let Some(key) = env_value(env, "TALKOPS_SECRET_KEY") {
        if SecretBox::from_hex(&key).is_err() && !missing.iter().any(|s| s == "TALKOPS_SECRET_KEY")
        {
            missing.push("TALKOPS_SECRET_KEY".into());
        }
    }
    DeploymentInfo {
        source: source.into(),
        captured_at,
        complete: missing.is_empty(),
        missing,
        files: input.files.keys().cloned().collect(),
        project_name: input.project_name.clone(),
        profiles: input.profiles.clone(),
        postgres_version: String::new(),
    }
}

pub fn capture(cfg: &BackupConfig) -> Result<(DeploymentInput, DeploymentInfo)> {
    let (input, source, captured_at) = if let Some(dir) = &cfg.deployment_dir {
        let mut files = BTreeMap::new();
        for name in &cfg.deployment_files {
            if !configuration_name(name) {
                return Err(BackupError::Invalid("invalid deployment file name".into()));
            }
            let path = dir.join(name);
            match std::fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
                Ok(meta) if !meta.is_file() || meta.len() > CONFIG_LIMIT => {
                    return Err(BackupError::Invalid(
                        "deployment source must be a regular file smaller than 1 MiB".into(),
                    ));
                }
                Ok(_) => {}
            }
            let mut contents = String::new();
            File::open(path)?
                .take(CONFIG_LIMIT + 1)
                .read_to_string(&mut contents)?;
            files.insert(name.clone(), contents);
        }
        (
            DeploymentInput {
                files,
                project_name: cfg.project_name.clone(),
                profiles: cfg.profiles.clone(),
            },
            "mounted",
            Utc::now(),
        )
    } else if cfg.dir.join(IMPORT_FILE).is_file() {
        let key = cfg.deployment_key.as_ref().ok_or_else(|| {
            BackupError::Invalid("imported configuration requires TALKOPS_DEPLOYMENT_KEY".into())
        })?;
        let mut encrypted = String::new();
        File::open(cfg.dir.join(IMPORT_FILE))?
            .take(4 * CONFIG_LIMIT)
            .read_to_string(&mut encrypted)?;
        let plaintext = SecretBox::from_hex(key.expose_secret())
            .map_err(|_| BackupError::Invalid("invalid deployment key".into()))?
            .decrypt(&encrypted)
            .map_err(|_| BackupError::Invalid("cannot decrypt imported configuration".into()))?;
        let input = serde_json::from_str(&plaintext)
            .map_err(|_| BackupError::Invalid("invalid imported configuration".into()))?;
        (
            input,
            "imported",
            std::fs::metadata(cfg.dir.join(IMPORT_FILE))?
                .modified()?
                .into(),
        )
    } else {
        (
            DeploymentInput {
                files: BTreeMap::new(),
                project_name: cfg.project_name.clone(),
                profiles: cfg.profiles.clone(),
            },
            "missing",
            Utc::now(),
        )
    };
    validate_configuration(&input)?;
    // Recovery passwords must be supplied through a separate file or runtime environment.
    for text in input.files.values() {
        if env_value(text, "TALKOPS_BACKUP_PASSWORD").is_some_and(|p| !p.is_empty()) {
            return Err(BackupError::Invalid(
                "remove TALKOPS_BACKUP_PASSWORD from captured files; supply it separately".into(),
            ));
        }
        if let Some(password) = &cfg.password {
            if !password.expose_secret().is_empty() && text.contains(password.expose_secret()) {
                return Err(BackupError::Invalid(
                    "backup password must not be included in deployment files".into(),
                ));
            }
        }
    }
    let mut info = deployment_info(&input, source, captured_at);
    if source == "mounted" {
        for name in &cfg.deployment_files {
            if !input.files.contains_key(name) {
                info.missing.push(name.clone());
            }
        }
        info.complete = info.missing.is_empty();
    }
    if let (Some(key), Some(env)) = (&cfg.deployment_key, input.files.get(".env")) {
        if env_value(env, "TALKOPS_SECRET_KEY").is_some_and(|v| {
            !v.is_empty()
                && !v.contains('$')
                && hex::decode(v.trim()).ok() != hex::decode(key.expose_secret().trim()).ok()
        }) {
            return Err(BackupError::Invalid(
                "captured application key differs from the running instance".into(),
            ));
        }
    }
    Ok((input, info))
}

pub fn private_file(path: &Path) -> std::io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
}

pub fn file_info(path: &Path) -> Result<FileInfo> {
    let mut reader = File::open(path)?;
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buf = [0u8; 65536];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        size += n as u64;
    }
    Ok(FileInfo {
        size,
        sha256: hex::encode(hash.finalize()),
    })
}

pub fn append_bytes<W: Write>(tar: &mut tar::Builder<W>, name: &str, data: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    tar.append_data(&mut header, name, data)?;
    Ok(())
}

/// Staged files are private and are removed when the operation ends.
pub struct Prepared {
    pub root: tempfile::TempDir,
    pub manifest: Manifest,
}
impl Prepared {
    pub fn export(&self, target: &Path) -> Result<()> {
        if self
            .manifest
            .deployment
            .as_ref()
            .is_none_or(|d| d.files.is_empty())
        {
            return Err(BackupError::Invalid(
                "this backup has no deployment files; supply the original .env and Compose files"
                    .into(),
            ));
        }
        #[allow(unused_mut)]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(target)?; // Never reuse or overwrite an existing directory.
        for name in &self.manifest.deployment.as_ref().expect("checked").files {
            let bytes = std::fs::read(self.root.path().join("deployment").join(name))?;
            private_file(&target.join(name))?.write_all(&bytes)?;
        }
        private_file(&target.join("recovery.json"))?
            .write_all(&serde_json::to_vec_pretty(&self.manifest).expect("manifest"))?;
        Ok(())
    }
    pub fn bundle(&self) -> Result<Option<Vec<u8>>> {
        let Some(d) = &self.manifest.deployment else {
            return Ok(None);
        };
        if d.files.is_empty() {
            return Ok(None);
        }
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        for name in &d.files {
            append_bytes(
                &mut tar,
                name,
                &std::fs::read(self.root.path().join("deployment").join(name))?,
            )?;
        }
        append_bytes(
            &mut tar,
            "recovery.json",
            &serde_json::to_vec_pretty(&self.manifest).expect("manifest"),
        )?;
        Ok(Some(tar.into_inner()?.finish()?))
    }
}

fn safe_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|c| matches!(c, Component::Normal(_)))
        && !path.to_string_lossy().contains('\\')
        && !path.to_string_lossy().contains(':')
}

/// Fully authenticate, decompress and validate before database or volume mutation.
pub fn prepare(archive: &Path, password: Option<&SecretString>, limit: u64) -> Result<Prepared> {
    prepare_inner(archive, password, limit).map_err(|err| match err {
        BackupError::Io(_) => BackupError::Invalid(
            "archive validation failed: corrupt data, size limit or unavailable storage".into(),
        ),
        other => other,
    })
}

fn prepare_inner(archive: &Path, password: Option<&SecretString>, limit: u64) -> Result<Prepared> {
    let root = tempfile::tempdir()?;
    let mut source = std::io::BufReader::new(File::open(archive)?);
    let mut magic = [0u8; 2];
    source.read_exact(&mut magic)?;
    use std::io::Seek;
    source.rewind()?;
    let plain = root.path().join("archive.gz");
    let mut out = private_file(&plain)?;
    let mut reader: Box<dyn Read> = if magic == [0x1f, 0x8b] {
        Box::new(source)
    } else {
        let password = password.ok_or_else(|| {
            BackupError::Invalid("encrypted backup requires a backup password".into())
        })?;
        let decryptor = age::Decryptor::new(source)
            .map_err(|_| BackupError::Invalid("invalid encrypted backup".into()))?;
        if !decryptor.is_scrypt() {
            return Err(BackupError::Invalid(
                "expected a password-encrypted backup".into(),
            ));
        }
        let mut identity = age::scrypt::Identity::new(password.clone());
        identity.set_max_work_factor(18);
        Box::new(
            decryptor
                .decrypt(std::iter::once(&identity as &dyn age::Identity))
                .map_err(|_| {
                    BackupError::Invalid("wrong backup password or invalid encrypted backup".into())
                })?,
        )
    };
    if std::io::copy(&mut reader.by_ref().take(limit.saturating_add(1)), &mut out)? > limit {
        return Err(BackupError::Invalid(
            "archive exceeds restore size limit".into(),
        ));
    }
    out.sync_all()?;
    let decoder = flate2::read::MultiGzDecoder::new(File::open(&plain)?);
    let mut tar = tar::Archive::new(decoder.take(limit.saturating_add(1)));
    let mut seen = BTreeSet::new();
    let mut actual = BTreeMap::new();
    let mut total = 0u64;
    let mut configuration_bytes = 0u64;
    for entry in tar.entries()? {
        let mut entry = entry?;
        let raw = entry.path_bytes();
        let raw = std::str::from_utf8(&raw)
            .map_err(|_| BackupError::Invalid("non-UTF-8 archive path".into()))?;
        if raw.contains(['\\', ':']) {
            return Err(BackupError::Invalid("unsafe archive path".into()));
        }
        let path = entry.path()?.into_owned();
        let name = path.to_string_lossy().replace('\\', "/");
        if !safe_path(&path) || !seen.insert(name.clone()) || seen.len() > MAX_ENTRIES {
            return Err(BackupError::Invalid(
                "unsafe, duplicate or excessive archive paths".into(),
            ));
        }
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            return Err(BackupError::Invalid(
                "archive links and devices are forbidden".into(),
            ));
        }
        if kind.is_dir() && entry.size() != 0 {
            return Err(BackupError::Invalid("invalid archive directory".into()));
        }
        let allowed = if name == "manifest.json" || name == "database.dump" {
            kind.is_file()
        } else if let Some(n) = name.strip_prefix("deployment/") {
            configuration_name(n) && kind.is_file()
        } else if let Some(rest) = name.strip_prefix("files/") {
            [
                "voicemail",
                "sounds",
                "recordings",
                "snapshots",
                "provisioning",
            ]
            .iter()
            .any(|v| rest == *v || rest.starts_with(&format!("{v}/")))
        } else {
            false
        };
        if !allowed {
            return Err(BackupError::Invalid("unexpected archive entry".into()));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| BackupError::Invalid("archive too large".into()))?;
        if total > limit || (name == "manifest.json" && entry.size() > CONFIG_LIMIT) {
            return Err(BackupError::Invalid(
                "archive exceeds restore size limit".into(),
            ));
        }
        if name.starts_with("deployment/") {
            configuration_bytes += entry.size();
            if configuration_bytes > CONFIG_LIMIT {
                return Err(BackupError::Invalid(
                    "deployment configuration exceeds 1 MiB".into(),
                ));
            }
        }
        if fs2::available_space(root.path())? < entry.size() {
            return Err(BackupError::Invalid(
                "insufficient staging disk space".into(),
            ));
        }
        let dest = root.path().join(&path);
        if kind.is_dir() {
            std::fs::create_dir_all(dest)?;
            continue;
        }
        std::fs::create_dir_all(dest.parent().expect("parent"))?;
        std::io::copy(&mut entry, &mut private_file(&dest)?)?;
        if name != "manifest.json" {
            actual.insert(name, file_info(&dest)?);
        }
    }
    // Consume gzip through EOF to validate checksums, size and trailing streams.
    let mut remainder = tar.into_inner();
    std::io::copy(&mut remainder, &mut std::io::sink())?;
    if remainder.limit() == 0 {
        return Err(BackupError::Invalid(
            "decompressed archive exceeds restore size limit".into(),
        ));
    }
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(root.path().join("manifest.json"))?)
            .map_err(|_| BackupError::Invalid("invalid backup manifest".into()))?;
    if ![1, 2].contains(&manifest.format) {
        return Err(BackupError::Invalid("unsupported backup format".into()));
    }
    let latest = talkops_core::db::MIGRATOR
        .iter()
        .map(|m| m.version)
        .max()
        .unwrap_or(0);
    if manifest.schema_version > latest {
        return Err(BackupError::Invalid(
            "backup schema is newer than this TalkOps; update first".into(),
        ));
    }
    let mut dump_magic = [0; 5];
    File::open(root.path().join("database.dump"))?.read_exact(&mut dump_magic)?;
    if &dump_magic != b"PGDMP" {
        return Err(BackupError::Invalid("invalid PostgreSQL dump".into()));
    }
    if manifest.volumes.iter().any(|v| {
        ![
            "voicemail",
            "sounds",
            "recordings",
            "snapshots",
            "provisioning",
        ]
        .contains(&v.as_str())
    }) || manifest.volumes.iter().collect::<BTreeSet<_>>().len() != manifest.volumes.len()
    {
        return Err(BackupError::Invalid("invalid volume manifest".into()));
    }
    for name in actual.keys().filter(|n| n.starts_with("files/")) {
        let vol = name.split('/').nth(1).expect("volume");
        if !manifest.volumes.iter().any(|v| v == vol) {
            return Err(BackupError::Invalid("unlisted volume".into()));
        }
    }
    if manifest.format == 2
        && (manifest.inventory.len() != actual.len()
            || actual.iter().any(|(n, i)| {
                manifest
                    .inventory
                    .get(n)
                    .is_none_or(|m| m.size != i.size || m.sha256 != i.sha256)
            }))
    {
        return Err(BackupError::Invalid(
            "backup inventory or checksum mismatch".into(),
        ));
    }
    let names: Vec<String> = actual
        .keys()
        .filter_map(|n| n.strip_prefix("deployment/").map(str::to_owned))
        .collect();
    if manifest
        .deployment
        .as_ref()
        .map(|d| &d.files)
        .unwrap_or(&Vec::new())
        != &names
    {
        return Err(BackupError::Invalid("deployment inventory mismatch".into()));
    }
    if !names.is_empty() && magic == [0x1f, 0x8b] {
        return Err(BackupError::Invalid(
            "deployment secrets require an encrypted archive".into(),
        ));
    }
    std::fs::remove_file(plain)?;
    Ok(Prepared { root, manifest })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn legacy(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        append_bytes(&mut tar, "database.dump", b"PGDMP-test").unwrap();
        for (name, data) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o600);
            header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
            header.set_cksum();
            tar.append(&header, *data).unwrap();
        }
        append_bytes(&mut tar,"manifest.json",br#"{"format":1,"talkops_version":"1.7.5","created_at":"2026-10-12T00:00:00Z","schema_version":0,"volumes":["voicemail"]}"#).unwrap();
        tar.into_inner().unwrap().finish().unwrap()
    }
    #[test]
    fn legacy_complete_scan_limits_duplicates_and_corruption() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("archive");
        let bytes = legacy(&[("files/voicemail/msg.wav", b"message")]);
        std::fs::write(&file, &bytes).unwrap();
        let prepared = prepare(&file, None, 1 << 20).unwrap();
        assert_eq!(
            std::fs::read(prepared.root.path().join("files/voicemail/msg.wav")).unwrap(),
            b"message"
        );
        assert!(prepared.bundle().unwrap().is_none());
        assert!(prepared.export(&root.path().join("out")).is_err());
        assert!(prepare(&file, None, 10).is_err());
        std::fs::write(&file, legacy(&[("database.dump", b"duplicate")])).unwrap();
        assert!(prepare(&file, None, 1 << 20).is_err());
        std::fs::write(&file, &bytes[..bytes.len() - 5]).unwrap();
        assert!(prepare(&file, None, 1 << 20).is_err());
        std::fs::write(&file, legacy(&[("files/voicemail/evil\\path", b"bad")])).unwrap();
        assert!(prepare(&file, None, 1 << 20).is_err());
    }
    #[test]
    fn paths_and_configuration_are_explicit() {
        for path in ["../x", "/tmp/x", "files/voicemail/../x", "C:/x", "a\\b"] {
            assert!(!safe_path(Path::new(path)));
        }
        for name in [
            ".env",
            "docker-compose.yml",
            "docker-compose.backup.yml",
            "custom.env",
        ] {
            assert!(configuration_name(name));
        }
        for name in ["../.env", "file.sh", "../../docker-compose.yml"] {
            assert!(!configuration_name(name));
        }
        assert_eq!(
            env_value("export VALUE='abc'\nVALUE=def\n", "VALUE"),
            Some("def".into())
        );
    }
    #[test]
    fn encrypted_configuration_roundtrip_password_and_export() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("backup.age");
        let data = BTreeMap::from([
            (
                ".env".into(),
                format!(
                    "TALKOPS_SECRET_KEY={}\nPOSTGRES_PASSWORD=test\nTALKOPS_ESL_PASSWORD=test\nTALKOPS_XMLCURL_PASSWORD=test\n",
                    "01".repeat(32)
                ),
            ),
            ("docker-compose.yml".into(), "services: {}\n".into()),
        ]);
        let input = DeploymentInput {
            files: data,
            project_name: "talkops".into(),
            profiles: "caddy".into(),
        };
        let mut manifest = Manifest {
            format: 2,
            talkops_version: "1.7.5".into(),
            created_at: Utc::now(),
            schema_version: 0,
            volumes: vec![],
            inventory: BTreeMap::new(),
            deployment: Some(deployment_info(&input, "mounted", Utc::now())),
        };
        let mut files = BTreeMap::from([("database.dump".into(), b"PGDMP-test".to_vec())]);
        for (name, value) in &input.files {
            files.insert(format!("deployment/{name}"), value.as_bytes().to_vec());
        }
        for (name, bytes) in &files {
            manifest.inventory.insert(
                name.clone(),
                FileInfo {
                    size: bytes.len() as u64,
                    sha256: hex::encode(Sha256::digest(bytes)),
                },
            );
        }
        let pass: SecretString = "separate-long-test-password".to_owned().into();
        let writer = age::Encryptor::with_user_passphrase(pass.clone())
            .wrap_output(private_file(&archive).unwrap())
            .unwrap();
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            writer,
            flate2::Compression::fast(),
        ));
        for (name, bytes) in &files {
            append_bytes(&mut tar, name, bytes).unwrap();
        }
        append_bytes(
            &mut tar,
            "manifest.json",
            &serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        tar.into_inner()
            .unwrap()
            .finish()
            .unwrap()
            .finish()
            .unwrap();
        assert!(prepare(&archive, None, 1 << 20).is_err());
        assert!(
            prepare(
                &archive,
                Some(&"wrong-long-test-password".to_owned().into()),
                1 << 20
            )
            .is_err()
        );
        let prepared = prepare(&archive, Some(&pass), 1 << 20).unwrap();
        let output = root.path().join("out");
        prepared.export(&output).unwrap();
        assert_eq!(
            std::fs::read_to_string(output.join(".env")).unwrap(),
            input.files[".env"]
        );
        assert!(prepared.export(&output).is_err());
        assert!(prepared.bundle().unwrap().is_some());
        let bytes = std::fs::read(&archive).unwrap();
        std::fs::write(&archive, &bytes[..bytes.len() - 1]).unwrap();
        assert!(prepare(&archive, Some(&pass), 1 << 20).is_err());
    }
    #[test]
    fn checksum_schema_and_links_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("test");
        for case in 0..3 {
            let mut manifest = serde_json::json!({"format":1,"talkops_version":"1.7.5","created_at":"2026-10-12T00:00:00Z","schema_version":0,"volumes":["voicemail"]});
            if case == 0 {
                manifest["schema_version"] = serde_json::json!(i64::MAX);
            }
            if case == 1 {
                manifest["format"] = serde_json::json!(2);
            }
            let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
                private_file(&archive).unwrap(),
                flate2::Compression::fast(),
            ));
            append_bytes(&mut tar, "database.dump", b"PGDMP-test").unwrap();
            if case == 2 {
                let mut header = tar::Header::new_gnu();
                header.set_size(0);
                header.set_mode(0o600);
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_link_name("/etc/passwd").unwrap();
                header.set_cksum();
                tar.append_data(&mut header, "files/voicemail/link", std::io::empty())
                    .unwrap();
            }
            append_bytes(
                &mut tar,
                "manifest.json",
                &serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            tar.into_inner().unwrap().finish().unwrap();
            let error = prepare(&archive, None, 1 << 20)
                .err()
                .expect("rejection")
                .to_string();
            assert!(
                error.contains(["newer", "checksum", "links"][case]),
                "{error}"
            );
            std::fs::remove_file(&archive).unwrap();
        }
    }
}
