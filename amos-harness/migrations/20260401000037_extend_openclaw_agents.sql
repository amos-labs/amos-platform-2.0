-- Extend openclaw_agents for autoresearch: multi-provider, always-on, cost tiers

ALTER TABLE openclaw_agents
    ADD COLUMN IF NOT EXISTS provider_type VARCHAR(50) NOT NULL DEFAULT 'anthropic',
    ADD COLUMN IF NOT EXISTS api_base VARCHAR(500),
    ADD COLUMN IF NOT EXISTS api_key_credential_id UUID REFERENCES credential_vault(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS max_concurrent_tasks INTEGER NOT NULL DEFAULT 2,
    ADD COLUMN IF NOT EXISTS always_on BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS cost_tier VARCHAR(50) NOT NULL DEFAULT 'standard',
    ADD COLUMN IF NOT EXISTS last_heartbeat_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS task_specializations JSONB NOT NULL DEFAULT '[]',
    ADD COLUMN IF NOT EXISTS experiment_cooldown_until TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_openclaw_agents_provider ON openclaw_agents(provider_type);
CREATE INDEX IF NOT EXISTS idx_openclaw_agents_always_on ON openclaw_agents(always_on) WHERE always_on = true;
CREATE INDEX IF NOT EXISTS idx_openclaw_agents_cost_tier ON openclaw_agents(cost_tier);
