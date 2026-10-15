-- Transcription jobs: progress, pause/stop, resume points, speed estimates
-- and worker resource reports (ADR 0030).

ALTER TYPE job_status ADD VALUE IF NOT EXISTS 'paused';
ALTER TYPE job_status ADD VALUE IF NOT EXISTS 'cancelled';

ALTER TABLE jobs
    -- Share of the work done (0 to 1), reported by the worker.
    ADD COLUMN progress           REAL,
    -- First start of the job, and start and progress of the current run.
    ADD COLUMN started_at         TIMESTAMPTZ,
    ADD COLUMN run_started_at     TIMESTAMPTZ,
    ADD COLUMN run_progress_start REAL NOT NULL DEFAULT 0,
    -- User who started a manual job.
    ADD COLUMN requested_by       UUID REFERENCES users (id) ON DELETE SET NULL,
    -- Request from the UI to a running job.
    ADD COLUMN control            TEXT CHECK (control IN ('pause', 'stop')),
    -- Resume point of a paused job (worker format).
    ADD COLUMN checkpoint         JSONB;

-- Measured processing time per second of recording, for each engine
-- (`local:fast`, `cloud:gpt-transcribe`, …). Hardware-wide, no tenant.
CREATE TABLE transcription_speed (
    engine             TEXT PRIMARY KEY,
    secs_per_audio_sec REAL NOT NULL CHECK (secs_per_audio_sec > 0),
    samples            INTEGER NOT NULL DEFAULT 0,
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Resource usage that each media worker reports every few seconds.
CREATE TABLE worker_status (
    worker_id       TEXT PRIMARY KEY,
    cpu_percent     REAL NOT NULL DEFAULT 0,
    cpus            INTEGER NOT NULL DEFAULT 1,
    load1           REAL NOT NULL DEFAULT 0,
    rss_bytes       BIGINT NOT NULL DEFAULT 0,
    mem_used_bytes  BIGINT NOT NULL DEFAULT 0,
    mem_total_bytes BIGINT NOT NULL DEFAULT 0,
    job_id          UUID,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
