-- Phase 7: single sign-on (OIDC) and directory logins (LDAP/AD).

CREATE TABLE identity_settings (
    tenant_id               UUID PRIMARY KEY REFERENCES tenants (id) ON DELETE CASCADE,
    -- Base URL users open TalkOps with (OIDC redirect); empty = from the request.
    public_url              TEXT NOT NULL DEFAULT '' CHECK (public_url = '' OR public_url ~ '^https?://'),
    oidc_enabled            BOOLEAN NOT NULL DEFAULT false,
    oidc_issuer             TEXT NOT NULL DEFAULT '',
    oidc_client_id          TEXT NOT NULL DEFAULT '',
    oidc_client_secret_enc  TEXT,
    oidc_scopes             TEXT NOT NULL DEFAULT 'openid profile email',
    oidc_username_claim     TEXT NOT NULL DEFAULT 'preferred_username',
    oidc_groups_claim       TEXT NOT NULL DEFAULT 'groups',
    oidc_button_label       TEXT NOT NULL DEFAULT '',
    ldap_enabled            BOOLEAN NOT NULL DEFAULT false,
    -- ldap://host:389 or ldaps://host:636
    ldap_url                TEXT NOT NULL DEFAULT '',
    ldap_starttls           BOOLEAN NOT NULL DEFAULT false,
    ldap_bind_dn            TEXT NOT NULL DEFAULT '',
    ldap_bind_password_enc  TEXT,
    ldap_base_dn            TEXT NOT NULL DEFAULT '',
    -- {username} is replaced by the escaped login name.
    ldap_user_filter        TEXT NOT NULL DEFAULT '(&(objectClass=person)(uid={username}))',
    ldap_username_attr      TEXT NOT NULL DEFAULT 'uid',
    ldap_display_attr       TEXT NOT NULL DEFAULT 'cn',
    ldap_email_attr         TEXT NOT NULL DEFAULT 'mail',
    -- Group membership: attribute on the user entry (AD/OpenLDAP memberOf).
    ldap_group_attr         TEXT NOT NULL DEFAULT 'memberOf',
    -- Group names (OIDC claim values) or DNs (LDAP) mapped to roles.
    admin_group             TEXT NOT NULL DEFAULT '',
    operator_group          TEXT NOT NULL DEFAULT '',
    -- Empty: everyone in the directory may log in as user.
    user_group              TEXT NOT NULL DEFAULT '',
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO identity_settings (tenant_id) SELECT id FROM tenants;

-- Directory accounts are matched by a stable id (OIDC `sub`, LDAP DN/GUID).
ALTER TABLE users ADD COLUMN external_id TEXT;
CREATE UNIQUE INDEX users_external_idx ON users (tenant_id, auth_source, external_id)
    WHERE external_id IS NOT NULL;

-- Pending OIDC logins (state → PKCE verifier and nonce).
CREATE TABLE oidc_logins (
    state_digest TEXT PRIMARY KEY,
    tenant_id    UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    verifier     TEXT NOT NULL,
    nonce        TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    expires_at   TIMESTAMPTZ NOT NULL
);
