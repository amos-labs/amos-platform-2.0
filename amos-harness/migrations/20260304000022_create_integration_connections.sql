-- User/entity connections to integrations
-- A connection binds a specific credential to an integration for a given context

CREATE TABLE integration_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    integration_id UUID NOT NULL REFERENCES integrations(id) ON DELETE CASCADE,
    credential_id UUID REFERENCES integration_credentials(id) ON DELETE SET NULL,

    -- Connection identity
    name VARCHAR(255),  -- e.g. "My Stripe Account", "Team Gmail"

    -- Status tracking
    status VARCHAR(50) NOT NULL DEFAULT 'disconnected',
    -- disconnected, connected, error, rate_limited, suspended
    health VARCHAR(50) NOT NULL DEFAULT 'unknown',
    -- unknown, healthy, degraded, failing

    -- Usage tracking
    last_used_at TIMESTAMPTZ,
    last_sync_at TIMESTAMPTZ,
    error_message TEXT,
    consecutive_errors INTEGER NOT NULL DEFAULT 0,

    -- Rate limiting
    rate_limit_tier VARCHAR(50) DEFAULT 'standard',  -- basic, standard, premium
    daily_write_budget INTEGER DEFAULT 1000,
    daily_writes_used INTEGER DEFAULT 0,
    budget_reset_at TIMESTAMPTZ,

    -- Configuration overrides (per-connection settings)
    config JSONB DEFAULT '{}',
    -- e.g. {"shop_domain": "mystore.myshopify.com", "realm_id": "123456"}

    -- Metadata
    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_connections_integration ON integration_connections(integration_id);
CREATE INDEX idx_connections_credential ON integration_connections(credential_id);
CREATE INDEX idx_connections_status ON integration_connections(status);
CREATE UNIQUE INDEX idx_connections_unique ON integration_connections(integration_id, name);
