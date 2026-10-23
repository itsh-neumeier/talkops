-- Outbound dialing permissions (ADR 0045): tenant-wide dialing profiles
-- with rules, an optional profile per extension and the outbound reason in
-- the call record.

CREATE TABLE dialing_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 100),
    description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 500),
    -- What happens to a number that no rule matches.
    base_mode TEXT NOT NULL CHECK (base_mode IN ('allow', 'deny')),
    -- SIP address dialing (user@host over the Internet).
    sip_addresses BOOLEAN NOT NULL DEFAULT TRUE,
    -- The profile of extensions without their own profile.
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id)
);
CREATE UNIQUE INDEX dialing_profiles_name_idx ON dialing_profiles (tenant_id, lower(name));
CREATE UNIQUE INDEX dialing_profiles_default_idx ON dialing_profiles (tenant_id) WHERE is_default;

CREATE TABLE dialing_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    profile_id UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('country', 'prefix', 'number')),
    value TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('allow', 'deny')),
    label TEXT NOT NULL DEFAULT '' CHECK (length(label) <= 200),
    FOREIGN KEY (tenant_id, profile_id) REFERENCES dialing_profiles (tenant_id, id) ON DELETE CASCADE,
    UNIQUE (profile_id, kind, value),
    CHECK (
        (kind = 'country' AND value ~ '^[A-Z]{2}$')
        OR (kind = 'prefix' AND value ~ '^\+[0-9]{1,15}$')
        OR (kind = 'number' AND value ~ '^\+[1-9][0-9]{2,14}$')
    )
);
CREATE INDEX dialing_rules_profile_idx ON dialing_rules (profile_id);

-- NULL: the default profile of the tenant. A profile in use cannot be deleted.
ALTER TABLE extensions ADD COLUMN dialing_profile_id UUID;
ALTER TABLE extensions ADD CONSTRAINT extensions_dialing_profile_fk
    FOREIGN KEY (tenant_id, dialing_profile_id) REFERENCES dialing_profiles (tenant_id, id);

-- Outbound calls blocked by a dialing profile keep their reason.
ALTER TABLE cdr DROP CONSTRAINT cdr_blocked_reason_valid;
ALTER TABLE cdr ADD CONSTRAINT cdr_blocked_reason_valid CHECK (
    blocked_reason IS NULL OR (
        direction = 'inbound' AND (
            blocked_reason = 'anonymous'
            OR blocked_reason ~ '^list:\+[0-9]{3,16}\*?$'
            OR blocked_reason ~ '^phoneblock:[0-9]{1,10}$'
        )
    ) OR (
        direction = 'outbound' AND (
            blocked_reason IN ('dialing:base', 'dialing:sip')
            OR blocked_reason ~ '^dialing:country:[A-Z]{2}$'
            OR blocked_reason ~ '^dialing:prefix:\+[0-9]{1,15}$'
            OR blocked_reason ~ '^dialing:number:\+[0-9]{3,15}$'
        )
    )
);

-- Five editable profiles for each tenant. Keep the premium-rate rules equal
-- to talkops_core::dial_permissions::PREMIUM_RULES.
CREATE TEMPORARY TABLE dialing_seed_premium (value TEXT, label TEXT) ON COMMIT DROP;
INSERT INTO dialing_seed_premium (value, label) VALUES
    ('+49900', 'DE premium-rate services (0900)'),
    ('+49137', 'DE mass-traffic services (0137)'),
    ('+49118', 'DE directory enquiry services (118xy)'),
    ('+4912', 'DE innovative services (012)'),
    ('+4919', 'DE online services (019)'),
    ('+43900', 'AT value-added services (0900)'),
    ('+43901', 'AT value-added services (0901)'),
    ('+43930', 'AT value-added services (0930)'),
    ('+43931', 'AT value-added services (0931)'),
    ('+43939', 'AT value-added services (0939)'),
    ('+43118', 'AT directory enquiry services (118)'),
    ('+41900', 'CH value-added services (0900)'),
    ('+41901', 'CH value-added services (0901)'),
    ('+41906', 'CH value-added services (0906)'),
    ('+870', 'Inmarsat (+870)'),
    ('+881', 'Satellite networks (+881)'),
    ('+882', 'International networks (+882)'),
    ('+883', 'International networks (+883)');

CREATE TEMPORARY TABLE dialing_seed_europe (iso TEXT) ON COMMIT DROP;
INSERT INTO dialing_seed_europe (iso) VALUES
    ('AT'), ('BE'), ('BG'), ('HR'), ('CY'), ('CZ'), ('DK'), ('EE'), ('FI'),
    ('FR'), ('DE'), ('GR'), ('HU'), ('IE'), ('IT'), ('LV'), ('LT'), ('LU'),
    ('MT'), ('NL'), ('PL'), ('PT'), ('RO'), ('SK'), ('SI'), ('ES'), ('SE'),
    ('IS'), ('LI'), ('NO'), ('CH');

INSERT INTO dialing_profiles (tenant_id, name, description, base_mode, sip_addresses, is_default)
SELECT t.id, p.name, p.description, p.base_mode, p.sip_addresses,
       -- Existing installations keep calling everything; new ones block
       -- premium-rate numbers.
       CASE WHEN EXISTS (SELECT 1 FROM extensions e WHERE e.tenant_id = t.id)
                 OR EXISTS (SELECT 1 FROM trunks tr WHERE tr.tenant_id = t.id)
            THEN p.name = 'Unrestricted'
            ELSE p.name = 'International' END
FROM tenants t
CROSS JOIN (VALUES
    ('Unrestricted', 'All numbers. The behavior before dialing profiles.', 'allow', TRUE),
    ('Internal only', 'Internal calls and emergency calls only.', 'deny', FALSE),
    ('National', 'Numbers of the own country, without premium-rate numbers.', 'deny', TRUE),
    ('National + Europe', 'Own country, EU, EEA and Switzerland, without premium-rate numbers.', 'deny', TRUE),
    ('International', 'All countries, without premium-rate numbers and international networks.', 'allow', TRUE)
) AS p (name, description, base_mode, sip_addresses);

INSERT INTO dialing_rules (tenant_id, profile_id, kind, value, action, label)
SELECT p.tenant_id, p.id, 'prefix', '+' || COALESCE(s.country_code, '49'), 'allow', 'Own country'
FROM dialing_profiles p
LEFT JOIN tenant_settings s ON s.tenant_id = p.tenant_id
WHERE p.name IN ('National', 'National + Europe');

INSERT INTO dialing_rules (tenant_id, profile_id, kind, value, action, label)
SELECT p.tenant_id, p.id, 'country', e.iso, 'allow', ''
FROM dialing_profiles p CROSS JOIN dialing_seed_europe e
WHERE p.name = 'National + Europe';

INSERT INTO dialing_rules (tenant_id, profile_id, kind, value, action, label)
SELECT p.tenant_id, p.id, 'prefix', r.value, 'deny', r.label
FROM dialing_profiles p CROSS JOIN dialing_seed_premium r
WHERE p.name IN ('National', 'National + Europe', 'International')
ON CONFLICT (profile_id, kind, value) DO NOTHING;
