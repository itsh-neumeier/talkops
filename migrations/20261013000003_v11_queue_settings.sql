-- 1.1: call queue settings like UniFi Talk: greeting and music on hold
-- (audio clips), a limit of waiting callers with an overflow destination,
-- business hours with an after-hours destination, and a voicemail for
-- several recipients when nobody answers in time.

ALTER TABLE queues
    ADD COLUMN greeting_clip_id     UUID REFERENCES audio_clips (id) ON DELETE SET NULL,
    ADD COLUMN moh_clip_id          UUID REFERENCES audio_clips (id) ON DELETE SET NULL,
    -- Callers waiting at most (0 = no limit); more go to the overflow destination.
    ADD COLUMN max_callers          INTEGER NOT NULL DEFAULT 0 CHECK (max_callers BETWEEN 0 AND 500),
    ADD COLUMN overflow_type        number_destination NOT NULL DEFAULT 'none',
    ADD COLUMN overflow_id          UUID,
    -- Business hours; outside them calls go to the closed destination.
    ADD COLUMN time_condition_id    UUID REFERENCES time_conditions (id) ON DELETE SET NULL,
    ADD COLUMN closed_type          number_destination NOT NULL DEFAULT 'none',
    ADD COLUMN closed_id            UUID,
    -- Not answered in time: a message for these extensions instead of the
    -- timeout destination.
    ADD COLUMN voicemail_recipients UUID[] NOT NULL DEFAULT '{}',
    ADD COLUMN voicemail_clip_id    UUID REFERENCES audio_clips (id) ON DELETE SET NULL;
