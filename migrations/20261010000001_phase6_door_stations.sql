-- Phase 6: door stations (Dahua VTO). The station registers as a device of
-- an extension; TalkOps routes its calls and talks to its HTTP API.

CREATE TABLE door_stations (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id        UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    name             TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    -- The extension whose device is the door station.
    extension_id     UUID NOT NULL UNIQUE REFERENCES extensions (id) ON DELETE CASCADE,
    -- HTTP API (empty host: SIP only, no door opener/snapshots/events).
    host             TEXT NOT NULL DEFAULT '' CHECK (host ~ '^[A-Za-z0-9.:\-\[\]]{0,253}$'),
    port             INTEGER NOT NULL DEFAULT 80 CHECK (port BETWEEN 1 AND 65535),
    username         TEXT NOT NULL DEFAULT 'admin' CHECK (length(username) <= 64),
    password_enc     TEXT,
    -- Number of locks (1 = own relay, 2 = second lock via RS-485).
    doors            SMALLINT NOT NULL DEFAULT 1 CHECK (doors BETWEEN 1 AND 2),
    -- Where a ring goes; `buttons` maps dialed numbers of multi-button
    -- stations to other destinations: [{"number","type","id"}].
    destination_type number_destination NOT NULL DEFAULT 'none',
    destination_id   UUID,
    buttons          JSONB NOT NULL DEFAULT '[]',
    events_enabled   BOOLEAN NOT NULL DEFAULT true,
    snapshots        BOOLEAN NOT NULL DEFAULT true,
    -- Home Assistant (or any) webhook, encrypted (the URL contains a secret).
    webhook_url_enc  TEXT,
    -- SHA-256 of the token that may open the door via /hooks/door/<id>/open.
    api_token_hash   TEXT,
    -- Live state from the event stream.
    online           BOOLEAN NOT NULL DEFAULT false,
    model            TEXT NOT NULL DEFAULT '',
    last_seen        TIMESTAMPTZ,
    enabled          BOOLEAN NOT NULL DEFAULT true,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX door_stations_tenant_idx ON door_stations (tenant_id);

CREATE TABLE door_events (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    door_station_id UUID NOT NULL REFERENCES door_stations (id) ON DELETE CASCADE,
    -- ring: button pressed (call routed); open_command: TalkOps sent "open";
    -- opened: the station reports an unlock; door_open/door_closed: door
    -- contact; unlock_failed; alarm (tamper); online/offline.
    kind            TEXT NOT NULL CHECK (kind IN ('ring', 'open_command', 'opened', 'door_open',
                                                  'door_closed', 'unlock_failed', 'alarm',
                                                  'online', 'offline')),
    detail          JSONB NOT NULL DEFAULT '{}',
    -- JPEG below the snapshots volume.
    snapshot        TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX door_events_station_idx ON door_events (door_station_id, created_at DESC);
CREATE INDEX door_events_created_idx ON door_events (created_at);
