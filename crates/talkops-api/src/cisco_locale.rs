//! Cisco Enterprise user locales from administrator-supplied Cisco locale
//! installer packages (`po-locale-<lang>-<version>.cop.sha512`). TalkOps
//! never ships these files. It reads only the TFTP files of the package and
//! never runs the package scripts.
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};
use talkops_provisioning::vendors::CiscoLocale;

/// Upload limit for one package (the German package has 3.5 MB).
pub const MAX_PACKAGE: usize = 32 * 1024 * 1024;
const MAX_FILE: u64 = 4 * 1024 * 1024;
const MAX_TOTAL: u64 = 24 * 1024 * 1024;
const MAX_FILES: usize = 200;
/// The signature header comes before the gzip data.
const MAX_HEADER: usize = 64 * 1024;
const META: &str = "locale.json";

#[derive(Debug, thiserror::Error)]
pub enum LocaleError {
    #[error("not a Cisco locale installer package")]
    Format,
    #[error("the package has no phone locale files")]
    Empty,
    #[error("the package is too large")]
    TooLarge,
    #[error("locale storage failed")]
    Io(#[from] std::io::Error),
}

/// A parsed package: the locale and its TFTP files.
#[derive(Debug)]
pub struct Package {
    pub locale: CiscoLocale,
    pub files: BTreeMap<String, Vec<u8>>,
}

/// `german_germany` -> `German_Germany`.
fn cisco_name(dir: &str) -> String {
    dir.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("_")
}

fn safe_file(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

fn read_limited(reader: impl Read, limit: u64) -> Result<Vec<u8>, LocaleError> {
    let mut out = Vec::new();
    reader.take(limit + 1).read_to_end(&mut out)?;
    if out.len() as u64 > limit {
        return Err(LocaleError::TooLarge);
    }
    Ok(out)
}

/// Parses a signed Cisco locale installer package: a signature header,
/// then a gzip tar with an inner `po-locale-<lang>-<version>.tar`.
pub fn parse(data: &[u8]) -> Result<Package, LocaleError> {
    let start = data[..data.len().min(MAX_HEADER)]
        .windows(3)
        .position(|w| w == [0x1f, 0x8b, 0x08])
        .ok_or(LocaleError::Format)?;
    let mut outer = tar::Archive::new(flate2::read::GzDecoder::new(&data[start..]));
    let mut inner = None;
    for entry in outer.entries().map_err(|_| LocaleError::Format)? {
        let entry = entry.map_err(|_| LocaleError::Format)?;
        let path = entry.path().map_err(|_| LocaleError::Format)?;
        let name = path.to_string_lossy().to_string();
        if let Some(code) = name
            .strip_prefix("po-locale-")
            .filter(|n| n.ends_with(".tar") && !n.contains('/'))
            .and_then(|n| n.get(..5))
        {
            let code = code.to_owned();
            inner = Some((code, read_limited(entry, MAX_TOTAL * 2)?));
            break;
        }
    }
    let (lang_code, inner) = inner.ok_or(LocaleError::Format)?;
    let mut dir_name = None;
    let mut version = None;
    let mut files = BTreeMap::new();
    let mut total = 0;
    for entry in tar::Archive::new(inner.as_slice())
        .entries()
        .map_err(|_| LocaleError::Format)?
    {
        let entry = entry.map_err(|_| LocaleError::Format)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|_| LocaleError::Format)?
            .to_string_lossy()
            .to_string();
        let parts: Vec<&str> = path.split('/').collect();
        match parts.as_slice() {
            ["usr", "local", "cm", "tftp", dir, file] if safe_file(file) => {
                if dir_name.get_or_insert_with(|| dir.to_string()).as_str() != *dir {
                    return Err(LocaleError::Format);
                }
                total += entry.header().size().unwrap_or(u64::MAX);
                if files.len() >= MAX_FILES || total > MAX_TOTAL {
                    return Err(LocaleError::TooLarge);
                }
                files.insert(file.to_string(), read_limited(entry, MAX_FILE)?);
            }
            [
                "usr",
                "local",
                "cm",
                "application_locale",
                "localeinstaller",
                _,
                "localeversion.txt",
            ] => {
                let text = String::from_utf8(read_limited(entry, 4096)?)
                    .map_err(|_| LocaleError::Format)?;
                version = text.lines().find_map(|line| {
                    let value = line.split_once(':')?.1.trim();
                    // "14.3(1.9900)DEU" -> "14.3(1.9900)".
                    let end = value.rfind(')').map(|i| i + 1).unwrap_or(value.len());
                    Some(value[..end].to_owned())
                });
            }
            _ => {}
        }
    }
    let dir = dir_name.ok_or(LocaleError::Empty)?;
    let locale = CiscoLocale {
        name: cisco_name(&dir),
        lang_code,
        version: version.ok_or(LocaleError::Format)?,
    };
    if !locale.is_valid() || !dir.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return Err(LocaleError::Format);
    }
    if !files.contains_key("sp-sip.jar") {
        // The 78xx phones request only this file for the user locale.
        return Err(LocaleError::Empty);
    }
    Ok(Package { locale, files })
}

fn root(provisioning_dir: &Path) -> PathBuf {
    provisioning_dir.join("cisco-locales")
}

