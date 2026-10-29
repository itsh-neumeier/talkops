//! The model library of the administrator (ADR 0051, ADR 0052).
//!
//! TalkOps does not ship model weights. An administrator installs sherpa-onnx
//! model archives (`.tar`, `.tar.gz`, `.tar.bz2`) by upload or download and
//! selects one library model per slot. A multilingual model can fill the
//! `de` and `en` slots; it is stored once. The selection takes effect after
//! a restart.
//!
//! An archive is unpacked into a staging directory. Only the ONNX files and
//! the token list are kept: one file per role of the model family, the
//! full-precision variant first. The service checks the result: the files
//! exist, each ONNX file starts like an ONNX protobuf, and (optionally) a
//! child process loads the model with sherpa-onnx. Only a checked model
//! moves into the library.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use talkops_stt_service::library::{
    ID_LEN, InstallError, Usage, dir_bytes, invalid, model_id, now, read_json, remove_dir, sha256,
    sha256_file, statement,
};

use crate::engine::LoadedModel;
use crate::models::{
    Catalog, Family, LANGUAGES, LIBRARY, MANIFEST, Manifest, SELECTION, SLOTS, Selection,
    read_selection,
};

/// Library limits, so installations cannot fill the volume.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Unpacked size of one model (and size of one archive).
    pub max_model_bytes: u64,
    pub max_library_bytes: u64,
    pub max_models: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_model_bytes: 4 << 30,
            max_library_bytes: 20 << 30,
            max_models: 10,
        }
    }
}

/// How a new model is checked before it goes into the library.
#[derive(Debug, Clone)]
pub enum ModelCheck {
    /// Files and ONNX headers only (mock backend, tests).
    Files,
    /// Also load the model in a child process (`<exe> check-model`).
    Load {
        exe: PathBuf,
        provider: String,
        timeout: Duration,
    },
}

/// Which variant of a model file to keep when an archive has several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Precision {
    /// Full precision first (int8 operators run partly on the CPU with the
    /// CUDA provider).
    #[default]
    Full,
    Int8,
}

/// The statements of an installation.
#[derive(Debug, Clone, Default)]
pub struct Install {
    pub name: String,
    pub family: String,
    /// Comma-separated ISO 639-1 codes.
    pub languages: String,
    pub license: String,
    pub source: String,
    /// Expected SHA-256 of the archive (required for downloads).
    pub sha256: Option<String>,
    pub precision: Precision,
}

/// One library model.
#[derive(Debug, Clone, Serialize)]
pub struct LibraryModel {
    pub id: String,
    pub manifest: Manifest,
    pub bytes: u64,
    /// Slots that select this model.
    pub slots: Vec<String>,
}

/// One slot as the admin interface shows it.
#[derive(Debug, Clone, Serialize)]
pub struct SlotStatus {
    pub slot: &'static str,
    /// Library id of the selected model.
    pub model_id: Option<String>,
    pub name: Option<String>,
    /// The running service has loaded the selected model.
    pub active: bool,
    /// The running service has a model for this slot (maybe another one).
    pub loaded: bool,
    /// The selected model failed to load at startup.
    pub error: Option<String>,
    /// VRAM of the loaded model, MiB.
    pub vram_mib: Option<u64>,
}

/// Slots and library for the admin interface.
#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    pub slots: Vec<SlotStatus>,
    pub library: Vec<LibraryModel>,
    pub usage: Usage,
}

static STAGING: AtomicU64 = AtomicU64::new(0);

fn library(dir: &Path) -> PathBuf {
    dir.join(LIBRARY)
}

/// A new staging path in the library directory (same file system as the
/// target, so the final rename is atomic).
pub fn staging_path(dir: &Path, kind: &str) -> PathBuf {
    library(dir).join(format!(
        ".{kind}-{}-{}",
        std::process::id(),
        STAGING.fetch_add(1, Ordering::Relaxed)
    ))
}

