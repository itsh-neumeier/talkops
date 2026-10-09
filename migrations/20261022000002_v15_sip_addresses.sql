-- Calls by SIP address (user@domain) and provider accounts without a
-- phone number (e.g. sip2sip/SIP Thor).

-- Inbound calls to an account that match none of its numbers (accounts
-- without numbers) go to this destination.
ALTER TABLE trunk_accounts
    ADD COLUMN destination_type number_destination NOT NULL DEFAULT 'none',
    ADD COLUMN destination_id UUID;

-- Extensions may dial SIP addresses: through a trunk account of the same
-- domain, otherwise directly over the internet.
ALTER TABLE tenant_settings
    ADD COLUMN sip_uri_dialing BOOLEAN NOT NULL DEFAULT true;
