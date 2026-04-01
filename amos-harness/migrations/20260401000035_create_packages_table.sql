-- Package registry: tracks installed packages, their state, and system prompts.
CREATE TABLE IF NOT EXISTS packages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    display_name VARCHAR(255) NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    version VARCHAR(50) NOT NULL DEFAULT '0.0.0',
    enabled BOOLEAN NOT NULL DEFAULT false,
    system_prompt TEXT,
    tool_count INTEGER NOT NULL DEFAULT 0,
    tool_names JSONB NOT NULL DEFAULT '[]'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
