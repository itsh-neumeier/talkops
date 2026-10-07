//! Two-factor login with TOTP (RFC 6238: HMAC-SHA1, 30 s, 6 digits) and
//! one-time recovery codes. Secrets are encrypted at rest; a code (and its
//! time step) is accepted only once.

use chrono::{Duration, Utc};
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto::{self, SecretBox};
use crate::error::{CoreError, CoreResult};

/// Seconds per time step.
pub const STEP: u64 = 30;
/// Accepted clock drift in steps (±30 s).
const DRIFT: i64 = 1;
/// Recovery codes generated at enrollment.
pub const RECOVERY_CODES: usize = 10;
/// Wrong codes allowed per login challenge.
const MAX_ATTEMPTS: i32 = 5;
/// Minutes to enter the code after the password.
const CHALLENGE_MINUTES: i64 = 5;
/// Issuer shown in authenticator apps.
pub const ISSUER: &str = "TalkOps";

/// HOTP value (RFC 4226) for one counter.
fn hotp(secret: &[u8], counter: u64) -> u32 {
    let mut mac =
        <Hmac<Sha1> as KeyInit>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(&counter.to_be_bytes());
    let hash = mac.finalize().into_bytes();
    let offset = (hash[hash.len() - 1] & 0x0f) as usize;
    let bin = u32::from_be_bytes([
        hash[offset] & 0x7f,
        hash[offset + 1],
        hash[offset + 2],
        hash[offset + 3],
    ]);
    bin % 1_000_000
}

/// Time step of a Unix time.
pub fn step_of(unix: u64) -> u64 {
    unix / STEP
}

/// Current TOTP code (tests, documentation examples).
pub fn code_at(secret: &[u8], unix: u64) -> String {
    format!("{:06}", hotp(secret, step_of(unix)))
}

/// Code of a base32 secret at a Unix time (tests and tooling).
pub fn code_for(secret_b32: &str, unix: u64) -> CoreResult<String> {
    Ok(code_at(&decode_secret(secret_b32)?, unix))
}

/// The matching time step for `code` around `unix`, if any. Steps at or
/// below `last_step` are rejected (replay).
pub fn verify(secret: &[u8], code: &str, unix: u64, last_step: u64) -> Option<u64> {
    let code = code.trim().replace(' ', "");
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value: u32 = code.parse().ok()?;
    let now = step_of(unix) as i64;
    (-DRIFT..=DRIFT)
        .map(|d| now + d)
        .filter(|s| *s > last_step as i64 && *s >= 0)
        .find(|s| hotp(secret, *s as u64) == value)
        .map(|s| s as u64)
}

/// A new random 160-bit secret, base32 encoded (as authenticator apps expect).
pub fn new_secret() -> CoreResult<String> {
    let bytes = crypto::random_bytes(20)?;
    Ok(data_encoding::BASE32_NOPAD.encode(&bytes))
}

fn decode_secret(b32: &str) -> CoreResult<Vec<u8>> {
    data_encoding::BASE32_NOPAD
        .decode(b32.as_bytes())
        .map_err(|_| CoreError::Validation("invalid TOTP secret".into()))
}

/// `otpauth://` URI for QR codes.
pub fn otpauth_uri(secret: &str, account: &str) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect::<String>()
    };
    format!(
        "otpauth://totp/{issuer}:{account}?secret={secret}&issuer={issuer}&algorithm=SHA1&digits=6&period={STEP}",
        issuer = enc(ISSUER),
        account = enc(account),
    )
}

fn now_unix() -> u64 {
    Utc::now().timestamp().max(0) as u64
}

/// Whether a user has 2FA enabled.
pub async fn enabled(pool: &PgPool, user: Uuid) -> CoreResult<bool> {
    Ok(
        sqlx::query_scalar("SELECT totp_secret_enc IS NOT NULL FROM users WHERE id = $1")
            .bind(user)
            .fetch_optional(pool)
            .await?
            .unwrap_or(false),
    )
}

