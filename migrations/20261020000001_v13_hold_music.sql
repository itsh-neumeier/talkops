-- Selectable music on hold: one of the built-in pieces ('' = all, shuffled)
-- or an own audio clip.
ALTER TABLE tenant_settings
    ADD COLUMN hold_music TEXT NOT NULL DEFAULT '',
    ADD COLUMN hold_music_clip_id UUID REFERENCES audio_clips (id) ON DELETE SET NULL;
