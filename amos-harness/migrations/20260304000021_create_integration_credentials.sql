-- Encrypted credential storage for integration connections

CREATE TABLE integration_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    integration_id UUID NOT NULL REFERENCES integrations(id) ON DELETE CASCADE,

    -- Auth type
    auth_type VARCHAR(50) NOT NULL DEFAULT 'api_key',
    -- api_key, bearer_token, basic_auth, oauth2, sso_key, no_auth, custom

    -- Encrypted credential data (JSON with sensitive fields)
    -- In production, application-level encryption via AES-256-GCM before storage
    credentials_data JSONB NOT NULL DEFAULT '{}',
    -- e.g. {"api_key": "sk_live_...", "api_secret": "..."}
    -- or {"access_token": "...", "refresh_token": "...", "token_type": "Bearer"}

    -- OAuth2 specific fields
    access_token TEXT,
    refresh_token TEXT,
    token_expires_at TIMESTAMPTZ,
    oauth_scopes TEXT,                  -- comma-separated scopes

    -- OAuth2 provider configuration
    oauth_auth_url VARCHAR(500),
    oauth_token_url VARCHAR(500),
    oauth_client_id VARCHAR(255),
    oauth_client_secret TEXT,           -- encrypted

    -- Auth placement
    auth_placement VARCHAR(50) DEFAULT 'header',  -- header, query, body
    auth_key VARCHAR(255),                         -- e.g. "Authorization", "X-API-Key"
    auth_value_template VARCHAR(500),              -- e.g. "Bearer {access_token}", "sso-key {api_key}:{api_secret}"

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'active',  -- active, expired, revoked, rotating
    last_used_at TIMESTAMPTZ,
    last_rotated_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,

    -- Metadata
    label VARCHAR(255),  -- human-friendly name like "Production API Key"
    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_credentials_integration ON integration_credentials(integration_id);
CREATE INDEX idx_credentials_status ON integration_credentials(status);
CREATE INDEX idx_credentials_expires ON integration_credentials(token_expires_at) WHERE token_expires_at IS NOT NULL;
