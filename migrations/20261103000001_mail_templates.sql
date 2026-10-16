-- Mail design and templates (ADR 0034): HTML + plain-text mails with branding,
-- instance information and editable texts per mail type and language.

-- Name of this TalkOps installation. Mails use it in the subject prefix
-- "[name]" and in the footer. Empty: no prefix.
ALTER TABLE tenant_settings
    ADD COLUMN instance_name TEXT NOT NULL DEFAULT '' CHECK (length(instance_name) <= 60);

-- Branding of all mails (one row per tenant; no row = defaults).
CREATE TABLE mail_branding (
    tenant_id     UUID PRIMARY KEY REFERENCES tenants (id) ON DELETE CASCADE,
    -- Display name of the sender, e.g. "Office phone system". Empty: SMTP sender as set.
    sender_name   TEXT NOT NULL DEFAULT '' CHECK (length(sender_name) <= 80),
    -- Accent color of the header bar and the links.
    accent_color  TEXT NOT NULL DEFAULT '#0d9488' CHECK (accent_color ~ '^#[0-9a-f]{6}$'),
    -- Header and footer text (Markdown subset with placeholders, see ADR 0034).
    header_text   TEXT NOT NULL DEFAULT '' CHECK (length(header_text) <= 200),
    footer_text   TEXT NOT NULL DEFAULT '' CHECK (length(footer_text) <= 1000),
    -- Logo, embedded in each mail as inline image (Content-ID).
    logo          BYTEA CHECK (octet_length(logo) <= 262144),
    logo_type     TEXT CHECK (logo_type IN ('image/png', 'image/jpeg', 'image/gif')),
    logo_width    INTEGER CHECK (logo_width BETWEEN 1 AND 4096),
    logo_height   INTEGER CHECK (logo_height BETWEEN 1 AND 4096),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((logo IS NULL) = (logo_type IS NULL))
);

-- Own texts per mail type and language. No row: the built-in default text.
CREATE TABLE mail_templates (
    tenant_id   UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    -- Mail type from the registry in talkops-core (mail_render::KINDS).
    kind        TEXT NOT NULL CHECK (kind ~ '^[a-z][a-z0-9_.]{0,63}$'),
    language    TEXT NOT NULL CHECK (language IN ('de', 'en')),
    subject     TEXT NOT NULL CHECK (length(subject) BETWEEN 1 AND 300),
    body        TEXT NOT NULL CHECK (length(body) BETWEEN 1 AND 20000),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, kind, language)
);