fn slot(name: &str) -> Result<&'static str, InstallError> {
    SLOTS
        .into_iter()
        .find(|s| *s == name)
        .ok_or_else(|| invalid("unknown model slot; use de, en or langid"))
}

fn languages(value: &str) -> Result<Vec<String>, InstallError> {
    let list: Vec<String> = value
        .split([',', ' '])
        .map(|l| l.trim().to_ascii_lowercase())
        .filter(|l| !l.is_empty())
        .collect();
    if list.is_empty()
        || list.len() > 32
        || list
            .iter()
            .any(|l| l.len() != 2 || !l.bytes().all(|b| b.is_ascii_lowercase()))
    {
        return Err(invalid(
            "languages: one or more ISO 639-1 codes are required, e.g. de,en",
        ));
    }
    Ok(list)
}

/// The decoder of an archive, by its first bytes.
fn open_archive(path: &Path) -> Result<Box<dyn Read>, InstallError> {
    let mut magic = [0u8; 3];
    let mut file = std::fs::File::open(path)?;
    let n = file.read(&mut magic)?;
    let file = std::fs::File::open(path)?;
    Ok(match &magic[..n] {
        [0x1f, 0x8b, ..] => Box::new(flate2::read::GzDecoder::new(file)),
        b"BZh" => Box::new(bzip2::read::BzDecoder::new(file)),
        _ => Box::new(file),
    })
}

/// Unpacks the ONNX files and token lists of an archive flat into `to`.
fn unpack(archive: &Path, to: &Path, limits: &Limits) -> Result<(), InstallError> {
    std::fs::create_dir_all(to)?;
    let mut tar = tar::Archive::new(open_archive(archive)?);
    let mut total = 0u64;
    let entries = tar
        .entries()
        .map_err(|e| invalid(format!("not a tar archive: {e}")))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| invalid(format!("damaged archive: {e}")))?;
        let kind = entry.header().entry_type();
        let path = entry
            .path()
            .map_err(|e| invalid(format!("damaged archive: {e}")))?
            .into_owned();
        if path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(invalid("the archive has an unsafe path"));
        }
        if kind.is_dir() {
            continue;
        }
        if !kind.is_file() {
            return Err(invalid(
                "the archive may only have files and directories (no links)",
            ));
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !(name.ends_with(".onnx") || name.ends_with("tokens.txt")) || name.starts_with('.') {
            continue;
        }
        let target = to.join(name);
        if target.exists() {
            return Err(invalid(format!("the archive has two files named {name}")));
        }
        total += entry.header().size().unwrap_or(0);
        if total > limits.max_model_bytes {
            return Err(invalid(format!(
                "the model is larger than {} MiB",
                limits.max_model_bytes >> 20
            )));
        }
        let mut out = std::fs::File::create(&target)?;
        let copied = std::io::copy(&mut (&mut entry).take(limits.max_model_bytes + 1), &mut out)?;
        if copied > limits.max_model_bytes {
            return Err(invalid("a model file is too large"));
        }
    }
    Ok(())
}

fn is_int8(name: &str) -> bool {
    name.contains(".int8.") || name.contains("_int8") || name.contains("-int8")
}

/// The file for a role from the unpacked names.
fn pick(names: &[String], role: &str, precision: Precision) -> Option<String> {
    let mut candidates: Vec<&String> = if role == "tokens" {
        names.iter().filter(|n| n.ends_with("tokens.txt")).collect()
    } else if role == "model" {
        names
            .iter()
            .filter(|n| {
                n.ends_with(".onnx")
                    && !["encoder", "decoder", "joiner"]
                        .iter()
                        .any(|r| n.contains(r))
            })
            .collect()
    } else {
        names
            .iter()
            .filter(|n| n.ends_with(".onnx") && n.contains(role))
            .collect()
    };
    // Shortest name first (`tokens.txt` before `bpe-tokens.txt`), then the
    // precision.
    candidates.sort_by_key(|n| {
        (
            is_int8(n) != (precision == Precision::Int8),
            n.len(),
            (*n).clone(),
        )
    });
    candidates.first().map(|n| (*n).clone())
}

