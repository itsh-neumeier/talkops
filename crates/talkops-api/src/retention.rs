//! Deletes call recordings (file, row and transcript) once they are older
//! than their tenant's retention period.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::PgPool;
use talkops_core::error::CoreResult;
use talkops_core::recordings;

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

pub fn spawn(db: PgPool, dir: PathBuf) {
    tokio::spawn(async move {
        loop {
            loop {
                match purge(&db, &dir).await {
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
            tokio::time::sleep(INTERVAL).await;
        }
    });
}
