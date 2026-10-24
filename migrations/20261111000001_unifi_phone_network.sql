-- "Show network data on phones": the phone list reads the network state of
-- provisioned phones from the UniFi connection. Off by default.
ALTER TABLE unifi_connections
    ADD COLUMN phone_network BOOLEAN NOT NULL DEFAULT false;
