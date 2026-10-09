-- Editable voicemail control: menu keys, reworded prompts and voices per
-- language, caller announcement (see talkops_core::voicemail_config).
ALTER TABLE tenant_settings
    ADD COLUMN voicemail_config JSONB NOT NULL DEFAULT '{}';
