//! Admin-edited provisioning files (ADR 0052). A file with the name of a
//! generated file replaces it; another name adds a file. Contents are stored
//! encrypted: a copy of a generated file can contain SIP secrets.
use crate::{
    crypto::SecretBox,
    error::{CoreError, CoreResult},
    tenant::TenantId,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Largest file in bytes.
pub const MAX_SIZE: usize = 1024 * 1024;
/// Most files per tenant.
pub const MAX_FILES: i64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Http,
    Tftp,
}

impl Transport {
    pub fn as_str(self) -> &'static str {
        match self {
            Transport::Http => "http",
            Transport::Tftp => "tftp",
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct FileMeta {
    pub id: Uuid,
    pub transport: String,
    pub phone_id: Option<Uuid>,
    pub path: String,
    pub size_bytes: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct Stored {
    #[sqlx(flatten)]
    meta: FileMeta,
    content: String,
}

/// Checks a path for its transport. HTTP paths are relative paths below
/// `/provisioning/` outside the routes that TalkOps serves without a file
/// (firmware, media, events, Cisco directory). TFTP names are the root names
/// that the TFTP sidecar map serves (`docker/talkops/cisco-tftp.map`).
pub fn valid_path(transport: Transport, path: &str) -> bool {
    let name_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.');
    match transport {
        Transport::Http => {
            path.len() <= 200
                && !["firmware/", "media/", "cisco/", "events"]
                    .iter()
                    .any(|p| path.starts_with(p))
                && path.split('/').count() <= 4
                && path.split('/').all(|s| {
                    !s.is_empty()
                        && s.len() <= 100
                        && !s.starts_with('.')
                        && !s.contains("..")
                        && s.chars().all(name_char)
                })
        }
        Transport::Tftp => {
            if let Some(mac) = path
                .strip_prefix("SEP")
                .and_then(|p| p.strip_suffix(".cnf.xml"))
            {
                return mac.len() == 12
                    && mac
                        .chars()
                        .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c));
            }
            let Some((stem, ext)) = path.rsplit_once('.') else {
                return false;
            };
            let stem = stem.strip_suffix(".utf-8").unwrap_or(stem);
            matches!(ext, "jar" | "xml" | "dat")
                && (1..=63).contains(&stem.len())
                && stem.starts_with(|c: char| c.is_ascii_alphanumeric())
                && stem
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        }
    }
}

const COLUMNS: &str = "id,transport,phone_id,path,size_bytes,updated_at";

pub async fn list(db: &PgPool, tenant: TenantId) -> CoreResult<Vec<FileMeta>> {
    Ok(sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM provisioning_files WHERE tenant_id=$1 \
         ORDER BY transport,phone_id NULLS FIRST,path"
    ))
    .bind(tenant)
    .fetch_all(db)
    .await?)
}

fn open(secrets: &SecretBox, stored: &str) -> CoreResult<Vec<u8>> {
    STANDARD
        .decode(secrets.decrypt(stored)?)
        .map_err(|_| CoreError::Validation("stored provisioning file is malformed".into()))
}

/// One file with its content.
pub async fn content(
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<(FileMeta, Vec<u8>)> {
    let row: Stored = sqlx::query_as(&format!(
        "SELECT {COLUMNS},content FROM provisioning_files WHERE tenant_id=$1 AND id=$2"
    ))
    .bind(tenant)
    .bind(id)
    .fetch_one(db)
    .await?;
    let content = open(secrets, &row.content)?;
    Ok((row.meta, content))
}

/// The HTTP file at `path`, if an admin stored one.
pub async fn http_file(
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
    path: &str,
) -> CoreResult<Option<Vec<u8>>> {
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT content FROM provisioning_files WHERE tenant_id=$1 AND transport='http' AND path=$2",
    )
    .bind(tenant)
    .bind(path)
    .fetch_optional(db)
    .await?;
    stored.map(|s| open(secrets, &s)).transpose()
}

/// TFTP files of one phone: the files for every phone first, then the
/// phone's own files, so that a later entry with the same name wins.
pub async fn tftp_files(
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
    phone: Uuid,
) -> CoreResult<Vec<(String, Vec<u8>)>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT path,content FROM provisioning_files WHERE tenant_id=$1 AND transport='tftp' \
         AND (phone_id IS NULL OR phone_id=$2) ORDER BY phone_id NULLS FIRST,path",
    )
    .bind(tenant)
    .bind(phone)
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|(path, stored)| Ok((path, open(secrets, &stored)?)))
        .collect()
}

