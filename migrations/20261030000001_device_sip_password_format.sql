-- Format of generated SIP passwords per device. 'standard' = 20 characters
-- (the format before this migration), 'compact15' = 15 characters for
-- devices with a length limit (e.g. Dahua VTO/VTH). The format applies when
-- TalkOps generates a password; it never changes a stored password.
CREATE TYPE sip_password_format AS ENUM ('standard', 'compact15');

ALTER TABLE devices
    ADD COLUMN sip_password_format sip_password_format NOT NULL DEFAULT 'standard';
