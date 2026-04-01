-- Autoresearch: Darwinian optimization + swarm management
-- 5 new tables for agent coordination, fitness metrics, and self-improvement

-- Agent swarms: groups of agents with hierarchy and routing
CREATE TABLE IF NOT EXISTS agent_swarms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    parent_swarm_id UUID REFERENCES agent_swarms(id) ON DELETE SET NULL,
    layer_order INTEGER NOT NULL DEFAULT 0,
    routing_strategy VARCHAR(50) NOT NULL DEFAULT 'round_robin',
    max_agents INTEGER NOT NULL DEFAULT 10,
    enabled BOOLEAN NOT NULL DEFAULT true,
    domain VARCHAR(100) NOT NULL DEFAULT 'custom',
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_agent_swarms_parent ON agent_swarms(parent_swarm_id);
CREATE INDEX IF NOT EXISTS idx_agent_swarms_enabled ON agent_swarms(enabled);
CREATE INDEX IF NOT EXISTS idx_agent_swarms_domain ON agent_swarms(domain);

-- Agent swarm members: many-to-many agents <-> swarms with Darwinian weights
CREATE TABLE IF NOT EXISTS agent_swarm_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    swarm_id UUID NOT NULL REFERENCES agent_swarms(id) ON DELETE CASCADE,
    agent_id INTEGER NOT NULL REFERENCES openclaw_agents(id) ON DELETE CASCADE,
    weight DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    fitness_score DOUBLE PRECISION,
    role VARCHAR(50) NOT NULL DEFAULT 'worker',
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(swarm_id, agent_id)
);

CREATE INDEX IF NOT EXISTS idx_swarm_members_swarm ON agent_swarm_members(swarm_id);
CREATE INDEX IF NOT EXISTS idx_swarm_members_agent ON agent_swarm_members(agent_id);
CREATE INDEX IF NOT EXISTS idx_swarm_members_fitness ON agent_swarm_members(swarm_id, fitness_score DESC);

-- Fitness functions: configurable metrics per swarm
CREATE TABLE IF NOT EXISTS fitness_functions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    swarm_id UUID NOT NULL REFERENCES agent_swarms(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    metric_source VARCHAR(50) NOT NULL DEFAULT 'internal',
    metric_type VARCHAR(100) NOT NULL DEFAULT 'task_completion_rate',
    metric_query TEXT,
    metric_endpoint TEXT,
    metric_config JSONB NOT NULL DEFAULT '{}',
    window_days INTEGER NOT NULL DEFAULT 60,
    weight DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    last_value DOUBLE PRECISION,
    last_computed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_fitness_functions_swarm ON fitness_functions(swarm_id);

-- Autoresearch experiments: core Darwinian tracking
CREATE TABLE IF NOT EXISTS autoresearch_experiments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    swarm_id UUID NOT NULL REFERENCES agent_swarms(id) ON DELETE CASCADE,
    agent_id INTEGER NOT NULL REFERENCES openclaw_agents(id) ON DELETE CASCADE,
    experiment_type VARCHAR(50) NOT NULL DEFAULT 'prompt_mutation',
    diff JSONB NOT NULL DEFAULT '[]',
    original_prompt TEXT,
    mutated_prompt TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'proposed',
    baseline_fitness DOUBLE PRECISION,
    final_fitness DOUBLE PRECISION,
    fitness_delta DOUBLE PRECISION,
    evaluation_days INTEGER NOT NULL DEFAULT 5,
    cooldown_days INTEGER NOT NULL DEFAULT 5,
    proposed_by VARCHAR(255),
    proposal_reasoning TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_experiments_swarm ON autoresearch_experiments(swarm_id);
CREATE INDEX IF NOT EXISTS idx_experiments_agent ON autoresearch_experiments(agent_id);
CREATE INDEX IF NOT EXISTS idx_experiments_status ON autoresearch_experiments(status);

-- Agent scorecards: rolling performance snapshots
CREATE TABLE IF NOT EXISTS agent_scorecards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id INTEGER NOT NULL REFERENCES openclaw_agents(id) ON DELETE CASCADE,
    swarm_id UUID NOT NULL REFERENCES agent_swarms(id) ON DELETE CASCADE,
    fitness_score DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    tasks_completed INTEGER NOT NULL DEFAULT 0,
    tasks_failed INTEGER NOT NULL DEFAULT 0,
    avg_task_duration_ms BIGINT,
    total_tokens_used BIGINT NOT NULL DEFAULT 0,
    total_cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    metric_scores JSONB NOT NULL DEFAULT '{}',
    window_start TIMESTAMPTZ NOT NULL,
    window_end TIMESTAMPTZ NOT NULL,
    weight_at_snapshot DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_scorecards_agent ON agent_scorecards(agent_id);
CREATE INDEX IF NOT EXISTS idx_scorecards_swarm ON agent_scorecards(swarm_id);
CREATE INDEX IF NOT EXISTS idx_scorecards_window ON agent_scorecards(window_start, window_end);

-- Agent task attribution: links task outcomes to agents for fitness computation
CREATE TABLE IF NOT EXISTS agent_task_attribution (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id INTEGER NOT NULL REFERENCES openclaw_agents(id) ON DELETE CASCADE,
    task_id UUID NOT NULL,
    swarm_id UUID REFERENCES agent_swarms(id) ON DELETE SET NULL,
    tokens_used BIGINT NOT NULL DEFAULT 0,
    cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    duration_ms BIGINT NOT NULL DEFAULT 0,
    quality_score DOUBLE PRECISION,
    metric_impact JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_attribution_agent ON agent_task_attribution(agent_id);
CREATE INDEX IF NOT EXISTS idx_attribution_task ON agent_task_attribution(task_id);
CREATE INDEX IF NOT EXISTS idx_attribution_swarm ON agent_task_attribution(swarm_id);
CREATE INDEX IF NOT EXISTS idx_attribution_created ON agent_task_attribution(created_at);