/// Creates or replaces the file with this transport, phone and path.
pub async fn put(
    db: &PgPool,
    secrets: &SecretBox,
    tenant: TenantId,
    transport: Transport,
    phone: Option<Uuid>,
    path: &str,
    content: &[u8],
) -> CoreResult<FileMeta> {
    if !valid_path(transport, path) {
        return Err(CoreError::Validation(
            "invalid file name for this transport".into(),
        ));
    }
    if transport == Transport::Http && phone.is_some() {
        return Err(CoreError::Validation(
            "HTTP files apply to every phone".into(),
        ));
    }
    if content.len() > MAX_SIZE {
        return Err(CoreError::Validation("file is larger than 1 MiB".into()));
    }
    let stored = secrets.encrypt(&STANDARD.encode(content))?;
    let size = i32::try_from(content.len()).unwrap_or(i32::MAX);
    let mut tx = db.begin().await?;
    // Serializes the writers of a tenant for the count check.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('provisioning_files'), hashtext($1::text))")
        .bind(tenant)
        .execute(&mut *tx)
        .await?;
    if let Some(phone) = phone {
        sqlx::query("SELECT 1 FROM phones WHERE tenant_id=$1 AND id=$2")
            .bind(tenant)
            .bind(phone)
            .fetch_one(&mut *tx)
            .await?;
    }
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM provisioning_files WHERE tenant_id=$1 AND transport=$2 \
         AND phone_id IS NOT DISTINCT FROM $3 AND path=$4",
    )
    .bind(tenant)
    .bind(transport.as_str())
    .bind(phone)
    .bind(path)
    .fetch_optional(&mut *tx)
    .await?;
    let meta: FileMeta = match existing {
        Some(id) => {
            sqlx::query_as(&format!(
                "UPDATE provisioning_files SET content=$3,size_bytes=$4,updated_at=now() \
                 WHERE tenant_id=$1 AND id=$2 RETURNING {COLUMNS}"
            ))
            .bind(tenant)
            .bind(id)
            .bind(stored)
            .bind(size)
            .fetch_one(&mut *tx)
            .await?
        }
        None => {
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM provisioning_files WHERE tenant_id=$1")
                    .bind(tenant)
                    .fetch_one(&mut *tx)
                    .await?;
            if count >= MAX_FILES {
                return Err(CoreError::Validation("too many provisioning files".into()));
            }
            sqlx::query_as(&format!(
                "INSERT INTO provisioning_files \
                 (tenant_id,transport,phone_id,path,content,size_bytes) \
                 VALUES ($1,$2,$3,$4,$5,$6) RETURNING {COLUMNS}"
            ))
            .bind(tenant)
            .bind(transport.as_str())
            .bind(phone)
            .bind(path)
            .bind(stored)
            .bind(size)
            .fetch_one(&mut *tx)
            .await?
        }
    };
    tx.commit().await?;
    Ok(meta)
}

pub async fn delete(db: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<FileMeta> {
    Ok(sqlx::query_as(&format!(
        "DELETE FROM provisioning_files WHERE tenant_id=$1 AND id=$2 RETURNING {COLUMNS}"
    ))
    .bind(tenant)
    .bind(id)
    .fetch_one(db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_per_transport() {
        for ok in [
            "805ec0010203.cfg",
            "custom/extra.xml",
            "y000000000044.cfg",
            "devices/a/b.xml",
        ] {
            assert!(valid_path(Transport::Http, ok), "{ok}");
        }
        for bad in [
            "",
            "../x",
            "a/../b",
            ".hidden",
            "firmware/x.rom",
            "media/1",
            "events",
            "cisco/directory/x",
            "a//b",
            "a/b/c/d/e",
            "a b",
            "x?y",
        ] {
            assert!(!valid_path(Transport::Http, bad), "{bad}");
        }
        for ok in [
            "g3-tones.xml",
            "SEP00451D6BE327.cnf.xml",
            "dialplan.xml",
            "Ringlist-wb.xml",
            "sp-sip.jar",
            "7921-kate.utf-8.xml",
            "7921-font.dat",
        ] {
            assert!(valid_path(Transport::Tftp, ok), "{ok}");
        }
        for bad in [
            "",
            "a.txt",
            "German_Germany/sp-sip.jar",
            "../x.xml",
            "a..b.xml",
            ".x.xml",
            "x.cnf.xml",
            "SEP00451d6be327.cnf.xml",
            "talkops-0123abcd.rwb",
            "ITLFile.tlv",
        ] {
            assert!(!valid_path(Transport::Tftp, bad), "{bad}");
        }
    }
}
