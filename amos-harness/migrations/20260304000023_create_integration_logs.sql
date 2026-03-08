-- API call audit trail for all integration operations

CREATE TABLE integration_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    connection_id UUID REFERENCES integration_connections(id) ON DELETE SET NULL,
    integration_id UUID NOT NULL REFERENCES integrations(id) ON DELETE CASCADE,
    operation_id VARCHAR(255),

    -- Request details
    http_method VARCHAR(10),
    request_url TEXT,
    request_headers JSONB DEFAULT '{}',   -- sensitive values masked
    request_body JSONB,

    -- Response details
    http_status INTEGER,
    response_headers JSONB DEFAULT '{}',
    response_body JSONB,

    -- Performance
    duration_ms INTEGER,

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    -- pending, success, failed, rate_limited, timeout
    error_message TEXT,

    -- Rate limit tracking (from response headers)
    rate_limit_remaining INTEGER,
    rate_limit_reset_at TIMESTAMPTZ,

    -- Correlation
    correlation_id VARCHAR(255),  -- for tracing across systems

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- No updated_at since logs are append-only
CREATE INDEX idx_logs_connection ON integration_logs(connection_id);
CREATE INDEX idx_logs_integration ON integration_logs(integration_id);
CREATE INDEX idx_logs_created ON integration_logs(created_at DESC);
CREATE INDEX idx_logs_status ON integration_logs(status);
CREATE INDEX idx_logs_operation ON integration_logs(integration_id, operation_id);
