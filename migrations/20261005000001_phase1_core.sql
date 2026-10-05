-- Phase 1: users, extensions, devices, trunks, numbers, settings, CDR, audit log, sessions.

CREATE TYPE user_role AS ENUM ('admin', 'operator', 'user');

-- Per-tenant telephony settings (one row per tenant).
CREATE TABLE tenant_settings (
    tenant_id              UUID PRIMARY KEY REFERENCES tenants (id) ON DELETE CASCADE,
    -- Country calling code without '+', e.g. '49'.
    country_code           TEXT NOT NULL DEFAULT '49' CHECK (country_code ~ '^[1-9][0-9]{0,2}$'),
    -- Local area code without the national prefix, e.g. '89' for Munich. Empty: no local dialing.
    area_code              TEXT NOT NULL DEFAULT '' CHECK (area_code ~ '^[0-9]{0,6}$'),
    national_prefix        TEXT NOT NULL DEFAULT '0' CHECK (national_prefix ~ '^[0-9]{0,2}$'),
    international_prefix   TEXT NOT NULL DEFAULT '00' CHECK (international_prefix ~ '^[0-9]{1,4}$'),
    -- Emergency and other short service numbers that are sent to the trunk unchanged.
    emergency_numbers      TEXT[] NOT NULL DEFAULT ARRAY['110', '112'],
    -- Public IP (or 'stun:host:port') announced to SIP trunks; empty = do not rewrite.
    external_ip            TEXT NOT NULL DEFAULT '',
    default_language       TEXT NOT NULL DEFAULT 'de',
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO tenant_settings (tenant_id) SELECT id FROM tenants;

CREATE TABLE users (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    username       TEXT NOT NULL CHECK (username ~ '^[a-zA-Z0-9._@-]{1,64}$'),
    display_name   TEXT NOT NULL,
    email          TEXT,
    role           user_role NOT NULL DEFAULT 'user',
    -- argon2id PHC string; NULL for accounts that cannot log in locally (LDAP/OIDC later).
    password_hash  TEXT,
    auth_source    TEXT NOT NULL DEFAULT 'local',
    enabled        BOOLEAN NOT NULL DEFAULT true,
    last_login_at  TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX users_username_idx ON users (tenant_id, lower(username));

CREATE TABLE sessions (
    -- SHA-256 of the session token; the token itself is only in the cookie.
    token_digest  TEXT PRIMARY KEY,
    user_id       UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    csrf_token    TEXT NOT NULL,
    ip            TEXT,
    user_agent    TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at    TIMESTAMPTZ NOT NULL
);
CREATE INDEX sessions_user_idx ON sessions (user_id);
CREATE INDEX sessions_expiry_idx ON sessions (expires_at);

CREATE TABLE trunks (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    -- Preset id from presets/trunks/<id>.yaml, or 'generic'.
    preset      TEXT NOT NULL,
    -- Overrides of preset parameters (registrar, transport, number_format, ...).
    overrides   JSONB NOT NULL DEFAULT '{}'::jsonb,
    enabled     BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX trunks_name_idx ON trunks (tenant_id, lower(name));

-- A registration/authentication identity at the provider. Providers with
-- per-number credentials (e.g. LEONET) have one account per number.
CREATE TABLE trunk_accounts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    trunk_id        UUID NOT NULL REFERENCES trunks (id) ON DELETE CASCADE,
    username        TEXT NOT NULL,
    auth_username   TEXT NOT NULL DEFAULT '',
    password_enc    TEXT NOT NULL,
    enabled         BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX trunk_accounts_trunk_idx ON trunk_accounts (trunk_id);

CREATE TABLE extensions (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id            UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    number               TEXT NOT NULL CHECK (number ~ '^[0-9]{2,8}$'),
    display_name         TEXT NOT NULL,
    user_id              UUID REFERENCES users (id) ON DELETE SET NULL,
    -- Caller ID for external calls; NULL = tenant default number.
    outbound_number_id   UUID,
    -- Suppress the caller ID on external calls (CLIR).
    hide_caller_id       BOOLEAN NOT NULL DEFAULT false,
    ring_timeout_secs    INTEGER NOT NULL DEFAULT 30 CHECK (ring_timeout_secs BETWEEN 5 AND 300),
    enabled              BOOLEAN NOT NULL DEFAULT true,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX extensions_number_idx ON extensions (tenant_id, number);
CREATE INDEX extensions_user_idx ON extensions (user_id);

CREATE TYPE device_kind AS ENUM ('desk', 'dect', 'softphone', 'mobile', 'door', 'other');

-- Every device has its own SIP credentials; all devices of an extension ring together.
CREATE TABLE devices (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    extension_id   UUID NOT NULL REFERENCES extensions (id) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    kind           device_kind NOT NULL DEFAULT 'desk',
    sip_username   TEXT NOT NULL CHECK (sip_username ~ '^[a-zA-Z0-9._-]{3,64}$'),
    sip_password_enc TEXT NOT NULL,
    mac            TEXT CHECK (mac ~ '^[0-9a-f]{12}$'),
    model          TEXT,
    enabled        BOOLEAN NOT NULL DEFAULT true,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- SIP usernames are unique across tenants: registrations are looked up by user only.
CREATE UNIQUE INDEX devices_sip_username_idx ON devices (lower(sip_username));
CREATE INDEX devices_extension_idx ON devices (extension_id);

CREATE TYPE number_destination AS ENUM ('none', 'extension');

-- Phone numbers (DIDs) owned at a trunk, stored in E.164 with '+'.
CREATE TABLE numbers (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    trunk_id          UUID NOT NULL REFERENCES trunks (id) ON DELETE CASCADE,
    account_id        UUID REFERENCES trunk_accounts (id) ON DELETE SET NULL,
    e164              TEXT NOT NULL CHECK (e164 ~ '^\+[1-9][0-9]{4,14}$'),
    label             TEXT NOT NULL DEFAULT '',
    destination_type  number_destination NOT NULL DEFAULT 'none',
    destination_id    UUID,
    enabled           BOOLEAN NOT NULL DEFAULT true,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX numbers_e164_idx ON numbers (tenant_id, e164);
CREATE INDEX numbers_trunk_idx ON numbers (trunk_id);

ALTER TABLE extensions
    ADD CONSTRAINT extensions_outbound_number_fk
    FOREIGN KEY (outbound_number_id) REFERENCES numbers (id) ON DELETE SET NULL;

ALTER TABLE tenant_settings
    ADD COLUMN default_number_id UUID REFERENCES numbers (id) ON DELETE SET NULL;

CREATE TYPE call_direction AS ENUM ('inbound', 'outbound', 'internal');

CREATE TABLE cdr (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id        UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    call_uuid        TEXT NOT NULL,
    direction        call_direction NOT NULL,
    caller_number    TEXT NOT NULL DEFAULT '',
    caller_name      TEXT NOT NULL DEFAULT '',
    destination      TEXT NOT NULL DEFAULT '',
    -- Calling extension (outbound/internal) or ringing extension (inbound).
    extension_id     UUID REFERENCES extensions (id) ON DELETE SET NULL,
    -- Called extension of internal calls.
    dest_extension_id UUID REFERENCES extensions (id) ON DELETE SET NULL,
    trunk_id         UUID REFERENCES trunks (id) ON DELETE SET NULL,
    number_id        UUID REFERENCES numbers (id) ON DELETE SET NULL,
    started_at       TIMESTAMPTZ NOT NULL,
    answered_at      TIMESTAMPTZ,
    ended_at         TIMESTAMPTZ NOT NULL,
    duration_secs    INTEGER NOT NULL DEFAULT 0,
    billsec          INTEGER NOT NULL DEFAULT 0,
    hangup_cause     TEXT NOT NULL DEFAULT '',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX cdr_call_uuid_idx ON cdr (call_uuid);
CREATE INDEX cdr_tenant_started_idx ON cdr (tenant_id, started_at DESC);
CREATE INDEX cdr_extension_idx ON cdr (extension_id, started_at DESC);
CREATE INDEX cdr_dest_extension_idx ON cdr (dest_extension_id, started_at DESC);

CREATE TABLE audit_log (
    id           BIGSERIAL PRIMARY KEY,
    tenant_id    UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    user_id      UUID REFERENCES users (id) ON DELETE SET NULL,
    action       TEXT NOT NULL,
    entity_type  TEXT NOT NULL,
    entity_id    TEXT,
    details      JSONB NOT NULL DEFAULT '{}'::jsonb,
    ip           TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX audit_log_tenant_idx ON audit_log (tenant_id, created_at DESC);
