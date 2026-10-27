-- One event per originating call, including concurrent XML lookup retries.
CREATE UNIQUE INDEX notification_emergency_call_once
    ON notification_events (tenant_id, event_type, source)
    WHERE event_type = 'calls.emergency_dialed';
