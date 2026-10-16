-- Mail notifications (ADR 0035): producers write events into an outbox; a
-- background worker matches them with rules and sends mails.

-- Outbox of events. `source` identifies the affected object (gateway name,
-- SIP user, profile ...) so that a recovery event can close a problem event.
CREATE TABLE notification_events (
    id            BIGSERIAL PRIMARY KEY,
    tenant_id     UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    event_type    TEXT NOT NULL CHECK (event_type ~ '^[a-z][a-z0-9_.]{0,63}$'),
    source        TEXT NOT NULL DEFAULT '' CHECK (length(source) <= 200),
    severity      TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    -- Values for the mail text; never secrets.
    params        JSONB NOT NULL DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at  TIMESTAMPTZ
);
CREATE INDEX notification_events_pending_idx ON notification_events (id) WHERE processed_at IS NULL;
CREATE INDEX notification_events_recent_idx ON notification_events (tenant_id, created_at DESC);

-- One rule per event type. No row: the defaults of the event catalog
-- (talkops_core::notifications::CATALOG) with the admin role as recipient.
CREATE TABLE notification_rules (
    tenant_id           UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    event_type          TEXT NOT NULL CHECK (event_type ~ '^[a-z][a-z0-9_.]{0,63}$'),
    enabled             BOOLEAN NOT NULL,
    min_severity        TEXT NOT NULL DEFAULT 'info' CHECK (min_severity IN ('info', 'warning', 'critical')),
    recipient_users     UUID[] NOT NULL DEFAULT '{}',
    recipient_roles     UUID[] NOT NULL DEFAULT '{}',
    recipient_emails    TEXT[] NOT NULL DEFAULT '{}',
    -- Problem events: wait this long; a recovery in this time cancels the mail.
    delay_secs          INTEGER NOT NULL DEFAULT 0 CHECK (delay_secs BETWEEN 0 AND 86400),
    -- Problem events: send a mail when the problem ends.
    send_recovery       BOOLEAN NOT NULL DEFAULT true,
    -- > 0: collect events and send one digest mail this long after the first.
    digest_secs         INTEGER NOT NULL DEFAULT 0 CHECK (digest_secs BETWEEN 0 AND 86400),
    -- Mails per rule and hour; more are logged as rate limited.
    rate_limit_per_hour INTEGER NOT NULL DEFAULT 20 CHECK (rate_limit_per_hour BETWEEN 1 AND 1000),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, event_type)
);

-- Open alerts: problem events waiting for their delay (`pending`), problems
-- that were reported (`alerted`, closed by the recovery event) and digests
-- that collect events (`digest`, source '').
CREATE TABLE notification_alerts (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    event_type  TEXT NOT NULL,
    source      TEXT NOT NULL DEFAULT '',
    kind        TEXT NOT NULL CHECK (kind IN ('pending', 'alerted', 'digest')),
    event_ids   BIGINT[] NOT NULL DEFAULT '{}',
    due_at      TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, event_type, source)
);
CREATE INDEX notification_alerts_due_idx ON notification_alerts (due_at) WHERE kind IN ('pending', 'digest');

-- Delivery log.
CREATE TABLE notification_deliveries (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    event_type  TEXT NOT NULL,
    -- alert, recovery, digest or test.
    kind        TEXT NOT NULL CHECK (kind IN ('alert', 'recovery', 'digest', 'test')),
    subject     TEXT NOT NULL DEFAULT '',
    recipients  TEXT[] NOT NULL DEFAULT '{}',
    status      TEXT NOT NULL CHECK (status IN ('sent', 'failed', 'rate_limited', 'no_recipients', 'no_smtp')),
    error       TEXT,
    event_count INTEGER NOT NULL DEFAULT 1,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX notification_deliveries_recent_idx ON notification_deliveries (tenant_id, created_at DESC);
CREATE INDEX notification_deliveries_rate_idx ON notification_deliveries (tenant_id, event_type, created_at) WHERE status = 'sent';

-- Addresses that a user logged in from (alert on an admin login from a new address).
CREATE TABLE login_addresses (
    tenant_id   UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    ip          TEXT NOT NULL,
    first_seen  TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, ip)
);
