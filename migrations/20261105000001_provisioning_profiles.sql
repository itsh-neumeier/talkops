-- Optional per-phone settings preserve requests from older API clients.
ALTER TABLE phones ADD COLUMN provisioning JSONB NOT NULL DEFAULT '{}'::jsonb
    CHECK (jsonb_typeof(provisioning) = 'object');
ALTER TABLE tenant_settings ADD COLUMN provisioning_options JSONB NOT NULL DEFAULT '{}'::jsonb
    CHECK (jsonb_typeof(provisioning_options) = 'object');

CREATE TABLE provisioning_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    vendor TEXT NOT NULL,
    model TEXT NOT NULL,
    active_revision INTEGER,
    UNIQUE (tenant_id, id)
);
CREATE TABLE provisioning_template_revisions (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    template_id UUID NOT NULL,
    revision INTEGER NOT NULL CHECK (revision > 0),
    files JSONB NOT NULL CHECK (jsonb_typeof(files) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, template_id, revision),
    FOREIGN KEY (tenant_id, template_id) REFERENCES provisioning_templates(tenant_id, id) ON DELETE CASCADE
);
ALTER TABLE provisioning_templates ADD CONSTRAINT active_template_revision_fk
    FOREIGN KEY (tenant_id, id, active_revision)
    REFERENCES provisioning_template_revisions(tenant_id, template_id, revision);

CREATE TABLE provisioning_requests (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    mac TEXT,
    filename TEXT NOT NULL,
    scheme TEXT NOT NULL CHECK (scheme IN ('http', 'https')),
    status SMALLINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX provisioning_requests_tenant_recent ON provisioning_requests(tenant_id, id DESC);
CREATE INDEX provisioning_requests_phone_recent ON provisioning_requests(tenant_id, mac, id DESC);
