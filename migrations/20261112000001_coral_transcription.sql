-- Coral Edge TPU speech recognition (ADR 0046).

-- A connection to the coral-stt service on this server.
ALTER TABLE transcription_providers DROP CONSTRAINT transcription_providers_kind_check;
ALTER TABLE transcription_providers ADD CONSTRAINT transcription_providers_kind_check
    CHECK (kind IN ('openai', 'compatible', 'coral'));

-- The engine that was selected but not available, when a local fallback
-- made the transcript (e.g. "Coral/coral-auto"). NULL: no fallback.
ALTER TABLE transcripts ADD COLUMN fallback_from TEXT;

-- The engine of the latest live preview text and its fallback.
ALTER TABLE live_transcript_previews
    ADD COLUMN engine TEXT NOT NULL DEFAULT '',
    ADD COLUMN fallback_from TEXT;
