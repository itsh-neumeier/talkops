-- Phase 4: call queues (mod_callcenter, configured by TalkOps).

CREATE TABLE queues (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id          UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    number             TEXT CHECK (number ~ '^[1-9][0-9]{1,7}$'),
    name               TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    -- mod_callcenter strategies offered in the UI.
    strategy           TEXT NOT NULL DEFAULT 'longest-idle-agent'
                       CHECK (strategy IN ('ring-all', 'longest-idle-agent', 'round-robin',
                                           'top-down', 'agent-with-fewest-calls', 'random')),
    -- Longest wait before the caller goes to the timeout destination (0 = no limit).
    max_wait_secs      INTEGER NOT NULL DEFAULT 300 CHECK (max_wait_secs BETWEEN 0 AND 7200),
    -- How long an agent's phones ring per attempt.
    agent_timeout_secs INTEGER NOT NULL DEFAULT 20 CHECK (agent_timeout_secs BETWEEN 5 AND 120),
    -- Pause after each call before the agent gets the next one.
    wrap_up_secs       INTEGER NOT NULL DEFAULT 5 CHECK (wrap_up_secs BETWEEN 0 AND 600),
    timeout_type       number_destination NOT NULL DEFAULT 'none',
    timeout_id         UUID,
    enabled            BOOLEAN NOT NULL DEFAULT true,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX queues_number_idx ON queues (tenant_id, number);

CREATE TABLE queue_members (
    queue_id     UUID NOT NULL REFERENCES queues (id) ON DELETE CASCADE,
    extension_id UUID NOT NULL REFERENCES extensions (id) ON DELETE CASCADE,
    position     INTEGER NOT NULL,
    PRIMARY KEY (queue_id, extension_id)
);
