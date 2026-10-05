-- Phase 2: provisioned phones (Yealink), firmware, phonebook contacts,
-- DND and unconditional call forwarding.

ALTER TABLE tenant_settings
    ADD COLUMN provisioning_username     TEXT NOT NULL DEFAULT 'provision',
    -- Encrypted with TALKOPS_SECRET_KEY; generated on first use.
    ADD COLUMN provisioning_password_enc TEXT,
    -- Admin password of the phones' web UI, written to every phone config.
    ADD COLUMN phone_admin_password_enc  TEXT,
    ADD COLUMN timezone                  TEXT NOT NULL DEFAULT 'Europe/Berlin';

-- A physical, provisioned phone or DECT base identified by its MAC address.
CREATE TABLE phones (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    mac             TEXT NOT NULL CHECK (mac ~ '^[0-9a-f]{12}$'),
    -- Model id from presets/phones/*.yaml.
    model           TEXT NOT NULL,
    name            TEXT NOT NULL,
    -- Programmable keys: [{"key": 2, "type": "blf", "value": "21", "label": "Lab", "account": 1}]
    line_keys       JSONB NOT NULL DEFAULT '[]'::jsonb,
    last_seen_at    TIMESTAMPTZ,
    last_ip         TEXT,
    last_firmware   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Provisioning requests carry only the MAC, so it is unique across tenants.
CREATE UNIQUE INDEX phones_mac_idx ON phones (mac);

-- SIP devices (accounts) can be placed on a phone: account N of the phone,
-- or handset N of a DECT base.
ALTER TABLE devices
    ADD COLUMN phone_id      UUID REFERENCES phones (id) ON DELETE SET NULL,
    ADD COLUMN account_index SMALLINT CHECK (account_index BETWEEN 1 AND 100);
CREATE UNIQUE INDEX devices_phone_account_idx ON devices (phone_id, account_index) WHERE phone_id IS NOT NULL;

-- Move the MAC/model stored on devices in phase 1 to phones.
INSERT INTO phones (tenant_id, mac, model, name)
SELECT DISTINCT ON (d.mac) d.tenant_id, d.mac, coalesce(lower(replace(d.model, '-', '')), 'unknown'), d.name
FROM devices d WHERE d.mac IS NOT NULL
ORDER BY d.mac, d.created_at;
UPDATE devices d SET phone_id = p.id, account_index = 1
FROM phones p WHERE p.mac = d.mac
  AND d.id = (SELECT d2.id FROM devices d2 WHERE d2.mac = d.mac ORDER BY d2.created_at LIMIT 1);
ALTER TABLE devices DROP COLUMN mac, DROP COLUMN model;

-- Uploaded firmware images; at most one active image per model.
CREATE TABLE firmware (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    model        TEXT NOT NULL,
    filename     TEXT NOT NULL CHECK (filename ~ '^[A-Za-z0-9._-]{1,128}$'),
    size_bytes   BIGINT NOT NULL,
    sha256       TEXT NOT NULL,
    active       BOOLEAN NOT NULL DEFAULT false,
    uploaded_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX firmware_active_idx ON firmware (tenant_id, model) WHERE active;

-- Shared phonebook entries (in addition to the internal extension list).
CREATE TABLE contacts (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    company     TEXT NOT NULL DEFAULT '',
    phone_work  TEXT NOT NULL DEFAULT '',
    phone_mobile TEXT NOT NULL DEFAULT '',
    phone_other TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX contacts_name_idx ON contacts (tenant_id, lower(name));

ALTER TABLE extensions
    ADD COLUMN dnd         BOOLEAN NOT NULL DEFAULT false,
    -- Unconditional forwarding target (extension number or external number as dialed).
    ADD COLUMN forward_all TEXT CHECK (forward_all ~ '^\+?[0-9]{2,20}$');
