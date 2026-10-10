-- Comfort settings for provisioned phones (key tone, display …), keys from
-- the phone catalog (presets/phones/*.yaml). Unset keys leave the phone's
-- own value alone; a phone's value wins over the tenant-wide one.
ALTER TABLE phones
    ADD COLUMN settings JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(settings) = 'object');
ALTER TABLE tenant_settings
    ADD COLUMN phone_settings JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(phone_settings) = 'object');
