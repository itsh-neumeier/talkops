//! Storage of the mail branding and the own mail texts (ADR 0034).

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use crate::error::{CoreError, CoreResult};
use crate::mail_render::{self, Design, InstanceInfo, LogoSize, MailKind};
use crate::tenant::TenantId;

/// Largest accepted logo file.
pub const MAX_LOGO_BYTES: usize = 256 * 1024;
pub const MAX_INSTANCE_NAME: usize = 60;
pub const MAX_SENDER_NAME: usize = 80;
pub const MAX_HEADER: usize = 200;
pub const MAX_FOOTER: usize = 1000;

/// Branding as shown in the UI (without the logo bytes).
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct MailBranding {
    /// Instance name (system setting), subject prefix `[name]`.
    pub instance_name: String,
    /// Display name of the sender; empty: the SMTP sender as entered.
    pub sender_name: String,
    pub accent_color: String,
    pub header_text: String,
    pub footer_text: String,
    pub has_logo: bool,
    pub logo_type: Option<String>,
    pub logo_width: Option<i32>,
    pub logo_height: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct BrandingInput {
    #[serde(default)]
    pub instance_name: String,
    #[serde(default)]
    pub sender_name: String,
    pub accent_color: String,
    #[serde(default)]
    pub header_text: String,
    #[serde(default)]
    pub footer_text: String,
}

/// The stored logo.
#[derive(Debug, Clone)]
pub struct Logo {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub size: LogoSize,
}

const BRANDING_SQL: &str = "SELECT s.instance_name,
        COALESCE(b.sender_name, '') AS sender_name,
        COALESCE(b.accent_color, '#0d9488') AS accent_color,
        COALESCE(b.header_text, '') AS header_text,
        COALESCE(b.footer_text, '') AS footer_text,
        b.logo IS NOT NULL AS has_logo, b.logo_type, b.logo_width, b.logo_height
     FROM tenant_settings s LEFT JOIN mail_branding b ON b.tenant_id = s.tenant_id
     WHERE s.tenant_id = $1";

pub async fn branding(pool: &PgPool, tenant: TenantId) -> CoreResult<MailBranding> {
    Ok(sqlx::query_as(BRANDING_SQL)
        .bind(tenant)
        .fetch_one(pool)
        .await?)
}

fn single_line(field: &str, value: &str, max: usize) -> CoreResult<()> {
    if value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(CoreError::Validation(format!(
            "{field}: at most {max} characters on one line"
        )));
    }
    Ok(())
}

pub async fn update_branding(
    pool: &PgPool,
    tenant: TenantId,
    input: &BrandingInput,
) -> CoreResult<MailBranding> {
    let instance_name = input.instance_name.trim();
    let sender_name = input.sender_name.trim();
    single_line("instance name", instance_name, MAX_INSTANCE_NAME)?;
    single_line("sender name", sender_name, MAX_SENDER_NAME)?;
    if sender_name.contains(['"', '<', '>', '@']) {
        return Err(CoreError::Validation(
            "sender name: no quotes, angle brackets or @".into(),
        ));
    }
    let accent = mail_render::normalize_color(&input.accent_color)
        .ok_or_else(|| CoreError::Validation("accent color must be #rrggbb".into()))?;
    let header = input.header_text.trim();
    let footer = input.footer_text.trim();
    mail_render::validate_fragment("header text", header, MAX_HEADER)
        .map_err(|e| CoreError::Validation(e.to_string()))?;
    mail_render::validate_fragment("footer text", footer, MAX_FOOTER)
        .map_err(|e| CoreError::Validation(e.to_string()))?;

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE tenant_settings SET instance_name = $2, updated_at = now() WHERE tenant_id = $1",
    )
    .bind(tenant)
    .bind(instance_name)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO mail_branding (tenant_id, sender_name, accent_color, header_text, footer_text)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (tenant_id) DO UPDATE SET sender_name = $2, accent_color = $3,
             header_text = $4, footer_text = $5, updated_at = now()",
    )
    .bind(tenant)
    .bind(sender_name)
    .bind(&accent)
    .bind(header)
    .bind(footer)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    branding(pool, tenant).await
}

