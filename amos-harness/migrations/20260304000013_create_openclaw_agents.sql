-- OpenClaw autonomous agents (managed by AgentManager)
CREATE TABLE IF NOT EXISTS openclaw_agents (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    role TEXT NOT NULL,
    capabilities JSONB NOT NULL DEFAULT '[]',
    system_prompt TEXT,
    model VARCHAR(255) NOT NULL DEFAULT 'claude-3-5-sonnet',
    status VARCHAR(50) NOT NULL DEFAULT 'registered',
    trust_level INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_openclaw_agents_status ON openclaw_agents(status);
CREATE INDEX IF NOT EXISTS idx_openclaw_agents_name ON openclaw_agents(name);
