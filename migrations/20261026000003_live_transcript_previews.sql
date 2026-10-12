-- Temporary, per-viewer transcript previews. Final transcripts remain separate.
CREATE TABLE live_transcript_previews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    call_uuid UUID NOT NULL,
    file TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    cursor_frames BIGINT NOT NULL DEFAULT -1 CHECK (cursor_frames >= -1),
    processed_secs REAL NOT NULL DEFAULT 0,
    segments JSONB NOT NULL DEFAULT '[]'::jsonb,
    job_id UUID REFERENCES jobs(id) ON DELETE SET NULL,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, user_id, call_uuid)
);
CREATE INDEX live_transcript_previews_expiry ON live_transcript_previews(requested_at);
