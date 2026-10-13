-- DTMF unlock codes of door stations, one per lock. TalkOps detects the code
-- during a ring call or a direct call to the station and opens the door
-- through the HTTP API. The codes open a door: stored encrypted
-- (TALKOPS_SECRET_KEY, ADR 0008), NULL = no code.
ALTER TABLE door_stations
    ADD COLUMN unlock_code_1_enc TEXT,
    ADD COLUMN unlock_code_2_enc TEXT;
