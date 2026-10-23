-- Changes to the UniFi voice SSID (ADR 0042). Writes are off by default.
-- Snapshots contain only allowlist fields; the passphrase and other
-- secrets of the SSID are never stored.

ALTER TABLE unifi_connections
    -- Integration API ID of the voice SSID (UUID text) and its name at selection.
    ADD COLUMN voice_wifi_id TEXT CHECK (voice_wifi_id ~ '^[0-9A-Fa-f-]{1,64}$'),
    ADD COLUMN voice_wifi_name TEXT CHECK (length(voice_wifi_name) <= 64),
    ADD COLUMN write_enabled BOOLEAN NOT NULL DEFAULT false;

CREATE TABLE unifi_wifi_changes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    site_id TEXT NOT NULL,
    wifi_id TEXT NOT NULL,
    wifi_name TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL CHECK (kind IN ('apply', 'rollback')),
    -- For a rollback: the change that it reverts.
    rollback_of UUID REFERENCES unifi_wifi_changes (id) ON DELETE SET NULL,
    -- The history stays after the survey retention.
    survey_id UUID REFERENCES wifi_surveys (id) ON DELETE SET NULL,
    state TEXT NOT NULL CHECK (state IN ('applying', 'applied', 'failed', 'rolled_back')),
    fields TEXT[] NOT NULL,
    -- Allowlist values: {"field": {"value": ...}} or {"field": {"absent": true}}.
    before JSONB,
    after JSONB,
    network_version TEXT,
    -- Error class only (no details from UniFi).
    error TEXT,
    created_by UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ
);
CREATE INDEX unifi_wifi_changes_tenant_idx ON unifi_wifi_changes (tenant_id, created_at DESC);
-- One change at a time for each tenant.
CREATE UNIQUE INDEX unifi_wifi_changes_applying_idx ON unifi_wifi_changes (tenant_id)
    WHERE state = 'applying';
