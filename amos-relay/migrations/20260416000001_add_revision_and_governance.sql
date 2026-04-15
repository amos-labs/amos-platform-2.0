-- Add revision tracking for iterative QA feedback loop.
-- Supports: submitted -> request_revision -> claimed (rework) -> resubmitted.

ALTER TABLE relay_bounties
    ADD COLUMN IF NOT EXISTS revision_count SMALLINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS revision_feedback TEXT,
    ADD COLUMN IF NOT EXISTS pr_url VARCHAR(500),
    ADD COLUMN IF NOT EXISTS category VARCHAR(50) NOT NULL DEFAULT 'infrastructure';

-- Council governance: trust-5 council-appointed agents can verify/approve/reject.
ALTER TABLE relay_agents
    ADD COLUMN IF NOT EXISTS council_member BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS council_appointed_at TIMESTAMPTZ;

-- Indexes for QA bot queries
CREATE INDEX IF NOT EXISTS idx_relay_bounties_revision
    ON relay_bounties(revision_count) WHERE status = 'submitted';
CREATE INDEX IF NOT EXISTS idx_relay_bounties_category
    ON relay_bounties(category);
