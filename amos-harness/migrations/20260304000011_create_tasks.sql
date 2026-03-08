-- Unified task queue for both internal sub-agent work and external bounties.
-- This is harness infrastructure: AMOS dispatches tasks, the harness manages execution.

CREATE TABLE IF NOT EXISTS tasks (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What
    title           VARCHAR(500) NOT NULL,
    description     TEXT,
    context         JSONB DEFAULT '{}',           -- Input data, parameters, references

    -- Classification
    category        VARCHAR(50) NOT NULL           -- 'internal' (sub-agent) or 'external' (bounty)
                    CHECK (category IN ('internal', 'external')),
    task_type       VARCHAR(100),                  -- free-form: research, code, analysis, content, report, etc.
    priority        INTEGER DEFAULT 5              -- 1 = highest, 10 = lowest
                    CHECK (priority BETWEEN 1 AND 10),

    -- Lifecycle
    status          VARCHAR(50) NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending', 'assigned', 'running', 'completed', 'failed', 'cancelled')),

    -- Assignment
    assigned_to     UUID,                          -- external_agents.id for bounties; NULL for internal
    session_id      UUID REFERENCES sessions(id) ON DELETE SET NULL,  -- originating conversation

    -- Hierarchy
    parent_task_id  UUID REFERENCES tasks(id) ON DELETE SET NULL,  -- sub-task support

    -- Results
    result          JSONB,                         -- output data on completion
    error_message   TEXT,                          -- failure reason

    -- Bounty (external tasks only)
    reward_tokens   BIGINT DEFAULT 0,
    reward_claimed  BOOLEAN DEFAULT false,
    deadline_at     TIMESTAMPTZ,

    -- Timestamps
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    assigned_at     TIMESTAMPTZ,
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ
);

-- Fast lookups
CREATE INDEX idx_tasks_status          ON tasks (status);
CREATE INDEX idx_tasks_category        ON tasks (category);
CREATE INDEX idx_tasks_priority        ON tasks (priority, created_at);
CREATE INDEX idx_tasks_session         ON tasks (session_id) WHERE session_id IS NOT NULL;
CREATE INDEX idx_tasks_assigned        ON tasks (assigned_to) WHERE assigned_to IS NOT NULL;
CREATE INDEX idx_tasks_parent          ON tasks (parent_task_id) WHERE parent_task_id IS NOT NULL;

-- Bounty browsing: external agents look for available work
CREATE INDEX idx_tasks_available_bounties ON tasks (priority, created_at)
    WHERE category = 'external' AND status = 'pending';
