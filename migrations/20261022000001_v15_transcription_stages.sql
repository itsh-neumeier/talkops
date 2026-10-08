-- Staged transcription: a quick first pass, optionally replaced by a more
-- accurate second pass. Either pass can use a local Whisper model or an
-- OpenAI-compatible transcription API ('api').
ALTER TABLE tenant_settings
    DROP CONSTRAINT tenant_settings_transcription_quality_check,
    ADD CONSTRAINT tenant_settings_transcription_quality_check
        CHECK (transcription_quality IN ('fast', 'accurate', 'best', 'german', 'api')),
    ADD COLUMN transcription_refine TEXT NOT NULL DEFAULT ''
        CHECK (transcription_refine IN ('', 'fast', 'accurate', 'best', 'german', 'api')),
    ADD COLUMN transcription_api_url TEXT NOT NULL DEFAULT 'https://api.openai.com/v1',
    ADD COLUMN transcription_api_model TEXT NOT NULL DEFAULT 'whisper-1',
    -- Encrypted with TALKOPS_SECRET_KEY (ADR 0008); NULL = no key.
    ADD COLUMN transcription_api_key_enc TEXT;

-- Which engine produced a transcript, and whether a better one may follow.
ALTER TABLE transcripts
    ADD COLUMN engine TEXT NOT NULL DEFAULT '',
    ADD COLUMN final BOOLEAN NOT NULL DEFAULT true;
