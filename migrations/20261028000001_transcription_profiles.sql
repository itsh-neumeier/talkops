-- Transcription per purpose (ADR 0024): several cloud connections and one
-- profile per purpose (live preview, recording and voicemail, each with an
-- optional second pass). The old tenant_settings columns stay in place but
-- are no longer read.

-- OpenAI or an OpenAI-compatible server (Groq, Mistral, Speaches, ...).
CREATE TABLE transcription_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
    kind TEXT NOT NULL CHECK (kind IN ('openai', 'compatible')),
    url TEXT NOT NULL,
    -- Encrypted with TALKOPS_SECRET_KEY (ADR 0008); NULL = no key.
    key_enc TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name),
    UNIQUE (tenant_id, id)
);

-- A missing row means the built-in default for that purpose.
CREATE TABLE transcription_profiles (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    purpose TEXT NOT NULL CHECK (purpose IN
        ('live', 'recording', 'recording_refine', 'voicemail', 'voicemail_refine')),
    engine TEXT NOT NULL CHECK (engine IN ('off', 'local', 'cloud')),
    local_model TEXT NOT NULL DEFAULT 'fast'
        CHECK (local_model IN ('base', 'fast', 'accurate', 'best', 'german')),
    provider_id UUID,
    model TEXT NOT NULL DEFAULT '',
    -- Live only: stream audio over the Realtime API instead of sending chunks.
    realtime BOOLEAN NOT NULL DEFAULT false,
    -- Estimated cost per audio minute; NULL = the built-in list price.
    price_per_minute DOUBLE PRECISION CHECK (price_per_minute >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, purpose),
    FOREIGN KEY (tenant_id, provider_id) REFERENCES transcription_providers(tenant_id, id),
    CHECK (engine <> 'cloud' OR (provider_id IS NOT NULL AND model <> '')),
    CHECK (engine <> 'off' OR purpose IN ('recording_refine', 'voicemail_refine'))
);

-- Audio sent to cloud providers, per day, purpose and engine.
CREATE TABLE transcription_usage (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    day DATE NOT NULL,
    purpose TEXT NOT NULL,
    -- "<provider name>/<model>" at the time of use.
    engine TEXT NOT NULL,
    seconds DOUBLE PRECISION NOT NULL DEFAULT 0,
    cost DOUBLE PRECISION NOT NULL DEFAULT 0,
    -- Audio without a known price; not part of cost.
    unpriced_seconds DOUBLE PRECISION NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, day, purpose, engine)
);

-- A realtime stream belongs to one worker at a time.
ALTER TABLE live_transcript_previews
    ADD COLUMN stream_owner TEXT,
    ADD COLUMN stream_beat TIMESTAMPTZ,
    ADD COLUMN stream_failed_at TIMESTAMPTZ;

-- Copy the existing API settings into a first connection.
INSERT INTO transcription_providers (tenant_id, name, kind, url, key_enc)
SELECT tenant_id,
       CASE WHEN transcription_api_url = 'https://api.openai.com/v1' THEN 'OpenAI' ELSE 'API' END,
       CASE WHEN transcription_api_url = 'https://api.openai.com/v1' THEN 'openai' ELSE 'compatible' END,
       transcription_api_url, transcription_api_key_enc
FROM tenant_settings
WHERE transcription_api_key_enc IS NOT NULL
   OR transcription_quality = 'api'
   OR transcription_refine = 'api';

-- Copy both passes to recordings and voicemails. A second pass equal to the
-- first one never ran, so it becomes 'off'.
INSERT INTO transcription_profiles (tenant_id, purpose, engine, local_model, provider_id, model)
SELECT s.tenant_id, p.purpose,
       CASE WHEN p.value = '' OR (p.refine AND s.transcription_refine = s.transcription_quality) THEN 'off'
            WHEN p.value = 'api' THEN 'cloud'
            ELSE 'local' END,
       CASE WHEN p.value IN ('fast', 'accurate', 'best', 'german') THEN p.value ELSE 'fast' END,
       CASE WHEN p.value = 'api' THEN tp.id END,
       CASE WHEN p.value = 'api' THEN s.transcription_api_model ELSE '' END
FROM tenant_settings s
CROSS JOIN LATERAL (VALUES
    ('recording', s.transcription_quality, false),
    ('voicemail', s.transcription_quality, false),
    ('recording_refine', s.transcription_refine, true),
    ('voicemail_refine', s.transcription_refine, true)
) AS p(purpose, value, refine)
LEFT JOIN transcription_providers tp ON tp.tenant_id = s.tenant_id;
