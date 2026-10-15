-- Media settings per device (ADR 0027).
--
-- 'g711': audio profile with G.711 only (PCMA, PCMU) and no jitter buffer,
-- for devices that support only G.711 well (e.g. Dahua door stations).
ALTER TYPE codec_profile ADD VALUE IF NOT EXISTS 'g711';

-- Video on the device's own leg. NULL = automatic: 'auto' for devices with
-- the standard codec profile, 'off' for all other profiles (the behaviour
-- before this migration). 'auto' offers the video codecs the call allows,
-- 'h264' and 'vp8' only that codec, 'off' no video.
CREATE TYPE device_video AS ENUM ('auto', 'off', 'h264', 'vp8');

ALTER TABLE devices ADD COLUMN video device_video;
