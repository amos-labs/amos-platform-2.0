-- Add missing columns to relay_bounties for full EAP bounty lifecycle support.
-- The original schema used minimal columns; the API handlers need these additional fields.

-- Split claimed_by into agent + harness tracking
ALTER TABLE relay_bounties ADD COLUMN IF NOT EXISTS claimed_by_agent_id UUID REFERENCES relay_agents(id);
ALTER TABLE relay_bounties ADD COLUMN IF NOT EXISTS claimed_by_harness_id VARCHAR(255);

-- Submission details
ALTER TABLE relay_bounties ADD COLUMN IF NOT EXISTS submitted_at TIMESTAMPTZ;
ALTER TABLE relay_bounties ADD COLUMN IF NOT EXISTS quality_evidence JSONB;

-- Approval/rejection details
ALTER TABLE relay_bounties ADD COLUMN IF NOT EXISTS approved_at TIMESTAMPTZ;
ALTER TABLE relay_bounties ADD COLUMN IF NOT EXISTS rejected_at TIMESTAMPTZ;
ALTER TABLE relay_bounties ADD COLUMN IF NOT EXISTS rejection_reason TEXT;

-- Settlement tracking
ALTER TABLE relay_bounties ADD COLUMN IF NOT EXISTS settlement_tx VARCHAR(128);
ALTER TABLE relay_bounties ADD COLUMN IF NOT EXISTS settlement_status VARCHAR(50) NOT NULL DEFAULT 'pending';

-- Use a proper enum type for bounty status
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'bounty_status') THEN
        CREATE TYPE bounty_status AS ENUM (
            'open', 'claimed', 'submitted', 'approved', 'rejected', 'expired', 'cancelled'
        );
    END IF;
END$$;

-- Migrate claimed_by data to new columns (if any rows exist with old column)
UPDATE relay_bounties SET claimed_by_agent_id = claimed_by WHERE claimed_by IS NOT NULL AND claimed_by_agent_id IS NULL;

-- Index for settlement tracking
CREATE INDEX IF NOT EXISTS idx_relay_bounties_settlement ON relay_bounties(settlement_status);
