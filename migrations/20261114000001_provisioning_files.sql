-- Provisioning files that an admin edits or adds (ADR 0052). A file with the
-- name of a generated file replaces it; another name adds a file. The content
-- is encrypted because a copy of a generated file can contain SIP secrets.

CREATE TABLE provisioning_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    transport TEXT NOT NULL CHECK (transport IN ('http', 'tftp')),
    -- TFTP only: NULL applies to every Cisco Enterprise phone.
    phone_id UUID REFERENCES phones (id) ON DELETE CASCADE,
    -- HTTP: the path below /provisioning/; TFTP: the requested file name.
    path TEXT NOT NULL CHECK (length(path) BETWEEN 1 AND 200),
    -- SecretBox(base64(content)).
    content TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 0 AND 1048576),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (transport = 'tftp' OR phone_id IS NULL)
);
CREATE UNIQUE INDEX provisioning_files_path_idx ON provisioning_files
    (tenant_id, transport, COALESCE(phone_id, '00000000-0000-0000-0000-000000000000'::uuid), path);