/// Type and pixel size of a PNG, JPEG or GIF image; `None` for other data.
pub fn sniff_image(data: &[u8]) -> Option<(&'static str, LogoSize)> {
    let be16 = |i: usize| -> Option<u32> {
        Some(u32::from(u16::from_be_bytes([
            *data.get(i)?,
            *data.get(i + 1)?,
        ])))
    };
    if data.starts_with(b"\x89PNG\r\n\x1a\n") && data.get(12..16) == Some(b"IHDR") {
        let w = u32::from_be_bytes(data.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(data.get(20..24)?.try_into().ok()?);
        return Some((
            "image/png",
            LogoSize {
                width: w,
                height: h,
            },
        ));
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        let w = u32::from(u16::from_le_bytes([*data.get(6)?, *data.get(7)?]));
        let h = u32::from(u16::from_le_bytes([*data.get(8)?, *data.get(9)?]));
        return Some((
            "image/gif",
            LogoSize {
                width: w,
                height: h,
            },
        ));
    }
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 3 < data.len() {
            if data[i] != 0xFF {
                return None;
            }
            let marker = data[i + 1];
            if marker == 0xFF {
                i += 1;
                continue;
            }
            if marker == 0x01 || (0xD0..=0xD9).contains(&marker) {
                i += 2;
                continue;
            }
            let len = be16(i + 2)? as usize;
            let sof = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
            if sof {
                let h = be16(i + 5)?;
                let w = be16(i + 7)?;
                return Some((
                    "image/jpeg",
                    LogoSize {
                        width: w,
                        height: h,
                    },
                ));
            }
            i += 2 + len;
        }
    }
    None
}

/// Stores a logo after a check of type and size.
pub async fn set_logo(pool: &PgPool, tenant: TenantId, data: &[u8]) -> CoreResult<MailBranding> {
    if data.len() > MAX_LOGO_BYTES {
        return Err(CoreError::Validation(format!(
            "logo: at most {} KB",
            MAX_LOGO_BYTES / 1024
        )));
    }
    let (content_type, size) = sniff_image(data)
        .ok_or_else(|| CoreError::Validation("logo must be a PNG, JPEG or GIF image".into()))?;
    if !(1..=4096).contains(&size.width) || !(1..=4096).contains(&size.height) {
        return Err(CoreError::Validation(
            "logo: at most 4096 × 4096 pixels".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO mail_branding (tenant_id, logo, logo_type, logo_width, logo_height)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (tenant_id) DO UPDATE SET logo = $2, logo_type = $3, logo_width = $4,
             logo_height = $5, updated_at = now()",
    )
    .bind(tenant)
    .bind(data)
    .bind(content_type)
    .bind(size.width as i32)
    .bind(size.height as i32)
    .execute(pool)
    .await?;
    branding(pool, tenant).await
}

pub async fn delete_logo(pool: &PgPool, tenant: TenantId) -> CoreResult<MailBranding> {
    sqlx::query(
        "UPDATE mail_branding SET logo = NULL, logo_type = NULL, logo_width = NULL,
             logo_height = NULL, updated_at = now()
         WHERE tenant_id = $1",
    )
    .bind(tenant)
    .execute(pool)
    .await?;
    branding(pool, tenant).await
}

pub async fn logo(pool: &PgPool, tenant: TenantId) -> CoreResult<Option<Logo>> {
    let row: Option<(Vec<u8>, String, i32, i32)> = sqlx::query_as(
        "SELECT logo, logo_type, logo_width, logo_height FROM mail_branding
         WHERE tenant_id = $1 AND logo IS NOT NULL",
    )
    .bind(tenant)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(bytes, content_type, w, h)| Logo {
        bytes,
        content_type,
        size: LogoSize {
            width: w.max(1) as u32,
            height: h.max(1) as u32,
        },
    }))
}

/// Branding, instance information and logo for rendering. `public_url` is
/// the URL of the web interface (identity settings).
pub async fn design(pool: &PgPool, tenant: TenantId) -> CoreResult<(Design, Option<Logo>)> {
    let b = branding(pool, tenant).await?;
    let public_url: Option<String> =
        sqlx::query_scalar("SELECT public_url FROM identity_settings WHERE tenant_id = $1")
            .bind(tenant)
            .fetch_optional(pool)
            .await?;
    let logo = logo(pool, tenant).await?;
    let design = Design {
        sender_name: b.sender_name,
        accent_color: b.accent_color,
        header_text: b.header_text,
        footer_text: b.footer_text,
        logo: logo.as_ref().map(|l| l.size),
        logo_src: if logo.is_some() {
            format!("cid:{}", mail_render::LOGO_CID)
        } else {
            String::new()
        },
        instance: InstanceInfo::new(&b.instance_name, public_url.as_deref().unwrap_or("")),
    };
    Ok((design, logo))
}

