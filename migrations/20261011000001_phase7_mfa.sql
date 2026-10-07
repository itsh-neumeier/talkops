-- Phase 7: two-factor login with TOTP (RFC 6238) for local accounts.

ALTER TABLE users
    -- Active TOTP secret (encrypted with TALKOPS_SECRET_KEY); NULL = no 2FA.
    ADD COLUMN totp_secret_enc  TEXT,
    -- Secret shown during enrollment, active only after a valid code.
    ADD COLUMN totp_pending_enc TEXT,
    -- Last accepted time step: a code cannot be used twice.
    ADD COLUMN totp_last_step   BIGINT NOT NULL DEFAULT 0;

-- One-time recovery codes (SHA-256 only).
CREATE TABLE user_recovery_codes (
    user_id     UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    code_digest TEXT NOT NULL,
    used_at     TIMESTAMPTZ,
    PRIMARY KEY (user_id, code_digest)
);

-- Second login step: password accepted, code still missing.
CREATE TABLE login_challenges (
    token_digest TEXT PRIMARY KEY,
    user_id      UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    attempts     INTEGER NOT NULL DEFAULT 0,
    expires_at   TIMESTAMPTZ NOT NULL
);
CREATE INDEX login_challenges_expires_idx ON login_challenges (expires_at);
