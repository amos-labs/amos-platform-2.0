CREATE TABLE automations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            VARCHAR(255) NOT NULL,
    description     TEXT,
    enabled         BOOLEAN NOT NULL DEFAULT true,

    -- Trigger
    trigger_type    VARCHAR(50) NOT NULL,  -- record_created, record_updated, record_deleted, schedule, webhook, manual
    trigger_config  JSONB NOT NULL DEFAULT '{}',
    -- For record triggers: {"collection": "orders", "field_conditions": {"status": "paid"}}
    -- For schedule: {"cron": "0 9 * * MON"}
    -- For webhook: {"path": "my-hook"} → receives at /api/v1/hooks/{path}

    -- Condition (optional simple field match against trigger data)
    condition       JSONB,

    -- Action
    action_type     VARCHAR(50) NOT NULL,  -- create_record, update_record, send_notification, call_webhook, run_agent_task
    action_config   JSONB NOT NULL DEFAULT '{}',
    -- create_record: {"collection": "audit_log", "data_template": {"event": "{{trigger.event}}", ...}}
    -- call_webhook: {"url": "https://...", "method": "POST", "headers": {...}}
    -- run_agent_task: {"title": "...", "description": "..."}
    -- send_notification: {"channel": "canvas", "message": "..."}

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_automations_trigger ON automations (trigger_type, enabled);

CREATE TABLE automation_runs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    automation_id   UUID NOT NULL REFERENCES automations(id) ON DELETE CASCADE,
    trigger_data    JSONB NOT NULL DEFAULT '{}',
    status          VARCHAR(20) NOT NULL DEFAULT 'success',  -- success, error, skipped
    result          JSONB,
    error           TEXT,
    duration_ms     INTEGER,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_automation_runs_automation ON automation_runs (automation_id, created_at DESC);
