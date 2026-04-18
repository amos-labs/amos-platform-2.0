-- Automation retry queue for failed webhook deliveries.
--
-- When an outbound webhook (ActionType::CallWebhook) fails, the run is logged
-- to automation_runs (existing behavior) AND enqueued here for retry with
-- exponential backoff (30s, 2min, 10min). After 3 attempts, status moves to
-- 'dead_letter' for manual inspection via the failures canvas.

CREATE TABLE automation_retry_queue (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    automation_id   UUID NOT NULL REFERENCES automations(id) ON DELETE CASCADE,
    action_type     VARCHAR(50) NOT NULL,
    trigger_data    JSONB NOT NULL DEFAULT '{}',
    attempt         INTEGER NOT NULL DEFAULT 1,
    max_attempts    INTEGER NOT NULL DEFAULT 3,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_error      TEXT,
    -- status: 'pending' (waiting for next_attempt_at), 'in_progress' (being retried now),
    --         'dead_letter' (permanently failed), 'succeeded' (retry worked)
    status          VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- For the retry worker: find pending items whose backoff has elapsed.
CREATE INDEX idx_retry_queue_due
    ON automation_retry_queue (next_attempt_at)
    WHERE status = 'pending';

-- For the failures dashboard: list dead-letter items per automation.
CREATE INDEX idx_retry_queue_automation
    ON automation_retry_queue (automation_id, status, created_at DESC);
