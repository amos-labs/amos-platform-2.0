-- Adds the structured fields agents need to plan and verify bounty work
-- without parsing the freeform `description` blob.
--
-- Source: a real agent in production diagnosed that discover_bounties
-- returned only titles + rewards + descriptions. Acceptance criteria,
-- repo links, test commands, and trust requirements were all buried in
-- prose. This migration models them as first-class columns so the
-- harness, the verifier, and the agent share a single contract for
-- "what does done look like."

ALTER TABLE relay_bounties
    ADD COLUMN IF NOT EXISTS min_trust_level SMALLINT,
    ADD COLUMN IF NOT EXISTS tier SMALLINT,
    ADD COLUMN IF NOT EXISTS acceptance_criteria JSONB,
    ADD COLUMN IF NOT EXISTS repo_url VARCHAR(500),
    ADD COLUMN IF NOT EXISTS test_command TEXT;

CREATE INDEX IF NOT EXISTS idx_relay_bounties_min_trust ON relay_bounties(min_trust_level)
    WHERE min_trust_level IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_relay_bounties_tier ON relay_bounties(tier)
    WHERE tier IS NOT NULL;

COMMENT ON COLUMN relay_bounties.min_trust_level IS
    'Minimum trust level required to claim (1-5). NULL = open to all.';
COMMENT ON COLUMN relay_bounties.tier IS
    'Verification mode tier: 1=scripted, 2=spec+review (default), 3=creative.';
COMMENT ON COLUMN relay_bounties.acceptance_criteria IS
    'Structured contract for what "done" means. JSONB. Replaces parsing description prose.';
COMMENT ON COLUMN relay_bounties.repo_url IS
    'Where the work lives — typically GitHub repo URL.';
COMMENT ON COLUMN relay_bounties.test_command IS
    'Exact command the verifier runs. Agent should self-check with this before submitting.';
