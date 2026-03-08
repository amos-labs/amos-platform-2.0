-- Conversation messages (full history per session)
CREATE TABLE messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role VARCHAR(50) NOT NULL, -- 'user', 'assistant', 'system', 'tool_result'
    content JSONB NOT NULL, -- Array of content blocks
    model_id VARCHAR(255),
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    tool_use_id VARCHAR(255), -- If this is a tool result, which tool call
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sequence_number INTEGER NOT NULL -- Ordering within session
);

CREATE INDEX idx_messages_session ON messages(session_id, sequence_number);
CREATE INDEX idx_messages_role ON messages(role);
CREATE INDEX idx_messages_tool_use ON messages(tool_use_id) WHERE tool_use_id IS NOT NULL;
