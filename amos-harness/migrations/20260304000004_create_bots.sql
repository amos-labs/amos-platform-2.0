-- OpenClaw bot management
CREATE TABLE bots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'stopped', -- starting, running, stopped, error

    -- Configuration
    system_prompt TEXT,
    model_id VARCHAR(255) NOT NULL DEFAULT 'us.anthropic.claude-3-5-haiku-20241022-v1:0',
    skills JSONB DEFAULT '[]', -- List of skill identifiers

    -- Channel configurations
    channels JSONB DEFAULT '[]', -- [{type, credentials, enabled, config}]

    -- Runtime
    openclaw_instance_url VARCHAR(500),
    container_id VARCHAR(255),
    last_heartbeat_at TIMESTAMPTZ,
    error_message TEXT,

    -- Stats
    total_messages_processed BIGINT NOT NULL DEFAULT 0,
    total_conversations BIGINT NOT NULL DEFAULT 0,

    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bots_status ON bots(status);
CREATE INDEX idx_bots_name ON bots(name);
