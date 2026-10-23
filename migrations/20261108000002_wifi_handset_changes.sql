-- Handset changes in the Wi-Fi change history (ADR 0043). A handset row
-- stores the roaming threshold of one phone before and after the change.

ALTER TABLE unifi_wifi_changes
    ADD COLUMN target TEXT NOT NULL DEFAULT 'ssid' CHECK (target IN ('ssid', 'handset')),
    ADD COLUMN phone_id UUID REFERENCES phones (id) ON DELETE SET NULL,
    ADD COLUMN phone_name TEXT NOT NULL DEFAULT '',
    ALTER COLUMN site_id DROP NOT NULL,
    ALTER COLUMN wifi_id DROP NOT NULL,
    ADD CONSTRAINT unifi_wifi_changes_ssid_target_check
        CHECK (target = 'handset' OR (site_id IS NOT NULL AND wifi_id IS NOT NULL));

-- `waiting`: TalkOps stored the handset value; the handset has not fetched
-- its configuration yet.
ALTER TABLE unifi_wifi_changes DROP CONSTRAINT unifi_wifi_changes_state_check;
ALTER TABLE unifi_wifi_changes ADD CONSTRAINT unifi_wifi_changes_state_check
    CHECK (state IN ('applying', 'waiting', 'applied', 'failed', 'rolled_back'));
CREATE INDEX unifi_wifi_changes_waiting_idx ON unifi_wifi_changes (phone_id)
    WHERE state = 'waiting';
