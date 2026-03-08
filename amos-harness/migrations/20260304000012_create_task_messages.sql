-- Message bus between tasks (sub-agents / external agents) and the main AMOS conversation.
-- This is the buffering layer: agents write updates here, AMOS reads them and relays to the user.

CREATE TABLE IF NOT EXISTS task_messages (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id         UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,

    -- Direction of the message
    direction       VARCHAR(50) NOT NULL
                    CHECK (direction IN ('agent_to_amos', 'amos_to_agent', 'amos_to_user')),

    -- Message classification
    message_type    VARCHAR(50) NOT NULL
                    CHECK (message_type IN ('status_update', 'question', 'result', 'error', 'progress', 'approval_request')),

    -- Payload
    content         JSONB NOT NULL DEFAULT '{}',   -- { "text": "...", "data": {...}, "options": [...] }

    -- Acknowledgment tracking
    acknowledged    BOOLEAN NOT NULL DEFAULT false,
    acknowledged_at TIMESTAMPTZ,

    -- Timestamps
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Efficient polling: "give me unacknowledged messages for active tasks in my session"
CREATE INDEX idx_task_messages_unacked ON task_messages (task_id, created_at)
    WHERE acknowledged = false;

CREATE INDEX idx_task_messages_task    ON task_messages (task_id, created_at);
CREATE INDEX idx_task_messages_type    ON task_messages (message_type)
    WHERE acknowledged = false;