/// An ONNX model is a protobuf `ModelProto`; its first field is usually
/// `ir_version` (field 1, varint: byte 0x08).
fn looks_like_onnx(path: &Path) -> bool {
    let mut first = [0u8; 1];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut first))
        .is_ok()
        && first[0] == 0x08
}

fn run_check(check: &ModelCheck, dir: &Path) -> Result<(), InstallError> {
    let ModelCheck::Load {
        exe,
        provider,
        timeout,
    } = check
    else {
        return Ok(());
    };
    let log = dir.join(".check.log");
    let mut child = Command::new(exe)
        .args(["check-model", "--dir"])
        .arg(dir)
        .args(["--provider", provider])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&log)?)
        .spawn()?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if started.elapsed() > *timeout {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let output = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    match status {
        Some(s) if s.success() => Ok(()),
        Some(_) => {
            let tail: String = output
                .lines()
                .rev()
                .take(3)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" | ");
            Err(invalid(format!(
                "sherpa-onnx cannot load the model: {tail}"
            )))
        }
        None => Err(invalid("loading the model took too long")),
    }
}

fn library_ids(dir: &Path) -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(library(dir))
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| model_id(name).is_ok())
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

fn usage(dir: &Path, limits: &Limits) -> Usage {
    let ids = library_ids(dir);
    Usage {
        models: ids.len(),
        bytes: ids.iter().map(|id| dir_bytes(&library(dir).join(id))).sum(),
        max_models: limits.max_models,
        max_bytes: limits.max_library_bytes,
    }
}

/// Adds an archive to the library. An invalid archive changes nothing.
/// The archive file is not removed.
pub fn add_archive(
    dir: &Path,
    archive: &Path,
    install: Install,
    limits: &Limits,
    check: &ModelCheck,
) -> Result<LibraryModel, InstallError> {
    let name = statement(&install.name, "name")?;
    let license = statement(&install.license, "license")?;
    let source = statement(&install.source, "source")?;
    let family = Family::parse(&install.family)
        .ok_or_else(|| invalid("family: use nemo_ctc, nemo_transducer, canary or whisper"))?;
    let languages = languages(&install.languages)?;
    let archive_sha256 = sha256_file(archive)?;
    if let Some(expected) = &install.sha256
        && !expected.trim().eq_ignore_ascii_case(&archive_sha256)
    {
        return Err(invalid(format!(
            "the archive has the SHA-256 {archive_sha256}, not the expected one"
        )));
    }
    std::fs::create_dir_all(library(dir))?;
    let staging = staging_path(dir, "staging");
    let result = (|| {
        unpack(archive, &staging, limits)?;
        let names: Vec<String> = std::fs::read_dir(&staging)?
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        let mut files = BTreeMap::new();
        for role in family.roles().iter().chain(&["tokens"]) {
            let file = pick(&names, role, install.precision).ok_or_else(|| {
                invalid(format!(
                    "the archive has no {role} file for the family {}",
                    install.family.trim()
                ))
            })?;
            files.insert((*role).to_owned(), file);
        }
        for name in &names {
            if !files.values().any(|f| f == name) {
                std::fs::remove_file(staging.join(name))?;
            }
        }
        let mut digests = BTreeMap::new();
        for file in files.values() {
            let path = staging.join(file);
            if file.ends_with(".onnx") && !looks_like_onnx(&path) {
                return Err(invalid(format!("{file} is not an ONNX model")));
            }
            digests.insert(file.clone(), sha256_file(&path)?);
        }
        let id_source: String = files
            .iter()
            .map(|(role, file)| format!("{role}:{}\n", digests[file]))
            .collect();
        let id: String = sha256(id_source.as_bytes()).chars().take(ID_LEN).collect();
        let manifest = Manifest {
            name,
            family,
            languages,
            files,
            license,
            source,
            installed_at_unix: now(),
            archive_sha256: Some(archive_sha256),
            sha256: digests,
        };
        let target = library(dir).join(&id);
        if target.join(MANIFEST).is_file() {
            return Err(invalid(format!(
                "the model is already in the library ({id})"
            )));
        }
        let u = usage(dir, limits);
        let bytes = dir_bytes(&staging);
        if u.models >= limits.max_models || u.bytes + bytes > limits.max_library_bytes {
            return Err(invalid(format!(
                "the library is full ({} models, {} MiB); delete a model first",
                limits.max_models,
                limits.max_library_bytes >> 20
            )));
        }
        std::fs::write(
            staging.join(MANIFEST),
            serde_json::to_vec_pretty(&manifest).map_err(|e| invalid(e.to_string()))?,
        )?;
        run_check(check, &staging)?;
        remove_dir(&target)?;
        std::fs::rename(&staging, &target)?;
        Ok(LibraryModel {
            id,
            manifest,
            bytes,
            slots: Vec::new(),
        })
    })();
    if result.is_err() {
        let _ = remove_dir(&staging);
    }
    result
}

