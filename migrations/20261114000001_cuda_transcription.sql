-- Speech recognition on an NVIDIA GPU (ADR 0052).

-- A connection to the cuda-stt service on this server. Like `coral`, it
-- uses a loopback URL, has no costs and falls back to local Whisper.
ALTER TABLE transcription_providers DROP CONSTRAINT transcription_providers_kind_check;
ALTER TABLE transcription_providers ADD CONSTRAINT transcription_providers_kind_check
    CHECK (kind IN ('openai', 'compatible', 'coral', 'cuda'));
