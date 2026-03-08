-- Dynamic Schema System
-- Collections define structure (like tables), records store data (like rows).
-- The AI agent creates and configures these at runtime — no migrations needed per customer request.

CREATE TABLE collections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,        -- slug: "contacts", "deals", "support_tickets"
    display_name VARCHAR(500) NOT NULL,        -- human: "Contacts", "Deals", "Support Tickets"
    description TEXT,
    fields JSONB NOT NULL DEFAULT '[]',        -- array of field definitions
    settings JSONB NOT NULL DEFAULT '{}',      -- collection-level settings (e.g. default_sort, icon)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_collections_name ON collections(name);

CREATE TABLE records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collection_id UUID NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    data JSONB NOT NULL DEFAULT '{}',          -- the actual record data, keyed by field names
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_records_collection ON records(collection_id);
CREATE INDEX idx_records_data ON records USING GIN(data);
CREATE INDEX idx_records_created ON records(collection_id, created_at DESC);