/// Subject and body of a mail type in one language.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TemplateText {
    pub subject: String,
    pub body: String,
}

/// A mail text with its default, for the editor.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TemplateEntry {
    pub kind: String,
    pub language: String,
    /// The text in use (own text or default).
    pub subject: String,
    pub body: String,
    /// True if an own text is stored.
    pub custom: bool,
    pub default_subject: String,
    pub default_body: String,
}

fn check_language(lang: &str) -> CoreResult<usize> {
    mail_render::LANGUAGES
        .iter()
        .position(|l| *l == lang)
        .ok_or_else(|| CoreError::Validation("language must be de or en".into()))
}

/// The text of a mail type in a language: the own text or the default.
pub async fn template(
    pool: &PgPool,
    tenant: TenantId,
    kind: &MailKind,
    lang: &str,
) -> CoreResult<TemplateText> {
    let i = mail_render::lang_index(lang);
    let lang = mail_render::LANGUAGES[i];
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT subject, body FROM mail_templates WHERE tenant_id = $1 AND kind = $2 AND language = $3",
    )
    .bind(tenant)
    .bind(kind.key)
    .bind(lang)
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        Some((subject, body)) => TemplateText { subject, body },
        None => TemplateText {
            subject: kind.subject[i].to_owned(),
            body: kind.body[i].to_owned(),
        },
    })
}

/// All mail texts of all registered types and languages.
pub async fn list_templates(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<TemplateEntry>> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT kind, language, subject, body FROM mail_templates WHERE tenant_id = $1",
    )
    .bind(tenant)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for kind in mail_render::KINDS {
        for (i, lang) in mail_render::LANGUAGES.iter().enumerate() {
            let own = rows.iter().find(|r| r.0 == kind.key && r.1 == *lang);
            out.push(TemplateEntry {
                kind: kind.key.to_owned(),
                language: (*lang).to_owned(),
                subject: own.map_or(kind.subject[i], |r| r.2.as_str()).to_owned(),
                body: own.map_or(kind.body[i], |r| r.3.as_str()).to_owned(),
                custom: own.is_some(),
                default_subject: kind.subject[i].to_owned(),
                default_body: kind.body[i].to_owned(),
            });
        }
    }
    Ok(out)
}

/// Validates and stores an own text.
pub async fn save_template(
    pool: &PgPool,
    tenant: TenantId,
    kind: &MailKind,
    lang: &str,
    text: &TemplateText,
) -> CoreResult<()> {
    check_language(lang)?;
    mail_render::validate(kind, &text.subject, &text.body)
        .map_err(|e| CoreError::Validation(e.to_string()))?;
    sqlx::query(
        "INSERT INTO mail_templates (tenant_id, kind, language, subject, body)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (tenant_id, kind, language) DO UPDATE SET subject = $4, body = $5,
             updated_at = now()",
    )
    .bind(tenant)
    .bind(kind.key)
    .bind(lang)
    .bind(text.subject.trim())
    .bind(text.body.trim_end())
    .execute(pool)
    .await?;
    Ok(())
}

/// Removes an own text; the default applies again.
pub async fn reset_template(
    pool: &PgPool,
    tenant: TenantId,
    kind: &MailKind,
    lang: &str,
) -> CoreResult<()> {
    check_language(lang)?;
    sqlx::query("DELETE FROM mail_templates WHERE tenant_id = $1 AND kind = $2 AND language = $3")
        .bind(tenant)
        .bind(kind.key)
        .bind(lang)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_images() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&300u32.to_be_bytes());
        png.extend_from_slice(&80u32.to_be_bytes());
        assert_eq!(
            sniff_image(&png),
            Some((
                "image/png",
                LogoSize {
                    width: 300,
                    height: 80
                }
            ))
        );
        let gif = b"GIF89a\x10\x00\x20\x00rest";
        assert_eq!(
            sniff_image(gif),
            Some((
                "image/gif",
                LogoSize {
                    width: 16,
                    height: 32
                }
            ))
        );
        // SOI, APP0 (length 4), SOF0 with height 0x0102 and width 0x0304.
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x01,
            0x02, 0x03, 0x04, 0x03,
        ];
        assert_eq!(
            sniff_image(&jpeg),
            Some((
                "image/jpeg",
                LogoSize {
                    width: 0x0304,
                    height: 0x0102
                }
            ))
        );
        assert_eq!(sniff_image(b"<svg xmlns=...>"), None);
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0x00]), None);
    }
}
