-- Phase 3: voicemail boxes and messages, greetings (recorded or TTS),
-- e-mail notification via SMTP.

-- Voicemail settings of an extension. Missing row = voicemail disabled.
CREATE TABLE voicemail_boxes (
    extension_id     UUID PRIMARY KEY REFERENCES extensions (id) ON DELETE CASCADE,
    tenant_id        UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    enabled          BOOLEAN NOT NULL DEFAULT false,
    -- argon2id hash; needed to check the box from another phone (*98).
    pin_hash         TEXT,
    email_notify     BOOLEAN NOT NULL DEFAULT false,
    attach_audio     BOOLEAN NOT NULL DEFAULT true,
    -- Prompt language; NULL = tenant default.
    language         TEXT CHECK (language IN ('de', 'en')),
    -- default: system prompt; tts: greeting_text rendered by Piper; recorded: via phone.
    greeting         TEXT NOT NULL DEFAULT 'default' CHECK (greeting IN ('default', 'tts', 'recorded')),
    greeting_text    TEXT NOT NULL DEFAULT '' CHECK (length(greeting_text) <= 1000),
    greeting_status  TEXT NOT NULL DEFAULT 'none'
                     CHECK (greeting_status IN ('none', 'pending', 'ready', 'failed')),
    max_message_secs INTEGER NOT NULL DEFAULT 180 CHECK (max_message_secs BETWEEN 10 AND 600),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE voicemail_messages (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    extension_id  UUID NOT NULL REFERENCES extensions (id) ON DELETE CASCADE,
    caller_number TEXT NOT NULL DEFAULT '',
    caller_name   TEXT NOT NULL DEFAULT '',
    duration_secs INTEGER NOT NULL CHECK (duration_secs >= 0),
    -- WAV file relative to the voicemail directory.
    file          TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'new' CHECK (status IN ('new', 'saved')),
    call_uuid     TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    heard_at      TIMESTAMPTZ
);
CREATE INDEX voicemail_messages_box ON voicemail_messages (extension_id, status, created_at);

-- Outgoing mail (voicemail notification).
ALTER TABLE tenant_settings
    ADD COLUMN smtp_host         TEXT NOT NULL DEFAULT '',
    ADD COLUMN smtp_port         INTEGER NOT NULL DEFAULT 587 CHECK (smtp_port BETWEEN 1 AND 65535),
    ADD COLUMN smtp_security     TEXT NOT NULL DEFAULT 'starttls'
                                 CHECK (smtp_security IN ('starttls', 'tls', 'none')),
    ADD COLUMN smtp_username     TEXT NOT NULL DEFAULT '',
    -- Encrypted with TALKOPS_SECRET_KEY.
    ADD COLUMN smtp_password_enc TEXT,
    ADD COLUMN smtp_from         TEXT NOT NULL DEFAULT '';
