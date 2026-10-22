-- Wi-Fi voice survey (ADR 0041). Data exists only for started surveys and
-- is deleted after the retention period of the tenant.

CREATE TABLE wifi_survey_settings (
    tenant_id UUID PRIMARY KEY REFERENCES tenants (id) ON DELETE CASCADE,
    clean_ms INT NOT NULL DEFAULT 50 CHECK (clean_ms BETWEEN 10 AND 1000),
    bad_ms INT NOT NULL DEFAULT 150 CHECK (bad_ms BETWEEN 20 AND 5000),
    loss_block_pct INT NOT NULL DEFAULT 5 CHECK (loss_block_pct BETWEEN 1 AND 100),
    sticky_rssi INT NOT NULL DEFAULT -75 CHECK (sticky_rssi BETWEEN -100 AND -30),
    sticky_secs INT NOT NULL DEFAULT 10 CHECK (sticky_secs BETWEEN 2 AND 600),
    pingpong_count INT NOT NULL DEFAULT 3 CHECK (pingpong_count BETWEEN 2 AND 20),
    pingpong_secs INT NOT NULL DEFAULT 60 CHECK (pingpong_secs BETWEEN 5 AND 600),
    weak_rssi INT NOT NULL DEFAULT -80 CHECK (weak_rssi BETWEEN -100 AND -30),
    weak_secs INT NOT NULL DEFAULT 5 CHECK (weak_secs BETWEEN 1 AND 600),
    retention_days INT NOT NULL DEFAULT 30 CHECK (retention_days BETWEEN 1 AND 365),
    max_minutes INT NOT NULL DEFAULT 60 CHECK (max_minutes BETWEEN 1 AND 240),
    max_parallel INT NOT NULL DEFAULT 3 CHECK (max_parallel BETWEEN 1 AND 20),
    CHECK (clean_ms < bad_ms),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE wifi_surveys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    device_id UUID REFERENCES devices (id) ON DELETE SET NULL,
    extension_id UUID REFERENCES extensions (id) ON DELETE SET NULL,
    phone_id UUID REFERENCES phones (id) ON DELETE SET NULL,
    -- Snapshots: the survey stays readable after device changes.
    device_label TEXT NOT NULL DEFAULT '',
    mac TEXT CHECK (mac ~ '^[0-9a-f]{12}$'),
    label TEXT NOT NULL DEFAULT '' CHECK (length(label) <= 120),
    mode TEXT NOT NULL CHECK (mode IN ('originate', 'self_dial')),
    state TEXT NOT NULL CHECK (state IN ('armed', 'calling', 'running', 'analysing',
                                         'analysed', 'failed', 'cancelled')),
    call_uuid UUID UNIQUE,
    ptime_ms INT NOT NULL DEFAULT 20 CHECK (ptime_ms BETWEEN 10 AND 120),
    codec TEXT,
    recording_file TEXT,
    recording_secs INT,
    started_by UUID REFERENCES users (id) ON DELETE SET NULL,
    armed_until TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    answered_at TIMESTAMPTZ,
    ended_at TIMESTAMPTZ,
    hangup_cause TEXT,
    thresholds JSONB NOT NULL DEFAULT '{}',
    unifi JSONB NOT NULL DEFAULT '{}',
    summary JSONB,
    error TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX wifi_surveys_tenant_idx ON wifi_surveys (tenant_id, created_at DESC);
CREATE INDEX wifi_surveys_expiry_idx ON wifi_surveys (expires_at);
-- One active survey for each device.
CREATE UNIQUE INDEX wifi_surveys_active_device_idx ON wifi_surveys (device_id)
    WHERE state IN ('armed', 'calling', 'running');

-- Uplink media values for each poll (differences to the previous poll).
CREATE TABLE wifi_survey_media_samples (
    survey_id UUID NOT NULL REFERENCES wifi_surveys (id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    offset_ms INT NOT NULL,
    at TIMESTAMPTZ NOT NULL,
    interval_ms INT NOT NULL,
    received INT NOT NULL,
    expected INT NOT NULL,
    flaws INT NOT NULL,
    mos REAL,
    jitter_max_ms REAL,
    -- Latest RTCP receiver report of the handset (downlink).
    rtcp_fraction_lost REAL,
    rtcp_jitter_ms REAL,
    PRIMARY KEY (survey_id, offset_ms)
);

-- Wi-Fi state of the handset from UniFi for each poll.
CREATE TABLE wifi_survey_radio_samples (
    survey_id UUID NOT NULL REFERENCES wifi_surveys (id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    offset_ms INT NOT NULL,
    at TIMESTAMPTZ NOT NULL,
    ap_mac TEXT,
    ap_name TEXT,
    bssid TEXT,
    radio TEXT,
    channel INT,
    rssi INT,
    tx_rate_kbps INT,
    rx_rate_kbps INT,
    error TEXT,
    PRIMARY KEY (survey_id, offset_ms)
);

-- Markers, error periods and state changes.
CREATE TABLE wifi_survey_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    survey_id UUID NOT NULL REFERENCES wifi_surveys (id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    offset_ms INT NOT NULL,
    at TIMESTAMPTZ NOT NULL DEFAULT now(),
    kind TEXT NOT NULL CHECK (kind IN ('marker', 'error_period', 'state')),
    -- Marker: `disturbance`, `location` or `note`.
    marker TEXT CHECK (marker IN ('disturbance', 'location', 'note')),
    source TEXT CHECK (source IN ('dtmf', 'ui')),
    note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 200),
    data JSONB NOT NULL DEFAULT '{}'
);
CREATE INDEX wifi_survey_events_survey_idx ON wifi_survey_events (survey_id, offset_ms);

-- Result of the analysis job; replaced on each run.
CREATE TABLE wifi_survey_roams (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    survey_id UUID NOT NULL REFERENCES wifi_surveys (id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    offset_ms INT NOT NULL,
    from_ap_mac TEXT,
    from_ap_name TEXT,
    to_ap_mac TEXT,
    to_ap_name TEXT,
    from_radio TEXT,
    to_radio TEXT,
    from_channel INT,
    to_channel INT,
    rssi_before INT,
    rssi_after INT,
    gap_ms INT NOT NULL,
    loss_pct REAL NOT NULL,
    jitter_ms REAL,
    rating TEXT NOT NULL CHECK (rating IN ('clean', 'audible', 'bad')),
    flags TEXT[] NOT NULL DEFAULT '{}',
    markers INT NOT NULL DEFAULT 0
);
CREATE INDEX wifi_survey_roams_survey_idx ON wifi_survey_roams (survey_id, offset_ms);
