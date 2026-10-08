-- 1.1: Smart Attendant. A voice menu becomes a call flow: a tree of steps
-- (play audio, keypress menu, ring phones, schedule, voicemail, park,
-- forward, go to, hang up) stored as JSON, see talkops_core::attendant.
-- Existing menus are converted: their greeting becomes an audio clip with
-- the menu's id (the server moves the file on start), their options become
-- "forward" steps.

ALTER TABLE ivr_menus
    ADD COLUMN flow JSONB,
    -- Clips used by the flow (kept in sync on save; protects them from cleanup).
    ADD COLUMN clip_ids UUID[] NOT NULL DEFAULT '{}';

INSERT INTO audio_clips (id, tenant_id, source, text, language, status)
SELECT id, tenant_id,
       CASE greeting WHEN 'tts' THEN 'tts' ELSE 'upload' END,
       CASE greeting WHEN 'tts' THEN left(greeting_text, 1000) ELSE '' END,
       COALESCE(language, 'de'), 'ready'
FROM ivr_menus
WHERE greeting_status = 'ready';

UPDATE ivr_menus m SET
    clip_ids = CASE WHEN greeting_status = 'ready' THEN ARRAY[id] ELSE '{}' END,
    flow = jsonb_build_object(
        'type', 'menu',
        'id', 'start',
        'clip_id', CASE WHEN greeting_status = 'ready' THEN to_jsonb(id) ELSE 'null'::jsonb END,
        'timeout_secs', timeout_secs,
        'max_tries', max_tries,
        'direct_dial', direct_dial,
        'options', COALESCE((
            SELECT jsonb_agg(jsonb_build_object(
                'digit', o->>'digit',
                'next', jsonb_build_object(
                    'type', 'transfer',
                    'id', 'key-' || ord,
                    'destination_type', o->'type',
                    'destination_id', o->'id')) ORDER BY ord)
            FROM jsonb_array_elements(m.options) WITH ORDINALITY AS t(o, ord)
        ), '[]'::jsonb),
        'timeout', CASE WHEN timeout_type = 'none' THEN 'null'::jsonb ELSE jsonb_build_object(
            'type', 'transfer',
            'id', 'no-input',
            'destination_type', timeout_type::text,
            'destination_id', timeout_id) END);

ALTER TABLE ivr_menus
    ALTER COLUMN flow SET NOT NULL,
    DROP COLUMN greeting,
    DROP COLUMN greeting_text,
    DROP COLUMN greeting_status,
    DROP COLUMN timeout_secs,
    DROP COLUMN max_tries,
    DROP COLUMN direct_dial,
    DROP COLUMN options,
    DROP COLUMN timeout_type,
    DROP COLUMN timeout_id;

-- Rendering jobs of the old menu greetings are obsolete.
DELETE FROM jobs WHERE kind = 'tts.ivr' AND status IN ('queued', 'failed');
