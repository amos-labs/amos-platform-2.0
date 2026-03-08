-- External agent registry (EAP - External Agent Protocol)
CREATE TABLE external_agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    endpoint_url VARCHAR(500) NOT NULL,

    -- Trust & reputation
    trust_level SMALLINT NOT NULL DEFAULT 1, -- 1=Newcomer, 2=Bronze, 3=Silver, 4=Gold, 5=Elite
    total_tasks_completed BIGINT NOT NULL DEFAULT 0,
    total_tasks_failed BIGINT NOT NULL DEFAULT 0,
    completion_rate DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    average_quality_score DOUBLE PRECISION NOT NULL DEFAULT 0.0,

    -- Capabilities
    capabilities JSONB DEFAULT '[]', -- ["code_generation", "data_analysis", etc.]
    max_concurrent_tasks INTEGER NOT NULL DEFAULT 1,

    -- Authentication
    api_key_hash VARCHAR(255),
    wallet_address VARCHAR(64), -- Solana wallet for token rewards

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'active', -- active, suspended, banned
    last_seen_at TIMESTAMPTZ,

    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_external_agents_trust ON external_agents(trust_level);
CREATE INDEX idx_external_agents_status ON external_agents(status);
CREATE INDEX idx_external_agents_wallet ON external_agents(wallet_address) WHERE wallet_address IS NOT NULL;

-- Work items assigned to external agents
CREATE TABLE work_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES external_agents(id),

    -- Task details
    title VARCHAR(500) NOT NULL,
    description TEXT NOT NULL,
    task_type VARCHAR(100) NOT NULL, -- code, analysis, content, review, etc.
    input_data JSONB DEFAULT '{}',

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending, assigned, in_progress, completed, failed, cancelled
    priority INTEGER NOT NULL DEFAULT 5, -- 1=highest, 10=lowest

    -- Results
    output_data JSONB,
    quality_score DOUBLE PRECISION,
    reviewer_notes TEXT,

    -- Rewards
    reward_tokens BIGINT DEFAULT 0,
    reward_claimed BOOLEAN NOT NULL DEFAULT false,

    -- Timing
    assigned_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    deadline_at TIMESTAMPTZ,

    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_work_items_agent ON work_items(agent_id);
CREATE INDEX idx_work_items_status ON work_items(status);
CREATE INDEX idx_work_items_priority ON work_items(priority, created_at);
