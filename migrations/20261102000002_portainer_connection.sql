-- Portainer API connection (ADR 0032): before each backup, TalkOps reads the
-- stack file and the stack environment of the TalkOps stack for recovery.

CREATE TABLE portainer_connections (
    tenant_id     UUID PRIMARY KEY REFERENCES tenants (id) ON DELETE CASCADE,
    -- Base URL, e.g. https://127.0.0.1:9443.
    url           TEXT NOT NULL CHECK (length(url) BETWEEN 8 AND 300),
    stack_id      INTEGER NOT NULL CHECK (stack_id > 0),
    -- SHA-256 of a pinned self-signed certificate (hex); '' = public CA.
    cert_sha256   TEXT NOT NULL DEFAULT '' CHECK (cert_sha256 ~ '^([0-9a-f]{64})?$'),
    -- Access token, encrypted with TALKOPS_SECRET_KEY (ADR 0008).
    token_enc     TEXT NOT NULL,
    stack_name    TEXT,
    last_sync_at  TIMESTAMPTZ,
    last_error    TEXT,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
