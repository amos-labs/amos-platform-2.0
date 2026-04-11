-- Add unique constraint on external_agents.name so the sidecar agent
-- can upsert on registration (ON CONFLICT (name) DO UPDATE).
CREATE UNIQUE INDEX IF NOT EXISTS idx_external_agents_name_unique
    ON external_agents (name);
