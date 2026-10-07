-- Phase 8: scheduled backups (database dump + data volumes as tar.gz).

CREATE TABLE backup_settings (
    tenant_id          UUID PRIMARY KEY REFERENCES tenants (id) ON DELETE CASCADE,
    enabled            BOOLEAN NOT NULL DEFAULT true,
    -- Local hour (container time zone) the daily backup runs at.
    hour               SMALLINT NOT NULL DEFAULT 3 CHECK (hour BETWEEN 0 AND 23),
    -- Number of archives kept; older ones are deleted after a backup.
    keep               SMALLINT NOT NULL DEFAULT 7 CHECK (keep BETWEEN 1 AND 365),
    include_recordings BOOLEAN NOT NULL DEFAULT true,
    last_run_at        TIMESTAMPTZ,
    last_file          TEXT,
    last_error         TEXT,
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO backup_settings (tenant_id) SELECT id FROM tenants;
