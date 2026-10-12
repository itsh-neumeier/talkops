ALTER TABLE voicemail_boxes
    ADD COLUMN speech_speed DOUBLE PRECISION NOT NULL DEFAULT 1.0
    CHECK (speech_speed BETWEEN 0.5 AND 2.0);

ALTER TABLE audio_clips
    ADD COLUMN speech_speed DOUBLE PRECISION NOT NULL DEFAULT 1.0
    CHECK (speech_speed BETWEEN 0.5 AND 2.0);
