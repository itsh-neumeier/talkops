-- 1.7.1: Wi-Fi handsets as device kind, per-account label and display name on
-- provisioned phones, custom ringtones and wallpapers, phone book sections.

ALTER TYPE device_kind ADD VALUE IF NOT EXISTS 'wifi';

-- What the phone shows for the account; empty = extension number / name.
ALTER TABLE devices
    ADD COLUMN phone_label        TEXT NOT NULL DEFAULT '' CHECK (char_length(phone_label) <= 32),
    ADD COLUMN phone_display_name TEXT NOT NULL DEFAULT '' CHECK (char_length(phone_display_name) <= 64);

-- Uploaded ringtones (8 kHz mono WAV) and wallpapers (JPEG/PNG) served to
-- phones under /provisioning/media/<id>/<filename>.
CREATE TYPE phone_media_kind AS ENUM ('ringtone', 'wallpaper');
CREATE TABLE phone_media (
    id          UUID PRIMARY KEY,
    tenant_id   UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    kind        phone_media_kind NOT NULL,
    name        TEXT NOT NULL,
    filename    TEXT NOT NULL,
    size_bytes  BIGINT NOT NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX phone_media_tenant_idx ON phone_media (tenant_id, kind);

-- Phone book sections: contacts without a section are global (every phone),
-- a section is shown only on the phones it is assigned to.
CREATE TABLE phonebook_sections (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    name       TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name)
);

ALTER TABLE contacts
    ADD COLUMN section_id UUID REFERENCES phonebook_sections (id) ON DELETE CASCADE;

ALTER TABLE phones
    ADD COLUMN ringtone_id  UUID REFERENCES phone_media (id) ON DELETE SET NULL,
    ADD COLUMN wallpaper_id UUID REFERENCES phone_media (id) ON DELETE SET NULL,
    -- Phone book sections shown on this phone (at most 3, in this order).
    ADD COLUMN phonebook_sections UUID[] NOT NULL DEFAULT '{}';