/// Starts enrollment: stores a pending secret and returns it (base32).
pub async fn begin_enrollment(
    pool: &PgPool,
    secrets: &SecretBox,
    user: Uuid,
) -> CoreResult<String> {
    let secret = new_secret()?;
    sqlx::query("UPDATE users SET totp_pending_enc = $2 WHERE id = $1")
        .bind(user)
        .bind(secrets.encrypt(&secret)?)
        .execute(pool)
        .await?;
    Ok(secret)
}

/// Finishes enrollment with a valid code from the pending secret; returns
/// fresh recovery codes (shown once).
pub async fn confirm_enrollment(
    pool: &PgPool,
    secrets: &SecretBox,
    user: Uuid,
    code: &str,
) -> CoreResult<Vec<String>> {
    let pending: Option<Option<String>> =
        sqlx::query_scalar("SELECT totp_pending_enc FROM users WHERE id = $1")
            .bind(user)
            .fetch_optional(pool)
            .await?;
    let Some(pending) = pending.flatten() else {
        return Err(CoreError::Validation("start the setup first".into()));
    };
    let secret = decode_secret(&secrets.decrypt(&pending)?)?;
    let Some(step) = verify(&secret, code, now_unix(), 0) else {
        return Err(CoreError::Validation("the code is not valid".into()));
    };
    let codes: Vec<String> = (0..RECOVERY_CODES)
        .map(|_| recovery_code())
        .collect::<CoreResult<_>>()?;
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE users SET totp_secret_enc = totp_pending_enc, totp_pending_enc = NULL,
                          totp_last_step = $2 WHERE id = $1",
    )
    .bind(user)
    .bind(step as i64)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = $1")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    for c in &codes {
        sqlx::query("INSERT INTO user_recovery_codes (user_id, code_digest) VALUES ($1, $2)")
            .bind(user)
            .bind(crypto::token_digest(&normalize_recovery(c)))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(codes)
}

/// Turns 2FA off (own account with password, or an admin reset).
pub async fn disable(pool: &PgPool, user: Uuid) -> CoreResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE users SET totp_secret_enc = NULL, totp_pending_enc = NULL, totp_last_step = 0
         WHERE id = $1",
    )
    .bind(user)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = $1")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Unused recovery codes left.
pub async fn recovery_codes_left(pool: &PgPool, user: Uuid) -> CoreResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM user_recovery_codes WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user)
    .fetch_one(pool)
    .await?)
}

fn recovery_code() -> CoreResult<String> {
    // 10 base32 characters (50 bits), grouped for reading: abcde-fghij.
    let raw = data_encoding::BASE32_NOPAD
        .encode(&crypto::random_bytes(7)?)
        .to_lowercase();
    Ok(format!("{}-{}", &raw[..5], &raw[5..10]))
}

fn normalize_recovery(code: &str) -> String {
    code.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Checks a TOTP code or an unused recovery code of an enrolled user.
pub async fn check_code(
    pool: &PgPool,
    secrets: &SecretBox,
    user: Uuid,
    code: &str,
) -> CoreResult<bool> {
    let row: Option<(Option<String>, i64)> =
        sqlx::query_as("SELECT totp_secret_enc, totp_last_step FROM users WHERE id = $1")
            .bind(user)
            .fetch_optional(pool)
            .await?;
    let Some((Some(enc), last)) = row else {
        return Ok(false);
    };
    let secret = decode_secret(&secrets.decrypt(&enc)?)?;
    if let Some(step) = verify(&secret, code, now_unix(), last.max(0) as u64) {
        // Only one login may use this step, even when two race.
        let n = sqlx::query(
            "UPDATE users SET totp_last_step = $2 WHERE id = $1 AND totp_last_step < $2",
        )
        .bind(user)
        .bind(step as i64)
        .execute(pool)
        .await?
        .rows_affected();
        return Ok(n == 1);
    }
    let normalized = normalize_recovery(code);
    if normalized.len() != 10 {
        return Ok(false);
    }
    let n = sqlx::query(
        "UPDATE user_recovery_codes SET used_at = now()
         WHERE user_id = $1 AND code_digest = $2 AND used_at IS NULL",
    )
    .bind(user)
    .bind(crypto::token_digest(&normalized))
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n == 1)
}

