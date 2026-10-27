-- Opt-in complete-word previews for the Coral realtime profile.
ALTER TABLE transcription_profiles
    ADD COLUMN coral_stable_preview BOOLEAN NOT NULL DEFAULT false,
    ADD CONSTRAINT transcription_profiles_coral_preview_mode CHECK (
        NOT coral_stable_preview OR (purpose = 'live' AND engine = 'cloud' AND realtime)
    );
