-- Credential Vault: AES-256-GCM encrypted secret storage.
-- Secrets submitted via the Secure Input Canvas bypass the chat and are
-- stored here encrypted. The AI agent receives only opaque credential_ids.

CREATE TABLE credential_vault (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Human-readable label shown in UI (e.g. "Stripe Secret Key")
    label VARCHAR(255) NOT NULL,

    -- Service/integration this credential is for (e.g. "stripe", "github")
    service VARCHAR(255) NOT NULL,

    -- The credential type (e.g. "api_key", "oauth_token", "password")
    credential_type VARCHAR(100) NOT NULL DEFAULT 'api_key',

    -- AES-256-GCM encrypted blob (base64). Contains the actual secret value.
    -- Format: base64(nonce_12bytes || ciphertext || gcm_tag)
    encrypted_value TEXT NOT NULL,

    -- Optional: encrypted JSON with additional fields (e.g. multiple keys)
    -- Same encryption format as encrypted_value.
    encrypted_metadata TEXT,

    -- Status: active, revoked, expired
    status VARCHAR(50) NOT NULL DEFAULT 'active',

    -- Optional link to integration_credentials for auto-wiring
    integration_credential_id UUID REFERENCES integration_credentials(id) ON DELETE SET NULL,

    -- Audit fields
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_credential_vault_service ON credential_vault(service);
CREATE INDEX idx_credential_vault_status ON credential_vault(status);
CREATE INDEX idx_credential_vault_integration ON credential_vault(integration_credential_id)
    WHERE integration_credential_id IS NOT NULL;
