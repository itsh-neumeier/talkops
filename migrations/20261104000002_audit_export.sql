-- Audit export and audit retention (ADR 0037).
--
-- One row per tenant: the retention of audit entries in the database and the
-- automatic export to a storage target (ADR 0036). The export writes the
-- entries after `last_exported_id` into a new file on each run.

CREATE TABLE audit_settings (
    tenant_id          UUID PRIMARY KEY REFERENCES tenants (id) ON DELETE CASCADE,
    -- Delete entries older than N days; 0 = keep forever.
    retention_days     INTEGER NOT NULL DEFAULT 0 CHECK (retention_days BETWEEN 0 AND 36500),
    export_enabled     BOOLEAN NOT NULL DEFAULT false,
    storage_target_id  UUID,
    -- Sub-folder below the folder of the storage target.
    folder             TEXT NOT NULL DEFAULT 'audit' CHECK (length(folder) <= 1024),
    format             TEXT NOT NULL DEFAULT 'json' CHECK (format IN ('csv', 'json')),
    -- Schedule in local time, as for backup jobs. Bit 0 = Monday.
    weekdays           SMALLINT NOT NULL DEFAULT 127 CHECK (weekdays BETWEEN 0 AND 127),
    hour               SMALLINT NOT NULL DEFAULT 2 CHECK (hour BETWEEN 0 AND 23),
    minute             SMALLINT NOT NULL DEFAULT 15 CHECK (minute BETWEEN 0 AND 59),
    every_hours        SMALLINT NOT NULL DEFAULT 0 CHECK (every_hours BETWEEN 0 AND 23),
    -- Retention of export files on the storage target, as for backup jobs.
    keep_last          SMALLINT NOT NULL DEFAULT 30 CHECK (keep_last BETWEEN 1 AND 1000),
    keep_daily         SMALLINT NOT NULL DEFAULT 0 CHECK (keep_daily BETWEEN 0 AND 3660),
    keep_weekly        SMALLINT NOT NULL DEFAULT 0 CHECK (keep_weekly BETWEEN 0 AND 520),
    keep_monthly       SMALLINT NOT NULL DEFAULT 0 CHECK (keep_monthly BETWEEN 0 AND 1200),
    keep_yearly        SMALLINT NOT NULL DEFAULT 0 CHECK (keep_yearly BETWEEN 0 AND 100),
    -- Highest audit entry ID that an export file contains.
    last_exported_id   BIGINT NOT NULL DEFAULT 0,
    last_run_at        TIMESTAMPTZ,
    last_success_at    TIMESTAMPTZ,
    last_error         TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT audit_settings_storage_target_fk
        FOREIGN KEY (tenant_id, storage_target_id)
        REFERENCES storage_targets (tenant_id, id) ON DELETE NO ACTION,
    CONSTRAINT audit_settings_export_target
        CHECK (NOT export_enabled OR storage_target_id IS NOT NULL)
);

-- Files written by the automatic export, with their SHA-256 checksum.
CREATE TABLE audit_exports (
    id                 BIGSERIAL PRIMARY KEY,
    tenant_id          UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    storage_target_id  UUID,
    file               TEXT NOT NULL,
    first_id           BIGINT NOT NULL,
    last_id            BIGINT NOT NULL,
    entries            INTEGER NOT NULL,
    bytes              BIGINT NOT NULL,
    sha256             TEXT NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX audit_exports_tenant_idx ON audit_exports (tenant_id, id DESC);

-- Incremental export and retention read audit entries by ID.
CREATE INDEX audit_log_tenant_id_idx ON audit_log (tenant_id, id);
