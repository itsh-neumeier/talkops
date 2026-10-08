-- Transcription accuracy versus speed, and words Whisper should know
-- (names, companies, products).
ALTER TABLE tenant_settings
    ADD COLUMN transcription_quality TEXT NOT NULL DEFAULT 'fast'
        CHECK (transcription_quality IN ('fast', 'accurate', 'best')),
    ADD COLUMN transcription_vocabulary TEXT NOT NULL DEFAULT '';