/// Stores a package and replaces an older one with the same name.
pub async fn store(provisioning_dir: &Path, package: &Package) -> Result<(), LocaleError> {
    let root = root(provisioning_dir);
    tokio::fs::create_dir_all(&root).await?;
    let staging = root.join(format!(".{}.new", package.locale.name));
    let _ = tokio::fs::remove_dir_all(&staging).await;
    tokio::fs::create_dir(&staging).await?;
    tokio::fs::create_dir(staging.join("files")).await?;
    for (name, body) in &package.files {
        tokio::fs::write(staging.join("files").join(name), body).await?;
    }
    let meta = serde_json::to_vec(&package.locale).map_err(std::io::Error::other)?;
    tokio::fs::write(staging.join(META), meta).await?;
    let target = root.join(&package.locale.name);
    let old = root.join(format!(".{}.old", package.locale.name));
    let _ = tokio::fs::remove_dir_all(&old).await;
    match tokio::fs::rename(&target, &old).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    tokio::fs::rename(&staging, &target).await?;
    let _ = tokio::fs::remove_dir_all(&old).await;
    Ok(())
}

/// Installed locales, sorted by name. Invalid entries are skipped.
pub async fn list(provisioning_dir: &Path) -> Vec<CiscoLocale> {
    let mut out = Vec::new();
    let Ok(mut dir) = tokio::fs::read_dir(root(provisioning_dir)).await else {
        return out;
    };
    while let Ok(Some(entry)) = dir.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = tokio::fs::read(entry.path().join(META)).await else {
            continue;
        };
        if let Ok(locale) = serde_json::from_slice::<CiscoLocale>(&meta)
            && locale.is_valid()
            && locale.name == name
        {
            out.push(locale);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Removes an installed locale. Returns false if it does not exist.
pub async fn remove(provisioning_dir: &Path, name: &str) -> Result<bool, LocaleError> {
    if !name.chars().all(|c| c.is_ascii_alphabetic() || c == '_') || name.is_empty() {
        return Ok(false);
    }
    match tokio::fs::remove_dir_all(root(provisioning_dir).join(name)).await {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// The TFTP files of an installed locale.
pub async fn files(
    provisioning_dir: &Path,
    name: &str,
) -> Result<BTreeMap<String, Vec<u8>>, LocaleError> {
    let dir = root(provisioning_dir).join(name).join("files");
    let mut out = BTreeMap::new();
    let mut entries = tokio::fs::read_dir(&dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let file = entry.file_name().to_string_lossy().to_string();
        if safe_file(&file) && entry.file_type().await?.is_file() {
            out.insert(file, tokio::fs::read(entry.path()).await?);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small package with the real layout: header, gzip tar, inner tar.
    fn package(dir: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut inner = tar::Builder::new(Vec::new());
        let add = |builder: &mut tar::Builder<Vec<u8>>, path: &str, body: &[u8]| {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, path, body).unwrap();
        };
        for (name, body) in files {
            add(&mut inner, &format!("usr/local/cm/tftp/{dir}/{name}"), body);
        }
        add(
            &mut inner,
            &format!("usr/local/cm/application_locale/localeinstaller/{dir}/localeversion.txt"),
            b"User Locale Version : 14.3(1.9900)DEU\nBuild Time : 05/08/2025\n",
        );
        let inner = inner.into_inner().unwrap();
        let mut outer = tar::Builder::new(Vec::new());
        add(&mut outer, "po-locale-de_DE-14.3.1.5000-1.tar", &inner);
        add(&mut outer, "copstart.sh", b"#!/bin/sh\nexit 1\n");
        let tar = outer.into_inner().unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, &tar).unwrap();
        let mut out = b"\x01\x00\x02PT=ccmPatch_cop3;KT=release\x00signature".to_vec();
        out.extend(gz.finish().unwrap());
        out
    }

    #[test]
    fn parses_the_cisco_package_layout() {
        let data = package(
            "german_germany",
            &[("sp-sip.jar", b"jar"), ("7960-dictionary.xml", b"xml")],
        );
        let parsed = parse(&data).unwrap();
        assert_eq!(
            parsed.locale,
            CiscoLocale {
                name: "German_Germany".into(),
                lang_code: "de_DE".into(),
                version: "14.3(1.9900)".into(),
            }
        );
        assert_eq!(parsed.files.len(), 2);
        assert_eq!(parsed.files["sp-sip.jar"], b"jar");
    }

    #[test]
    fn rejects_other_files_and_unsafe_names() {
        assert!(matches!(parse(b"not a package"), Err(LocaleError::Format)));
        let no_jar = package("german_germany", &[("7960-dictionary.xml", b"x")]);
        assert!(matches!(parse(&no_jar), Err(LocaleError::Empty)));
        // Unsafe file names are skipped, never written.
        let odd = package("german_germany", &[("sp-sip.jar", b"j"), ("a b.jar", b"x")]);
        assert_eq!(parse(&odd).unwrap().files.len(), 1);
    }

    #[tokio::test]
    async fn stores_lists_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let parsed = parse(&package("german_germany", &[("sp-sip.jar", b"j")])).unwrap();
        store(dir.path(), &parsed).await.unwrap();
        store(dir.path(), &parsed).await.unwrap();
        let installed = list(dir.path()).await;
        assert_eq!(installed, vec![parsed.locale.clone()]);
        let stored = files(dir.path(), "German_Germany").await.unwrap();
        assert_eq!(stored["sp-sip.jar"], b"j");
        assert!(!remove(dir.path(), "../x").await.unwrap());
        assert!(remove(dir.path(), "German_Germany").await.unwrap());
        assert!(list(dir.path()).await.is_empty());
    }
}
