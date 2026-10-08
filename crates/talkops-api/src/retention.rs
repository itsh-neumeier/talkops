//! Deletes call recordings (file, row and transcript) and door events (with
//! snapshots) once they are older than their tenant's retention period.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use talkops_core::error::CoreResult;
use talkops_core::recordings;

use crate::MediaPaths;

const INTERVAL: Duration = Duration::from_secs(3600);
const BATCH: i64 = 500;

/// Deletes one batch of expired recordings; returns how many.
pub async fn purge(db: &PgPool, dir: &Path) -> CoreResult<usize> {
    let expired = recordings::expired(db, BATCH).await?;
    for (tenant, rec) in &expired {
        recordings::delete(db, *tenant, rec.id).await?;
        match tokio::fs::remove_file(dir.join(&rec.file)).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => tracing::warn!(error = %err, file = %rec.file, "cannot remove recording"),
        }
    }
    Ok(expired.len())
}

/// Deletes audio clips nobody uses any more (previews, replaced greetings).
pub async fn purge_clips(db: &PgPool, sounds: &Path) -> CoreResult<usize> {
    let gone = talkops_core::audio::purge_unused(db).await?;
    for (tenant, id) in &gone {
        let path = sounds.join(talkops_core::audio::clip_file(*tenant, *id));
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => tracing::warn!(error = %err, file = %path.display(), "cannot remove clip"),
        }
    }
    Ok(gone.len())
}

pub fn spawn(db: PgPool, media: Arc<MediaPaths>) {
    tokio::spawn(async move {
        loop {
            loop {
                match purge(&db, &media.recordings).await {
                    Ok(0) => break,
                    Ok(n) => {
                        tracing::info!(count = n, "expired recordings deleted");
                        if (n as i64) < BATCH {
                            break;
                        }
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "recording retention failed");
                        break;
                    }
                }
            }
            match crate::doors::purge(&db, &media.snapshots).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(count = n, "expired door events deleted"),
                Err(err) => tracing::warn!(error = %err, "door event retention failed"),
            }
            match purge_clips(&db, &media.sounds).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(count = n, "unused audio clips deleted"),
                Err(err) => tracing::warn!(error = %err, "audio clip cleanup failed"),
            }
            tokio::time::sleep(INTERVAL).await;
        }
    });
}
