-- Keep the decision made when the inbound call arrived.
ALTER TABLE cdr ADD COLUMN blocked_reason TEXT;
ALTER TABLE cdr ADD CONSTRAINT cdr_blocked_reason_valid CHECK (
    blocked_reason IS NULL OR (
        direction = 'inbound' AND (
            blocked_reason = 'anonymous'
            OR blocked_reason ~ '^list:\+[0-9]{3,16}\*?$'
            OR blocked_reason ~ '^phoneblock:[0-9]{1,10}$'
        )
    )
);

CREATE TABLE call_allowlist (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    number TEXT NOT NULL CHECK (number ~ '^\+[1-9][0-9]{2,14}$'),
    label TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, number)
);
-- blocking.allowlist is granted through the permission catalog to admins.
-- Do not grant it to existing non-admin roles.
