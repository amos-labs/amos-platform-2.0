-- Integration connectors (CRM, Email, Payment, etc.)
CREATE TABLE integrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    connector_type VARCHAR(100) NOT NULL, -- crm, email, payment, calendar, storage, custom

    -- Connection
    endpoint_url VARCHAR(500),
    credentials JSONB DEFAULT '{}', -- Encrypted at rest

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'disconnected', -- connected, disconnected, error
    last_sync_at TIMESTAMPTZ,
    error_message TEXT,

    -- Configuration
    sync_config JSONB DEFAULT '{}', -- {interval, direction, mappings}
    available_actions JSONB DEFAULT '[]', -- [{name, description, parameters}]

    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_integrations_type ON integrations(connector_type);
CREATE INDEX idx_integrations_status ON integrations(status);
