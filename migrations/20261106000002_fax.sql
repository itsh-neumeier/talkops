ALTER TABLE users ADD CONSTRAINT fax_users_scope UNIQUE (tenant_id, id);
ALTER TABLE numbers ADD CONSTRAINT fax_numbers_scope UNIQUE (tenant_id, id);

CREATE TABLE fax_settings (
    tenant_id uuid PRIMARY KEY REFERENCES tenants ON DELETE CASCADE,
    config jsonb NOT NULL DEFAULT '{}',
    imap_password_enc text
);
CREATE TABLE fax_endpoints (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES tenants ON DELETE CASCADE,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    owner_id uuid NOT NULL,
    send_users uuid[] NOT NULL DEFAULT '{}',
    receive_users uuid[] NOT NULL DEFAULT '{}',
    reception_email text NOT NULL DEFAULT '',
    enabled boolean NOT NULL DEFAULT true,
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, owner_id) REFERENCES users (tenant_id, id)
);
CREATE TABLE fax_whitelist (
    tenant_id uuid NOT NULL REFERENCES tenants ON DELETE CASCADE,
    email text NOT NULL CHECK (length(email) BETWEEN 3 AND 254),
    user_id uuid NOT NULL,
    enabled boolean NOT NULL DEFAULT true,
    PRIMARY KEY (tenant_id, email),
    FOREIGN KEY (tenant_id, user_id) REFERENCES users (tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE fax_templates (
    tenant_id uuid NOT NULL REFERENCES tenants ON DELETE CASCADE,
    user_id uuid NOT NULL,
    header text NOT NULL,
    footer text NOT NULL,
    PRIMARY KEY (tenant_id, user_id),
    FOREIGN KEY (tenant_id, user_id) REFERENCES users (tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE faxes (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES tenants ON DELETE CASCADE,
    owner_id uuid NOT NULL,
    endpoint_id uuid,
    direction text NOT NULL CHECK (direction IN ('inbound', 'outbound')),
    state text NOT NULL CHECK (state IN ('preparing', 'draft', 'queued', 'dialing', 'sending', 'received', 'succeeded', 'failed', 'cancelled', 'unknown')),
    number_id uuid,
    trunk_id uuid,
    source text NOT NULL DEFAULT '',
    destination text NOT NULL DEFAULT '',
    sender_email text NOT NULL DEFAULT '',
    header text NOT NULL DEFAULT '',
    footer text NOT NULL DEFAULT '',
    inputs jsonb NOT NULL DEFAULT '[]',
    pages integer NOT NULL DEFAULT 0,
    result text NOT NULL DEFAULT '',
    attempts integer NOT NULL DEFAULT 0,
    call_uuid uuid,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, owner_id) REFERENCES users (tenant_id, id),
    FOREIGN KEY (tenant_id, endpoint_id) REFERENCES fax_endpoints (tenant_id, id),
    FOREIGN KEY (tenant_id, number_id) REFERENCES numbers (tenant_id, id)
);
CREATE INDEX fax_queue ON faxes (next_attempt_at) WHERE state = 'queued';
CREATE INDEX fax_history ON faxes (tenant_id, created_at DESC);
CREATE UNIQUE INDEX fax_active_trunk ON faxes (trunk_id) WHERE direction = 'outbound' AND state IN ('dialing', 'sending');
CREATE UNIQUE INDEX fax_call ON faxes (call_uuid) WHERE call_uuid IS NOT NULL;
CREATE TABLE fax_attempts (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES tenants ON DELETE CASCADE,
    fax_id uuid NOT NULL,
    call_uuid uuid NOT NULL UNIQUE,
    result text NOT NULL DEFAULT '',
    pages integer NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    FOREIGN KEY (tenant_id, fax_id) REFERENCES faxes (tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE fax_mail_receipts (
    tenant_id uuid NOT NULL REFERENCES tenants ON DELETE CASCADE,
    receipt text NOT NULL,
    fingerprint text NOT NULL,
    sender text NOT NULL,
    result text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, receipt),
    UNIQUE (tenant_id, fingerprint)
);
CREATE TABLE fax_mail_outbox (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES tenants ON DELETE CASCADE,
    fax_id uuid,
    recipient text NOT NULL,
    kind text NOT NULL,
    reason text NOT NULL DEFAULT '',
    attempts integer NOT NULL DEFAULT 0,
    sent boolean NOT NULL DEFAULT false,
    run_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, fax_id) REFERENCES faxes (tenant_id, id) ON DELETE CASCADE
);
