-- Video calls only where switched on: per extension (internal calls) and
-- per trunk (calls to the provider). Off by default; calls are audio-only
-- otherwise. Door stations always send their video.
ALTER TABLE extensions ADD COLUMN video_enabled BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE trunks ADD COLUMN video_enabled BOOLEAN NOT NULL DEFAULT false;
