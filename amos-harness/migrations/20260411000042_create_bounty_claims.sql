-- Tracks agent bounty claim attempts and lifecycle through the relay
CREATE TABLE IF NOT EXISTS bounty_claims (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id    INTEGER NOT NULL REFERENCES openclaw_agents(id) ON DELETE CASCADE,
    bounty_id   TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'claimed'
                    CHECK (status IN ('claimed', 'executing', 'submitted', 'approved', 'rejected', 'expired')),
    fit_score   DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    estimated_completion INTERVAL,
    reward_tokens BIGINT NOT NULL DEFAULT 0,
    verification_feedback JSONB,
    claimed_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    submitted_at TIMESTAMPTZ,
    verified_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_bounty_claims_agent_id ON bounty_claims(agent_id);
CREATE INDEX idx_bounty_claims_status ON bounty_claims(status);
CREATE INDEX idx_bounty_claims_bounty_id ON bounty_claims(bounty_id);
