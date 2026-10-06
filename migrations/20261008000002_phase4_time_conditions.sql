-- Phase 4: time conditions (business hours, holidays, manual override).

CREATE TABLE time_conditions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    number          TEXT CHECK (number ~ '^[1-9][0-9]{1,7}$'),
    name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    -- Opening hours per weekday: {"mon": [["08:00", "12:00"], ["13:00", "17:00"]], ...}
    schedule        JSONB NOT NULL DEFAULT '{}',
    -- Public holidays count as closed: 'DE' or 'DE-XX'; NULL = ignore holidays.
    holiday_region  TEXT,
    -- Additional closed days (company holidays, bridge days).
    closed_dates    DATE[] NOT NULL DEFAULT '{}',
    -- auto: follow the schedule; open/closed: forced (e.g. emergency service).
    override        TEXT NOT NULL DEFAULT 'auto' CHECK (override IN ('auto', 'open', 'closed')),
    open_type       number_destination NOT NULL DEFAULT 'none',
    open_id         UUID,
    closed_type     number_destination NOT NULL DEFAULT 'none',
    closed_id       UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX time_conditions_number_idx ON time_conditions (tenant_id, number);
