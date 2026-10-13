//! Call records from FreeSWITCH: ingestion of `mod_xml_cdr` posts and of the
//! records it could not deliver.
//!
//! When TalkOps does not accept a record (restart, database down),
//! `mod_xml_cdr` writes it to its `err-log-dir`. That directory is the shared
//! spool volume; TalkOps imports the files from it periodically. Records are
//! keyed by call UUID, so a record that arrives twice is stored once.

use std::path::{Path, PathBuf};
use std::time::Duration;

use talkops_core::error::CoreError;
use talkops_core::tenant::TenantId;

use crate::AppState;
use crate::fsxml;

/// Interval between spool scans.
const SCAN_INTERVAL: Duration = Duration::from_secs(60);
/// Files younger than this can still be in the middle of a write.
const MIN_AGE: Duration = Duration::from_secs(10);
/// Subdirectory for files that cannot be parsed; kept for manual inspection.
pub const REJECTED_DIR: &str = "rejected";

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Stored,
    /// A record with this call UUID exists already.
    Duplicate,
    /// Call not routed by TalkOps (no `talkops_direction`), e.g. a scanner.
    Unrouted,
    Malformed(String),
}

/// Parses a record and stores it with its recording. `Err` means the
/// database failed and the record must be delivered again.
pub async fn ingest(state: &AppState, xml: &str) -> Result<Outcome, CoreError> {
    let (tenant, cdr, recording) = match fsxml::cdr::parse(xml) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => {
            tracing::debug!("call record without TalkOps routing ignored");
            return Ok(Outcome::Unrouted);
        }
        Err(err) => return Ok(Outcome::Malformed(err)),
    };
    if !talkops_core::cdr::insert(&state.db, tenant, &cdr).await? {
        tracing::debug!(call_uuid = %cdr.call_uuid, "call record already stored");
        return Ok(Outcome::Duplicate);
    }
    tracing::info!(
        call_uuid = %cdr.call_uuid,
        direction = ?cdr.direction,
        duration_secs = cdr.duration_secs,
        billsec = cdr.billsec,
        hangup_cause = %cdr.hangup_cause,
        "call record stored"
    );
    if let Some(file) = recording {
        store_recording(state, tenant, &cdr.call_uuid, &file).await;
    }
    Ok(Outcome::Stored)
}

async fn store_recording(state: &AppState, tenant: TenantId, call_uuid: &str, file: &str) {
    if file.split('/').any(|p| p == ".." || p.is_empty()) || !file.ends_with(".wav") {
        tracing::warn!(file, "ignoring invalid recording path");
        return;
    }
    let path = state.media.recordings.join(file);
    let size = match tokio::fs::metadata(&path).await {
        Ok(m) => m.len() as i64,
        Err(_) => {
            // Unanswered calls leave no file.
            return;
        }
    };
    let secs = crate::voicemail::ivr::wav_seconds(&path).unwrap_or(0) as i32;
    if secs < 1 {
        let _ = tokio::fs::remove_file(&path).await;
        return;
    }
    let transcribe = talkops_core::settings::get(&state.db, tenant)
        .await
        .map(|s| s.transcription_enabled)
        .unwrap_or(false);
    if let Err(err) =
        talkops_core::recordings::create(&state.db, tenant, call_uuid, file, secs, size, transcribe)
            .await
    {
        tracing::error!(error = %err, "cannot store recording");
    }
}

/// Result of one spool scan.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Imported {
    /// Files imported or found to be already stored; removed from the spool.
    pub done: usize,
    /// Files moved to [`REJECTED_DIR`].
    pub rejected: usize,
}

/// Imports the records in `dir` that are at least `min_age` old. Stops at
/// the first database error; the remaining files stay for the next scan.
pub async fn import_dir(
    state: &AppState,
    dir: &Path,
    min_age: Duration,
) -> std::io::Result<Imported> {
    let mut imported = Imported::default();
    let mut entries = tokio::fs::read_dir(dir).await?;
    let mut files = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let is_record = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".xml"));
        let Ok(meta) = entry.metadata().await else {
            continue;
        };
        let old_enough = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|age| age >= min_age);
        if is_record && meta.is_file() && old_enough {
            files.push(path);
        }
    }
    files.sort();
    for path in files {
        let body = tokio::fs::read(&path).await?;
        let xml = spool_xml(&String::from_utf8_lossy(&body));
        match ingest(state, &xml).await {
            Ok(Outcome::Malformed(err)) => {
                tracing::warn!(file = %path.display(), error = %err, "malformed call record moved aside");
                let rejected = dir.join(REJECTED_DIR);
                tokio::fs::create_dir_all(&rejected).await?;
                if let Some(name) = path.file_name() {
                    tokio::fs::rename(&path, rejected.join(name)).await?;
                }
                imported.rejected += 1;
            }
            Ok(_) => {
                tokio::fs::remove_file(&path).await?;
                imported.done += 1;
            }
            Err(err) => {
                tracing::warn!(error = %err, "cannot import spooled call records; retrying later");
                break;
            }
        }
    }
    Ok(imported)
}

/// Spool files hold the plain XML document; accept the URL-encoded POST
/// body (`cdr=…`) as well.
fn spool_xml(content: &str) -> String {
    let content = content.trim_start();
    if content.starts_with('<') {
        return content.to_owned();
    }
    url::form_urlencoded::parse(content.as_bytes())
        .find(|(key, _)| key == "cdr")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| content.to_owned())
}

/// Scans `dir` every minute.
pub fn spawn(state: AppState, dir: PathBuf) {
    tokio::spawn(async move {
        let mut missing_logged = false;
        loop {
            match import_dir(&state, &dir, MIN_AGE).await {
                Ok(Imported {
                    done: 0,
                    rejected: 0,
                }) => {}
                Ok(n) => tracing::info!(
                    imported = n.done,
                    rejected = n.rejected,
                    "spooled call records processed"
                ),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    if !missing_logged {
                        tracing::warn!(dir = %dir.display(), "call record spool directory missing; undelivered records cannot be imported");
                        missing_logged = true;
                    }
                }
                Err(err) => {
                    tracing::warn!(dir = %dir.display(), error = %err, "cannot read call record spool")
                }
            }
            tokio::time::sleep(SCAN_INTERVAL).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spool_content_forms() {
        assert_eq!(spool_xml("  <cdr/>"), "<cdr/>");
        assert_eq!(spool_xml("cdr=%3Ccdr%2F%3E"), "<cdr/>");
        assert_eq!(spool_xml("garbage"), "garbage");
    }
}
