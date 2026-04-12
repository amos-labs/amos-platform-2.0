-- Rolling performance metrics per agent per period
CREATE TABLE IF NOT EXISTS agent_metrics (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id        INTEGER NOT NULL REFERENCES openclaw_agents(id) ON DELETE CASCADE,
    period_start    TIMESTAMPTZ NOT NULL,
    period_end      TIMESTAMPTZ NOT NULL,
    bounties_discovered INTEGER NOT NULL DEFAULT 0,
    bounties_claimed    INTEGER NOT NULL DEFAULT 0,
    bounties_completed  INTEGER NOT NULL DEFAULT 0,
    bounties_failed     INTEGER NOT NULL DEFAULT 0,
    tokens_earned       BIGINT NOT NULL DEFAULT 0,
    average_quality_score DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    completion_rate     DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_agent_metrics_agent_id ON agent_metrics(agent_id);
CREATE INDEX idx_agent_metrics_period ON agent_metrics(period_start, period_end);
CREATE TABLE IF NOT EXISTS fleet_events (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type  TEXT NOT NULL CHECK (event_type IN ('deployed', 'stopped', 'rebalanced', 'promoted', 'demoted')),
    agent_id    INTEGER REFERENCES openclaw_agents(id) ON DELETE SET NULL,
    metadata    JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_fleet_events_agent_id ON fleet_events(agent_id);
CREATE INDEX idx_fleet_events_type ON fleet_events(event_type);
CREATE INDEX idx_fleet_events_created ON fleet_events(created_at DESC);