fn write_selection(dir: &Path, selection: &Selection) -> Result<(), InstallError> {
    let tmp = dir.join(".selection.json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(selection).map_err(|e| invalid(e.to_string()))?,
    )?;
    std::fs::rename(tmp, dir.join(SELECTION))?;
    Ok(())
}

fn manifest(dir: &Path, id: &str) -> Option<Manifest> {
    read_json(&library(dir).join(id).join(MANIFEST))
}

/// Selects a library model for a slot (active after a restart).
pub fn select(dir: &Path, slot_name: &str, id: &str) -> Result<(), InstallError> {
    let slot = slot(slot_name)?;
    let id = model_id(id)?;
    let manifest = manifest(dir, id).ok_or_else(|| invalid("unknown library model"))?;
    if slot == "langid" && manifest.family != Family::Whisper {
        return Err(invalid("language ID needs a multilingual Whisper model"));
    }
    if LANGUAGES.contains(&slot) && !manifest.languages.iter().any(|l| l == slot) {
        return Err(invalid(format!(
            "the model is not stated for the language `{slot}`"
        )));
    }
    let mut selection = read_selection(dir);
    selection.insert(slot.to_owned(), id.to_owned());
    write_selection(dir, &selection)
}

/// Empties a slot; the model stays in the library.
pub fn clear(dir: &Path, slot_name: &str) -> Result<(), InstallError> {
    let slot = slot(slot_name)?;
    let mut selection = read_selection(dir);
    if selection.remove(slot).is_some() {
        write_selection(dir, &selection)?;
    }
    Ok(())
}

/// Deletes a library model that no slot selects.
pub fn delete(dir: &Path, id: &str) -> Result<(), InstallError> {
    let id = model_id(id)?;
    if read_selection(dir).values().any(|v| v == id) {
        return Err(invalid("a slot selects this model; empty the slot first"));
    }
    let path = library(dir).join(id);
    if !path.is_dir() {
        return Err(invalid("unknown library model"));
    }
    remove_dir(&path)?;
    Ok(())
}

/// Removes staging directories and downloads of an earlier process.
pub fn clean(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(library(dir)) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with('.') {
            let path = entry.path();
            let _ = if path.is_dir() {
                remove_dir(&path)
            } else {
                std::fs::remove_file(&path)
            };
        }
    }
}

