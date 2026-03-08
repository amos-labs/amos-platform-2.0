-- Normalize integration operations into a proper relational table
-- (previously stored as JSONB in integrations.available_actions)

CREATE TABLE integration_operations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    integration_id UUID NOT NULL REFERENCES integrations(id) ON DELETE CASCADE,

    -- Operation identity
    operation_id VARCHAR(255) NOT NULL,       -- e.g. "stripe.list_customers"
    name VARCHAR(255) NOT NULL,               -- e.g. "List Customers"
    description TEXT,

    -- HTTP definition
    http_method VARCHAR(10) NOT NULL DEFAULT 'GET',  -- GET, POST, PUT, PATCH, DELETE
    path_template VARCHAR(500) NOT NULL,              -- e.g. "/v1/customers/{customer_id}"

    -- Schemas (JSON Schema format)
    request_schema JSONB DEFAULT '{}',
    response_schema JSONB DEFAULT '{}',

    -- Pagination
    pagination_strategy VARCHAR(50),  -- cursor, page, offset, token, link_header, none

    -- Behavior
    requires_confirmation BOOLEAN NOT NULL DEFAULT false,
    is_destructive BOOLEAN NOT NULL DEFAULT false,

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'active',  -- active, deprecated, testing

    -- Metadata
    examples JSONB DEFAULT '[]',
    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Unique: one operation_id per integration
CREATE UNIQUE INDEX idx_integration_ops_unique ON integration_operations(integration_id, operation_id);
CREATE INDEX idx_integration_ops_integration ON integration_operations(integration_id);
CREATE INDEX idx_integration_ops_status ON integration_operations(status);

-- Migrate existing available_actions JSONB data into the new table
INSERT INTO integration_operations (integration_id, operation_id, name, description, http_method, path_template, request_schema, pagination_strategy, requires_confirmation)
SELECT
    i.id,
    op->>'operation_id',
    op->>'name',
    op->>'description',
    COALESCE(op->>'http_method', 'GET'),
    COALESCE(op->>'path_template', '/'),
    COALESCE((op->'request_schema')::jsonb, '{}'::jsonb),
    op->>'pagination_strategy',
    COALESCE((op->>'requires_confirmation')::boolean, false)
FROM integrations i,
     jsonb_array_elements(i.available_actions) AS op
WHERE jsonb_typeof(i.available_actions) = 'array'
  AND jsonb_array_length(i.available_actions) > 0;
