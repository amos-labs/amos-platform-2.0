-- Working memory for AMOS agent
CREATE TABLE memory_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,

    -- Content
    content TEXT NOT NULL,
    category VARCHAR(100), -- 'fact', 'preference', 'procedure', 'context'
    tags JSONB DEFAULT '[]',

    -- Salience scoring
    salience DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    access_count INTEGER NOT NULL DEFAULT 0,
    last_accessed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Embedding for semantic search (pgvector)
    -- embedding vector(1536), -- Uncomment when pgvector is enabled

    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_memory_entries_session ON memory_entries(session_id) WHERE session_id IS NOT NULL;
CREATE INDEX idx_memory_entries_category ON memory_entries(category);
CREATE INDEX idx_memory_entries_salience ON memory_entries(salience DESC);
-- CREATE INDEX idx_memory_entries_embedding ON memory_entries USING ivfflat (embedding vector_cosine_ops);
