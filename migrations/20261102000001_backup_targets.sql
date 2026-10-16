-- Backup targets (ADR 0031): places that receive copies of backup archives.
-- Each target has its own schedule and retention. The server copies the
-- archives with rclone.

CREATE TABLE backup_targets (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id          UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    name               TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    kind               TEXT NOT NULL
        CHECK (kind IN ('local', 'smb', 's3', 'ftp', 'webdav', 'sftp')),
    enabled            BOOLEAN NOT NULL DEFAULT true,
    -- Connection settings without secrets (host, port, user, bucket provider ...).
    settings           JSONB NOT NULL DEFAULT '{}',
    -- Passwords and keys as JSON, encrypted with TALKOPS_SECRET_KEY (ADR 0008).
    secret_enc         TEXT,
    -- Folder on the target. S3: bucket/prefix, SMB: share/folder, local: absolute path.
    path               TEXT NOT NULL DEFAULT '',
    -- Plain FTP or HTTP (WebDAV, S3 endpoint) without TLS.
    allow_insecure     BOOLEAN NOT NULL DEFAULT false,
    include_recordings BOOLEAN NOT NULL DEFAULT true,
    -- Schedule in local time. Bit 0 = Monday ... bit 6 = Sunday; 0 = manual only.
    weekdays           SMALLINT NOT NULL DEFAULT 127 CHECK (weekdays BETWEEN 0 AND 127),
    hour               SMALLINT NOT NULL DEFAULT 3 CHECK (hour BETWEEN 0 AND 23),
    minute             SMALLINT NOT NULL DEFAULT 30 CHECK (minute BETWEEN 0 AND 59),
    -- 0 = once a day; otherwise repeat every N hours after the first run of the day.
    every_hours        SMALLINT NOT NULL DEFAULT 0 CHECK (every_hours BETWEEN 0 AND 23),
    -- Retention: always the latest N archives, plus the latest archive of each
    -- day, week, month and year inside the given periods.
    keep_last          SMALLINT NOT NULL DEFAULT 3 CHECK (keep_last BETWEEN 1 AND 1000),
    keep_daily         SMALLINT NOT NULL DEFAULT 7 CHECK (keep_daily BETWEEN 0 AND 3660),
    keep_weekly        SMALLINT NOT NULL DEFAULT 4 CHECK (keep_weekly BETWEEN 0 AND 520),
    keep_monthly       SMALLINT NOT NULL DEFAULT 12 CHECK (keep_monthly BETWEEN 0 AND 1200),
    keep_yearly        SMALLINT NOT NULL DEFAULT 0 CHECK (keep_yearly BETWEEN 0 AND 100),
    -- Upload limit in KiB/s; NULL = no limit.
    bwlimit_kbps       INTEGER CHECK (bwlimit_kbps BETWEEN 1 AND 10000000),
    last_run_at        TIMESTAMPTZ,
    last_success_at    TIMESTAMPTZ,
    last_file          TEXT,
    last_error         TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name)
);