pub fn overview(
    dir: &Path,
    catalog: &Catalog,
    loaded: &[LoadedModel],
    limits: &Limits,
) -> Overview {
    let selection = read_selection(dir);
    let slots = SLOTS
        .into_iter()
        .map(|slot| {
            let model_id = selection.get(slot).cloned();
            let running = loaded.iter().find(|m| m.slot == slot);
            let ready = running.filter(|m| m.state == "ready");
            SlotStatus {
                slot,
                name: model_id
                    .as_deref()
                    .and_then(|id| manifest(dir, id))
                    .map(|m| m.name),
                active: model_id.is_some() && ready.map(|m| &m.id) == model_id.as_ref(),
                loaded: ready.is_some(),
                error: running
                    .filter(|m| Some(&m.id) == model_id.as_ref())
                    .and_then(|m| m.error.clone())
                    .or_else(|| catalog.errors.get(slot).cloned()),
                vram_mib: ready.and_then(|m| m.vram_mib),
                model_id,
            }
        })
        .collect();
    let library = library_ids(dir)
        .into_iter()
        .filter_map(|id| {
            let manifest = manifest(dir, &id)?;
            Some(LibraryModel {
                bytes: dir_bytes(&library(dir).join(&id)),
                slots: selection
                    .iter()
                    .filter(|(_, v)| **v == id)
                    .map(|(k, _)| k.clone())
                    .collect(),
                id,
                manifest,
            })
        })
        .collect();
    Overview {
        slots,
        library,
        usage: usage(dir, limits),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A tar.gz archive with the given files under one top directory.
    pub fn archive(path: &Path, files: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let gz = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        for (name, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, format!("./model-dir/{name}"), *data)
                .unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
    }

    pub const ONNX: &[u8] = b"\x08\x07fake-onnx";

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cuda-library-{name}-{}", std::process::id()));
        let _ = remove_dir(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn install(family: &str, languages: &str) -> Install {
        Install {
            name: "Test model".into(),
            family: family.into(),
            languages: languages.into(),
            license: "CC-BY-4.0".into(),
            source: "https://example.org/model".into(),
            sha256: None,
            precision: Precision::Full,
        }
    }

    #[test]
    fn archive_is_unpacked_checked_and_selected() {
        let dir = temp("add");
        let file = dir.join("model.tar.gz");
        archive(
            &file,
            &[
                ("encoder.int8.onnx", b"\x08\x07int8"),
                ("encoder.onnx", ONNX),
                ("decoder.onnx", ONNX),
                ("joiner.onnx", ONNX),
                ("tokens.txt", b"a 0\n"),
                ("test_wavs/de.wav", b"RIFF"),
            ],
        );
        let limits = Limits::default();
        let model = add_archive(
            &dir,
            &file,
            install("nemo_transducer", "de, en"),
            &limits,
            &ModelCheck::Files,
        )
        .unwrap();
        assert_eq!(model.manifest.files["encoder"], "encoder.onnx");
        assert_eq!(model.manifest.languages, ["de", "en"]);
        let stored: Vec<String> = std::fs::read_dir(library(&dir).join(&model.id))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        assert_eq!(stored.len(), 5, "4 model files and model.json: {stored:?}");
        // The same files again are rejected.
        let again = add_archive(
            &dir,
            &file,
            install("nemo_transducer", "de"),
            &limits,
            &ModelCheck::Files,
        );
        assert!(again.unwrap_err().to_string().contains("already"));
        // int8 preference keeps another file set (another id).
        let mut int8 = install("nemo_transducer", "de");
        int8.precision = Precision::Int8;
        let small = add_archive(&dir, &file, int8, &limits, &ModelCheck::Files).unwrap();
        assert_eq!(small.manifest.files["encoder"], "encoder.int8.onnx");
        assert_ne!(small.id, model.id);

        select(&dir, "de", &model.id).unwrap();
        select(&dir, "en", &model.id).unwrap();
        assert!(select(&dir, "langid", &model.id).is_err(), "not Whisper");
        assert!(select(&dir, "xx", &model.id).is_err());
        assert!(delete(&dir, &model.id).is_err(), "selected");
        let catalog = Catalog::load(&dir);
        assert_eq!(catalog.get("en").unwrap().id, model.id);
        let o = overview(&dir, &catalog, &[], &limits);
        assert_eq!(o.library.len(), 2);
        assert_eq!(o.slots[0].model_id.as_ref(), Some(&model.id));
        assert!(!o.slots[0].active, "not loaded by a running service");
        clear(&dir, "de").unwrap();
        clear(&dir, "en").unwrap();
        delete(&dir, &model.id).unwrap();
        assert_eq!(usage(&dir, &limits).models, 1);
        remove_dir(&dir).unwrap();
    }

    #[test]
    fn invalid_archives_change_nothing() {
        let dir = temp("invalid");
        let file = dir.join("a.tar.gz");
        let limits = Limits::default();
        archive(&file, &[("model.onnx", ONNX)]);
        let err = add_archive(
            &dir,
            &file,
            install("nemo_ctc", "de"),
            &limits,
            &ModelCheck::Files,
        );
        assert!(err.unwrap_err().to_string().contains("tokens"));
        archive(
            &file,
            &[("model.onnx", b"not onnx"), ("tokens.txt", b"a 0\n")],
        );
        let err = add_archive(
            &dir,
            &file,
            install("nemo_ctc", "de"),
            &limits,
            &ModelCheck::Files,
        );
        assert!(err.unwrap_err().to_string().contains("ONNX"));
        archive(&file, &[("model.onnx", ONNX), ("tokens.txt", b"a 0\n")]);
        for (family, languages) in [("tflite", "de"), ("nemo_ctc", "german"), ("nemo_ctc", "")] {
            assert!(
                add_archive(
                    &dir,
                    &file,
                    install(family, languages),
                    &limits,
                    &ModelCheck::Files
                )
                .is_err()
            );
        }
        let mut wrong = install("nemo_ctc", "de");
        wrong.sha256 = Some("00".repeat(32));
        assert!(add_archive(&dir, &file, wrong, &limits, &ModelCheck::Files).is_err());
        let tiny = Limits {
            max_model_bytes: 4,
            ..limits
        };
        assert!(
            add_archive(
                &dir,
                &file,
                install("nemo_ctc", "de"),
                &tiny,
                &ModelCheck::Files
            )
            .is_err()
        );
        assert_eq!(library_ids(&dir).len(), 0);
        assert_eq!(
            std::fs::read_dir(library(&dir)).unwrap().count(),
            0,
            "no staging directory is left"
        );
        remove_dir(&dir).unwrap();
    }

    #[test]
    fn unsafe_paths_and_links_are_rejected() {
        let dir = temp("unsafe");
        let file = dir.join("evil.tar");
        {
            let mut tar = tar::Builder::new(std::fs::File::create(&file).unwrap());
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_cksum();
            tar.append_link(&mut header, "model.onnx", "/etc/passwd")
                .unwrap();
            tar.finish().unwrap();
        }
        let err = add_archive(
            &dir,
            &file,
            install("nemo_ctc", "de"),
            &Limits::default(),
            &ModelCheck::Files,
        );
        assert!(err.unwrap_err().to_string().contains("links"));
        remove_dir(&dir).unwrap();
    }

    #[test]
    fn roles_pick_the_expected_files() {
        let names: Vec<String> = [
            "tiny-encoder.onnx",
            "tiny-encoder.int8.onnx",
            "tiny-decoder.int8.onnx",
            "tiny-tokens.txt",
        ]
        .map(str::to_owned)
        .to_vec();
        assert_eq!(
            pick(&names, "encoder", Precision::Full).unwrap(),
            "tiny-encoder.onnx"
        );
        assert_eq!(
            pick(&names, "decoder", Precision::Full).unwrap(),
            "tiny-decoder.int8.onnx"
        );
        assert_eq!(
            pick(&names, "tokens", Precision::Full).unwrap(),
            "tiny-tokens.txt"
        );
        assert_eq!(pick(&names, "model", Precision::Full), None);
    }
}
