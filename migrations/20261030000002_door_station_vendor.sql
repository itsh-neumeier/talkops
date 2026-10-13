-- Manufacturer of a door station (ADR 0026). It selects the HTTP integration
-- (door opener, snapshots, events) and the setup guide. All stations before
-- this migration are Dahua stations.
CREATE TYPE door_vendor AS ENUM ('dahua');

ALTER TABLE door_stations
    ADD COLUMN vendor door_vendor NOT NULL DEFAULT 'dahua';
