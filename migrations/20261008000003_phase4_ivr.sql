-- Phase 4: IVR menus ("press 1 for sales").

CREATE TABLE ivr_menus (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    number          TEXT CHECK (number ~ '^[1-9][0-9]{1,7}$'),
    name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    -- Prompt language for TTS and system prompts; NULL = tenant default.
    language        TEXT CHECK (language IN ('de', 'en')),
    -- tts: greeting_text rendered by Piper; upload: WAV uploaded in the UI.
    greeting        TEXT NOT NULL DEFAULT 'tts' CHECK (greeting IN ('tts', 'upload')),
    greeting_text   TEXT NOT NULL DEFAULT '' CHECK (length(greeting_text) <= 2000),
    greeting_status TEXT NOT NULL DEFAULT 'none'
                    CHECK (greeting_status IN ('none', 'pending', 'ready', 'failed')),
    -- Seconds to wait for input after the greeting.
    timeout_secs    INTEGER NOT NULL DEFAULT 5 CHECK (timeout_secs BETWEEN 1 AND 30),
    max_tries       INTEGER NOT NULL DEFAULT 3 CHECK (max_tries BETWEEN 1 AND 10),
    -- Callers may dial internal numbers directly.
    direct_dial     BOOLEAN NOT NULL DEFAULT false,
    -- [{"digit": "1", "type": "ring_group", "id": "…"}, …]
    options         JSONB NOT NULL DEFAULT '[]',
    -- No (valid) input after max_tries.
    timeout_type    number_destination NOT NULL DEFAULT 'none',
    timeout_id      UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX ivr_menus_number_idx ON ivr_menus (tenant_id, number);
