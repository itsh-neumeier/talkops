-- Phase 5: call recording, transcription (whisper.cpp) and full-text search.

ALTER TABLE tenant_settings
    -- Which calls are recorded by default.
    ADD COLUMN record_inbound           BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN record_outbound          BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN record_internal          BOOLEAN NOT NULL DEFAULT false,
    -- Tell both parties that the call is recorded (required in Germany).
    ADD COLUMN recording_announcement   BOOLEAN NOT NULL DEFAULT true,
    -- Delete recordings (and their transcripts) after N days; 0 = keep.
    ADD COLUMN recording_retention_days INTEGER NOT NULL DEFAULT 90
                                        CHECK (recording_retention_days BETWEEN 0 AND 3650),
    -- Transcribe recordings and voicemails with whisper.cpp.
    ADD COLUMN transcription_enabled    BOOLEAN NOT NULL DEFAULT false;

-- Per-extension override of the tenant defaults.
ALTER TABLE extensions
    ADD COLUMN record_calls TEXT NOT NULL DEFAULT 'inherit'
                            CHECK (record_calls IN ('inherit', 'always', 'never'));

CREATE TABLE recordings (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    call_uuid         TEXT NOT NULL UNIQUE,
    cdr_id            UUID REFERENCES cdr (id) ON DELETE SET NULL,
    -- Stereo WAV (left: caller, right: called party) relative to the recordings volume.
    file              TEXT NOT NULL,
    duration_secs     INTEGER NOT NULL DEFAULT 0,
    size_bytes        BIGINT NOT NULL DEFAULT 0,
    transcript_status TEXT NOT NULL DEFAULT 'none'
                      CHECK (transcript_status IN ('none', 'pending', 'done', 'failed')),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX recordings_created_idx ON recordings (tenant_id, created_at);

-- Transcripts of recordings and voicemails. `search` uses the 'simple'
-- configuration (no stemming) so one index serves German and English.
CREATE TABLE transcripts (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    recording_id  UUID UNIQUE REFERENCES recordings (id) ON DELETE CASCADE,
    voicemail_id  UUID UNIQUE REFERENCES voicemail_messages (id) ON DELETE CASCADE,
    language      TEXT NOT NULL DEFAULT '',
    text          TEXT NOT NULL,
    -- [{"start": 1.2, "end": 3.4, "speaker": "caller", "text": "…"}, …]
    segments      JSONB NOT NULL DEFAULT '[]',
    search        TSVECTOR GENERATED ALWAYS AS (to_tsvector('simple', text)) STORED,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((recording_id IS NULL) <> (voicemail_id IS NULL))
);
CREATE INDEX transcripts_search_idx ON transcripts USING GIN (search);

ALTER TABLE voicemail_messages
    ADD COLUMN transcript_status TEXT NOT NULL DEFAULT 'none'
                                 CHECK (transcript_status IN ('none', 'pending', 'done', 'failed'));
