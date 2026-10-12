-- Each user has a separate browser SIP identity on each selected extension.
ALTER TABLE devices ADD COLUMN browser_user_id UUID REFERENCES users (id) ON DELETE CASCADE;

-- Keep the first existing browser identity for the assigned extension owner.
WITH owned AS (
    SELECT d.id, e.user_id,
           row_number() OVER (PARTITION BY d.extension_id ORDER BY d.created_at, d.id) AS position
    FROM devices d JOIN extensions e ON e.id = d.extension_id AND e.tenant_id = d.tenant_id
    WHERE d.kind = 'browser' AND e.user_id IS NOT NULL
)
UPDATE devices d SET browser_user_id = owned.user_id
FROM owned WHERE owned.id = d.id AND owned.position = 1;

CREATE UNIQUE INDEX devices_browser_user_idx
    ON devices (tenant_id, extension_id, browser_user_id)
    WHERE kind = 'browser' AND browser_user_id IS NOT NULL;
