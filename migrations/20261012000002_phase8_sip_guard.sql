-- Phase 8: protection against SIP password guessing. FreeSWITCH reports
-- failed registrations and calls; an address with too many failures is
-- banned for a while (TalkOps then refuses its directory lookups).

CREATE TABLE sip_guard_settings (
    tenant_id        UUID PRIMARY KEY REFERENCES tenants (id) ON DELETE CASCADE,
    enabled          BOOLEAN NOT NULL DEFAULT true,
    -- Failed attempts within `window_minutes` that lead to a ban.
    max_failures     INTEGER NOT NULL DEFAULT 10 CHECK (max_failures BETWEEN 3 AND 1000),
    window_minutes   INTEGER NOT NULL DEFAULT 10 CHECK (window_minutes BETWEEN 1 AND 1440),
    ban_minutes      INTEGER NOT NULL DEFAULT 60 CHECK (ban_minutes BETWEEN 1 AND 525600),
    -- Addresses/networks that are never banned (e.g. the office LAN).
    trusted_networks INET[] NOT NULL DEFAULT '{}',
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO sip_guard_settings (tenant_id) SELECT id FROM tenants;

CREATE TABLE sip_bans (
    tenant_id    UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    ip           INET NOT NULL,
    failures     INTEGER NOT NULL,
    -- SIP user of the last failed attempt (for the admin's information).
    last_user    TEXT NOT NULL DEFAULT '',
    banned_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    banned_until TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (tenant_id, ip)
);