/// Creates the second-step challenge after a correct password.
pub async fn new_challenge(pool: &PgPool, user: Uuid) -> CoreResult<String> {
    let token = crypto::random_token(32)?;
    sqlx::query("DELETE FROM login_challenges WHERE expires_at < now()")
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO login_challenges (token_digest, user_id, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(crypto::token_digest(&token))
    .bind(user)
    .bind(Utc::now() + Duration::minutes(CHALLENGE_MINUTES))
    .execute(pool)
    .await?;
    Ok(token)
}

/// Completes a challenge: the user id if `code` is valid. Each attempt
/// counts; after too many the challenge is gone and the login restarts.
pub async fn complete_challenge(
    pool: &PgPool,
    secrets: &SecretBox,
    token: &str,
    code: &str,
) -> CoreResult<Option<Uuid>> {
    let digest = crypto::token_digest(token);
    let user: Option<Uuid> = sqlx::query_scalar(
        "UPDATE login_challenges SET attempts = attempts + 1
         WHERE token_digest = $1 AND expires_at > now() AND attempts < $2
         RETURNING user_id",
    )
    .bind(&digest)
    .bind(MAX_ATTEMPTS)
    .fetch_optional(pool)
    .await?;
    let Some(user) = user else {
        return Ok(None);
    };
    if check_code(pool, secrets, user, code).await? {
        sqlx::query("DELETE FROM login_challenges WHERE token_digest = $1")
            .bind(&digest)
            .execute(pool)
            .await?;
        return Ok(Some(user));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6238_vectors() {
        // RFC 6238 appendix B, SHA-1 secret "12345678901234567890" (8 digits there;
        // the last 6 digits are the 6-digit code).
        let secret = b"12345678901234567890";
        for (t, expected) in [
            (59, "287082"),
            (1_111_111_109, "081804"),
            (1_111_111_111, "050471"),
            (1_234_567_890, "005924"),
            (2_000_000_000, "279037"),
        ] {
            assert_eq!(code_at(secret, t), expected, "t={t}");
        }
    }

    #[test]
    fn verify_window_and_replay() {
        let secret = b"12345678901234567890";
        let t = 1_234_567_890;
        let code = code_at(secret, t);
        let step = step_of(t);
        assert_eq!(verify(secret, &code, t, 0), Some(step));
        assert_eq!(
            verify(secret, &code, t + 30, 0),
            Some(step),
            "previous step"
        );
        assert_eq!(verify(secret, &code, t + 90, 0), None, "too old");
        assert_eq!(verify(secret, &code, t, step), None, "replay");
        assert_eq!(verify(secret, "12345", t, 0), None);
        assert_eq!(
            verify(secret, &format!("{} {}", &code[..3], &code[3..]), t, 0),
            Some(step)
        );
    }

    #[test]
    fn secrets_and_uris() {
        let s = new_secret().unwrap();
        assert_eq!(s.len(), 32);
        assert_eq!(decode_secret(&s).unwrap().len(), 20);
        let uri = otpauth_uri("JBSWY3DPEHPK3PXP", "anna beispiel");
        assert_eq!(
            uri,
            "otpauth://totp/TalkOps:anna%20beispiel?secret=JBSWY3DPEHPK3PXP&issuer=TalkOps&algorithm=SHA1&digits=6&period=30"
        );
        let r = recovery_code().unwrap();
        assert_eq!(r.len(), 11);
        assert_eq!(normalize_recovery(&r.to_uppercase()).len(), 10);
    }
}
