-- Phase 4: generic call destinations, ring groups and a shared internal
-- number space.

-- Destinations a call can be sent to (numbers, fallbacks, menus, schedules).
-- New enum values cannot be used in the transaction that adds them.
ALTER TYPE number_destination ADD VALUE IF NOT EXISTS 'ring_group';
ALTER TYPE number_destination ADD VALUE IF NOT EXISTS 'voicemail';
ALTER TYPE number_destination ADD VALUE IF NOT EXISTS 'time_condition';
ALTER TYPE number_destination ADD VALUE IF NOT EXISTS 'ivr';
ALTER TYPE number_destination ADD VALUE IF NOT EXISTS 'queue';

CREATE TABLE ring_groups (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    -- Internal number; NULL = only reachable via phone numbers or menus.
    number            TEXT CHECK (number ~ '^[1-9][0-9]{1,7}$'),
    name              TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    -- simultaneous: all members at once; sequential: one after another.
    strategy          TEXT NOT NULL DEFAULT 'simultaneous'
                      CHECK (strategy IN ('simultaneous', 'sequential')),
    -- Total ring time (simultaneous) or per member (sequential).
    ring_timeout_secs INTEGER NOT NULL DEFAULT 25 CHECK (ring_timeout_secs BETWEEN 5 AND 300),
    -- Shown before the caller's name, e.g. "Support: ".
    caller_id_prefix  TEXT NOT NULL DEFAULT '' CHECK (length(caller_id_prefix) <= 20),
    -- Where unanswered calls go.
    fallback_type     number_destination NOT NULL DEFAULT 'none',
    fallback_id       UUID,
    enabled           BOOLEAN NOT NULL DEFAULT true,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX ring_groups_number_idx ON ring_groups (tenant_id, number);

CREATE TABLE ring_group_members (
    group_id     UUID NOT NULL REFERENCES ring_groups (id) ON DELETE CASCADE,
    extension_id UUID NOT NULL REFERENCES extensions (id) ON DELETE CASCADE,
    position     INTEGER NOT NULL,
    PRIMARY KEY (group_id, extension_id)
);
