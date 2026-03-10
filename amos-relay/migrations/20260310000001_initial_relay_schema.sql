-- AMOS Network Relay Schema
-- Global bounty marketplace, agent directory, and reputation oracle

-- Connected harness instances
CREATE TABLE IF NOT EXISTS relay_harnesses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    version VARCHAR(50) NOT NULL,
    endpoint_url VARCHAR(500) NOT NULL,
    api_key_hash VARCHAR(128) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    agent_count INTEGER NOT NULL DEFAULT 0,
    active_bounties INTEGER NOT NULL DEFAULT 0,
    healthy BOOLEAN NOT NULL DEFAULT true,
    connected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_heartbeat TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_relay_harnesses_status ON relay_harnesses(status);

-- Global agent directory
CREATE TABLE IF NOT EXISTS relay_agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    display_name VARCHAR(255),
    endpoint_url VARCHAR(500),
    capabilities JSONB NOT NULL DEFAULT '[]',
    description TEXT,
    wallet_address VARCHAR(64),
    harness_id UUID REFERENCES relay_harnesses(id),
    -- Reputation fields
    trust_level SMALLINT NOT NULL DEFAULT 1,
    total_bounties_completed BIGINT NOT NULL DEFAULT 0,
    total_bounties_failed BIGINT NOT NULL DEFAULT 0,
    completion_rate DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    avg_quality_score DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_heartbeat TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_relay_agents_status ON relay_agents(status);
CREATE INDEX idx_relay_agents_trust ON relay_agents(trust_level);
CREATE INDEX idx_relay_agents_harness ON relay_agents(harness_id);
CREATE INDEX idx_relay_agents_wallet ON relay_agents(wallet_address);

-- Bounty marketplace
CREATE TABLE IF NOT EXISTS relay_bounties (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(500) NOT NULL,
    description TEXT,
    -- Economics
    reward_tokens BIGINT NOT NULL DEFAULT 0,
    protocol_fee_tokens BIGINT NOT NULL DEFAULT 0,
    -- Metadata
    required_capabilities JSONB DEFAULT '[]',
    context JSONB DEFAULT '{}',
    -- Poster info
    poster_wallet VARCHAR(64),
    poster_harness_id UUID REFERENCES relay_harnesses(id),
    -- Assignment
    claimed_by UUID REFERENCES relay_agents(id),
    claimed_at TIMESTAMPTZ,
    -- Result
    result JSONB,
    quality_score SMALLINT,
    -- Status workflow: open -> claimed -> submitted -> approved/rejected
    status VARCHAR(50) NOT NULL DEFAULT 'open',
    -- Timestamps
    deadline_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_relay_bounties_status ON relay_bounties(status);
CREATE INDEX idx_relay_bounties_reward ON relay_bounties(reward_tokens);
CREATE INDEX idx_relay_bounties_poster ON relay_bounties(poster_harness_id);
CREATE INDEX idx_relay_bounties_claimed ON relay_bounties(claimed_by);
CREATE INDEX idx_relay_bounties_deadline ON relay_bounties(deadline_at);

-- Reputation reports from harnesses
CREATE TABLE IF NOT EXISTS reputation_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES relay_agents(id),
    harness_id UUID NOT NULL REFERENCES relay_harnesses(id),
    bounty_id UUID REFERENCES relay_bounties(id),
    outcome VARCHAR(50) NOT NULL, -- 'completed', 'failed'
    quality_score SMALLINT NOT NULL DEFAULT 0, -- 0-100
    evidence_hash VARCHAR(128),
    reported_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_reputation_reports_agent ON reputation_reports(agent_id);
CREATE INDEX idx_reputation_reports_harness ON reputation_reports(harness_id);

-- Protocol fee ledger (tracks all fees collected for distribution)
CREATE TABLE IF NOT EXISTS protocol_fee_ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bounty_id UUID NOT NULL REFERENCES relay_bounties(id),
    total_fee BIGINT NOT NULL,
    holder_share BIGINT NOT NULL,
    treasury_share BIGINT NOT NULL,
    ops_burn_share BIGINT NOT NULL,
    -- Settlement
    settled_on_chain BOOLEAN NOT NULL DEFAULT false,
    settlement_tx VARCHAR(128),
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_fee_ledger_bounty ON protocol_fee_ledger(bounty_id);
CREATE INDEX idx_fee_ledger_settled ON protocol_fee_ledger(settled_on_chain);
