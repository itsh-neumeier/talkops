-- Call blocking: own list of numbers and prefixes, anonymous callers, and
-- the PhoneBlock community spam list (phoneblock.net).
CREATE TABLE call_blocks (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    -- E.164 number ("+4930123456") or prefix ending in "*" ("+49900*").
    pattern    TEXT NOT NULL,
    label      TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, pattern)
);

CREATE TABLE call_block_settings (
    tenant_id            UUID PRIMARY KEY REFERENCES tenants (id) ON DELETE CASCADE,
    block_anonymous      BOOLEAN NOT NULL DEFAULT false,
    phoneblock_enabled   BOOLEAN NOT NULL DEFAULT false,
    phoneblock_token_enc TEXT,
    -- Spam reports needed before a number is blocked.
    phoneblock_min_votes INTEGER NOT NULL DEFAULT 4 CHECK (phoneblock_min_votes BETWEEN 1 AND 100),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

