-- Optional tile server of the call map (ADR 0044). Empty: offline base map.
ALTER TABLE tenant_settings
    ADD COLUMN map_tile_url TEXT NOT NULL DEFAULT ''
        CHECK (map_tile_url = '' OR (map_tile_url LIKE 'https://%' AND length(map_tile_url) <= 500)),
    ADD COLUMN map_tile_attribution TEXT NOT NULL DEFAULT ''
        CHECK (length(map_tile_attribution) <= 300);
