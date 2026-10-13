-- Codec profile per device (ADR 0023). NULL = automatic from the device kind:
-- Wi-Fi handsets use 'hd', all other devices 'standard'. FreeSWITCH
-- transcodes between the device's codec and the codec of the other party.
CREATE TYPE codec_profile AS ENUM ('standard', 'hd', 'robust');

ALTER TABLE devices ADD COLUMN codec_profile codec_profile;
