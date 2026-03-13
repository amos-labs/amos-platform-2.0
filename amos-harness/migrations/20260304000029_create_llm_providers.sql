-- LLM Provider configurations for BYOK (Bring Your Own Key) support.
-- Users configure their preferred LLM provider and supply their own API key
-- (stored encrypted in the credential_vault table).

CREATE TABLE IF NOT EXISTS llm_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Provider identity
    name TEXT NOT NULL,                    -- "anthropic", "openai", "ollama", "custom"
    display_name TEXT NOT NULL,            -- "Claude (Anthropic)", "GPT (OpenAI)"

    -- Connection details
    api_base TEXT NOT NULL,                -- "https://api.anthropic.com/v1", "https://api.openai.com/v1"
    credential_id UUID REFERENCES credential_vault(id) ON DELETE SET NULL,

    -- Model configuration
    default_model TEXT NOT NULL,           -- "claude-sonnet-4-20250514", "gpt-4o"
    available_models JSONB DEFAULT '[]',   -- ["claude-sonnet-4-20250514", "claude-opus-4-20250514", ...]

    -- State
    is_active BOOLEAN NOT NULL DEFAULT false,  -- only one active at a time
    is_verified BOOLEAN NOT NULL DEFAULT false, -- true after successful test call
    last_error TEXT,                        -- last connection test error

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Only one active provider at a time
CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_providers_active
    ON llm_providers (is_active) WHERE is_active = true;

-- Seed with well-known providers (no API key yet - user must configure)
INSERT INTO llm_providers (name, display_name, api_base, default_model, available_models, is_active)
VALUES
    ('anthropic', 'Claude (Anthropic)', 'https://api.anthropic.com/v1',
     'claude-sonnet-4-20250514',
     '["claude-sonnet-4-20250514", "claude-opus-4-20250514", "claude-haiku-4-20250514"]'::jsonb,
     false),
    ('openai', 'GPT (OpenAI)', 'https://api.openai.com/v1',
     'gpt-4o',
     '["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "o1-preview"]'::jsonb,
     false)
ON CONFLICT DO NOTHING;
