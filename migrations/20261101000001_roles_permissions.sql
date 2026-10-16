-- Roles with fine-grained permissions (ADR 0029).
--
-- Every tenant has three built-in roles: `admin` (all permissions, cannot be
-- changed), `operator` and `user`. Their default permissions reproduce the
-- access rules of the fixed roles before this migration. Admins can change
-- the permissions of `operator` and `user` and add own roles.

CREATE TABLE roles (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    -- Built-in role (`admin`, `operator`, `user`); NULL for own roles.
    builtin         user_role,
    name            TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 64),
    description     TEXT NOT NULL DEFAULT '',
    -- Permission keys, e.g. `calls.view_all` (see talkops_core::roles).
    permissions     TEXT[] NOT NULL DEFAULT '{}',
    -- LDAP/OIDC group that selects this role for directory users.
    directory_group TEXT NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX roles_builtin_idx ON roles (tenant_id, builtin) WHERE builtin IS NOT NULL;
CREATE UNIQUE INDEX roles_name_idx ON roles (tenant_id, lower(name));

-- An own role of the user. NULL: the built-in role in `users.role`.
-- A user with an own role has `users.role = 'user'`.
ALTER TABLE users ADD COLUMN role_id UUID REFERENCES roles (id) ON DELETE SET NULL;
CREATE INDEX users_role_id_idx ON users (role_id);

CREATE FUNCTION create_builtin_roles(tenant UUID) RETURNS void
LANGUAGE sql AS $$
    INSERT INTO roles (tenant_id, builtin, name, description, permissions) VALUES
        (tenant, 'admin', 'Administrator', 'All permissions.', '{}'),
        (tenant, 'operator', 'Operator', 'Daily operation: active calls, all calls, phone book, call blocking and read access to the setup.',
         ARRAY['calls.active', 'calls.view_all', 'calls.live_transcript', 'phonebook.edit',
               'blocking.manage', 'routing.view', 'routing.override', 'extensions.view',
               'users.view', 'trunks.view', 'phones.view']),
        (tenant, 'user', 'Default user', 'Own extensions, calls, recordings and voicemail.', '{}')
    ON CONFLICT DO NOTHING;
$$;

SELECT create_builtin_roles(id) FROM tenants;

CREATE FUNCTION tenants_builtin_roles() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM create_builtin_roles(NEW.id);
    RETURN NEW;
END;
$$;

CREATE TRIGGER tenants_builtin_roles AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION tenants_builtin_roles();
