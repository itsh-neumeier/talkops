-- Read-only connection to the UniFi Network application (ADR 0041).
-- The API key is encrypted with TALKOPS_SECRET_KEY (ADR 0008) and never
-- returned by the API.
CREATE TABLE unifi_connections (
    tenant_id UUID PRIMARY KEY REFERENCES tenants (id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    -- Site name of the classic API (`default` on most consoles).
    site TEXT NOT NULL DEFAULT 'default' CHECK (site ~ '^[A-Za-z0-9_-]{1,64}$'),
    api_key_enc TEXT NOT NULL,
    -- SHA-256 of the pinned server certificate (hex); empty for a public CA.
    cert_sha256 TEXT NOT NULL DEFAULT '' CHECK (cert_sha256 ~ '^([0-9a-f]{64})?$'),
    -- Result of the last connection test.
    version TEXT,
    capabilities JSONB NOT NULL DEFAULT '{}',
    last_check_at TIMESTAMPTZ,
    last_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
