-- Cache of relay-side agent UUIDs for openclaw_agents.
--
-- Surfaced by Finding #2 of the harness↔relay integration bug report
-- (bounty ea3466b2): claim_bounty was sending the local serial id as
-- agent_id, which the relay couldn't match against its UUID-keyed
-- relay_agents table → 401. The harness now ensures-registered with the
-- relay on first claim attempt and caches the returned UUID here so
-- subsequent calls go straight through.

ALTER TABLE openclaw_agents
    ADD COLUMN IF NOT EXISTS relay_agent_id UUID;

CREATE UNIQUE INDEX IF NOT EXISTS idx_openclaw_agents_relay_agent_id
    ON openclaw_agents (relay_agent_id)
    WHERE relay_agent_id IS NOT NULL;

COMMENT ON COLUMN openclaw_agents.relay_agent_id IS
    'Cached relay_agents.id (UUID) from POST /api/v1/agents/register. '
    'NULL until first ensure_registered_with_relay() call succeeds.';
