-- Add embedding support to memory_entries for RAG/semantic search.
-- Uses pgvector with 1536 dimensions (OpenAI text-embedding-3-small).

ALTER TABLE memory_entries ADD COLUMN IF NOT EXISTS embedding vector(1536);
ALTER TABLE memory_entries ADD COLUMN IF NOT EXISTS source VARCHAR(50) DEFAULT 'agent';
ALTER TABLE memory_entries ADD COLUMN IF NOT EXISTS chunk_index INTEGER DEFAULT 0;
ALTER TABLE memory_entries ADD COLUMN IF NOT EXISTS parent_id UUID REFERENCES memory_entries(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_memory_entries_embedding
    ON memory_entries USING hnsw (embedding vector_cosine_ops);
CREATE INDEX IF NOT EXISTS idx_memory_entries_source ON memory_entries(source);
CREATE INDEX IF NOT EXISTS idx_memory_entries_parent ON memory_entries(parent_id) WHERE parent_id IS NOT NULL;
