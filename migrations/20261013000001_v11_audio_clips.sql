-- 1.1: audio clips. A clip is generated (Piper), uploaded or recorded in the
-- browser; greetings and menu prompts reference clips instead of keeping
-- their own text and file. Files: sounds volume, clips/<tenant>/<id>.wav.

CREATE TABLE audio_clips (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    source      TEXT NOT NULL CHECK (source IN ('tts', 'upload', 'recording')),
    -- Generated clips: text, language and voice (1 or 2, see prompts::voices).
    text        TEXT NOT NULL DEFAULT '' CHECK (length(text) <= 1000),
    language    TEXT NOT NULL DEFAULT 'de' CHECK (language IN ('de', 'en')),
    voice       SMALLINT NOT NULL DEFAULT 1 CHECK (voice IN (1, 2)),
    status      TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'ready', 'failed')),
    duration_ms INTEGER NOT NULL DEFAULT 0,
    created_by  UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX audio_clips_tenant_idx ON audio_clips (tenant_id, created_at);

-- Voicemail: greeting from a clip, or no greeting (straight to the beep).
ALTER TABLE voicemail_boxes DROP CONSTRAINT voicemail_boxes_greeting_check;
ALTER TABLE voicemail_boxes
    ADD CONSTRAINT voicemail_boxes_greeting_check
        CHECK (greeting IN ('default', 'tts', 'recorded', 'clip', 'none')),
    ADD COLUMN greeting_clip_id UUID REFERENCES audio_clips (id) ON DELETE SET NULL;

