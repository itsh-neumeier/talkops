-- Storage targets (ADR 0036): connections to local folders, SMB, S3, FTP,
-- WebDAV and SFTP that several features use. Backup jobs (table
-- `backup_targets`) keep only their schedule and retention and refer to a
-- storage target. Other features (for example the audit export) refer to
-- storage targets in the same way.

CREATE TABLE storage_targets (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    kind            TEXT NOT NULL
        CHECK (kind IN ('local', 'smb', 's3', 'ftp', 'webdav', 'sftp')),
    -- Connection settings without secrets (host, port, user, bucket provider ...).
    settings        JSONB NOT NULL DEFAULT '{}',
    -- Passwords and keys as JSON, encrypted with TALKOPS_SECRET_KEY (ADR 0008).
    secret_enc      TEXT,
    -- Base folder. S3: bucket/prefix, SMB: share/folder, local: absolute path.
    path            TEXT NOT NULL DEFAULT '',
    -- Plain FTP or HTTP (WebDAV, S3 endpoint) without TLS.
    allow_insecure  BOOLEAN NOT NULL DEFAULT false,
    -- Upload limit in KiB/s; NULL = no limit.
    bwlimit_kbps    INTEGER CHECK (bwlimit_kbps BETWEEN 1 AND 10000000),
    -- Result of the last connection test of the saved target.
    last_test_at    TIMESTAMPTZ,
    last_test_ok    BOOLEAN,
    last_test_error TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name),
    -- Target of the tenant-safe foreign keys of the users.
    UNIQUE (tenant_id, id)
);

-- Each backup target becomes a storage target with the same id and name.
INSERT INTO storage_targets (id, tenant_id, name, kind, settings, secret_enc, path,
                             allow_insecure, bwlimit_kbps, created_at, updated_at)
SELECT id, tenant_id, name, kind, settings, secret_enc, path,
       allow_insecure, bwlimit_kbps, created_at, updated_at
FROM backup_targets;

ALTER TABLE backup_targets ADD COLUMN storage_target_id UUID;
-- Sub-folder below the folder of the storage target; '' = the folder itself.
ALTER TABLE backup_targets ADD COLUMN folder TEXT NOT NULL DEFAULT ''
    CHECK (length(folder) <= 1024);
UPDATE backup_targets SET storage_target_id = id;
ALTER TABLE backup_targets ALTER COLUMN storage_target_id SET NOT NULL;
-- NO ACTION (the default) rejects the deletion of a used storage target at
-- the end of the statement, like RESTRICT, but lets a tenant deletion
-- cascade to both tables.
ALTER TABLE backup_targets ADD CONSTRAINT backup_targets_storage_target_fk
    FOREIGN KEY (tenant_id, storage_target_id)
    REFERENCES storage_targets (tenant_id, id) ON DELETE NO ACTION;
-- Retention deletes TalkOps archives in the folder of a job. Two jobs in
-- the same folder would delete the archives of each other.
ALTER TABLE backup_targets ADD CONSTRAINT backup_targets_folder_unique
    UNIQUE (storage_target_id, folder);
CREATE INDEX backup_targets_storage_target_idx ON backup_targets (storage_target_id);

ALTER TABLE backup_targets
    DROP COLUMN kind,
    DROP COLUMN settings,
    DROP COLUMN secret_enc,
    DROP COLUMN path,
    DROP COLUMN allow_insecure,
    DROP COLUMN bwlimit_kbps;

-- New permission: manage storage targets. Roles that manage backups keep
-- the same access (backup targets were managed with `backups.manage`).
UPDATE roles SET permissions = array_append(permissions, 'storage.manage')
WHERE 'backups.manage' = ANY (permissions)
  AND NOT 'storage.manage' = ANY (permissions);
