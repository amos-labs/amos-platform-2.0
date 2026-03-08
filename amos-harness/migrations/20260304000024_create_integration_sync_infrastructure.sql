-- Phase 3: ETL/Sync infrastructure
-- Sync configs, records (dedup tracking), and cursors (pagination state)

-- ═══════════════════════════════════════════════════════════════════════════
-- 1. SYNC CONFIGS - Defines how data flows between external APIs and AMOS
-- ═══════════════════════════════════════════════════════════════════════════
CREATE TABLE integration_sync_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    connection_id UUID NOT NULL REFERENCES integration_connections(id) ON DELETE CASCADE,

    -- What to sync
    resource_type VARCHAR(255) NOT NULL,    -- external resource, e.g. "customers", "invoices"
    target_collection VARCHAR(255) NOT NULL, -- AMOS collection to sync into

    -- Sync behavior
    sync_mode VARCHAR(50) NOT NULL DEFAULT 'incremental',  -- full, incremental
    sync_direction VARCHAR(50) NOT NULL DEFAULT 'inbound',  -- inbound, outbound, bidirectional

    -- Field mappings: how external fields map to AMOS collection fields
    field_mappings JSONB NOT NULL DEFAULT '[]',
    -- e.g. [{"source": "email", "target": "email", "transform": null},
    --       {"source": "name", "target": "full_name", "transform": "titlecase"}]

    -- Conflict resolution
    conflict_resolution VARCHAR(50) DEFAULT 'external_wins',
    -- external_wins, internal_wins, manual, newest

    -- Scheduling
    schedule_type VARCHAR(50) NOT NULL DEFAULT 'manual',  -- manual, scheduled, realtime
    schedule_cron VARCHAR(100),                            -- e.g. "0 */6 * * *" (every 6 hours)

    -- Approval workflow
    requires_approval BOOLEAN NOT NULL DEFAULT false,
    approval_threshold INTEGER DEFAULT 100,  -- stage records for review if batch > N

    -- Fetch configuration
    fetch_operation_id VARCHAR(255) NOT NULL,  -- which operation to call for extraction
    fetch_params JSONB DEFAULT '{}',           -- static params to pass

    -- Status
    enabled BOOLEAN NOT NULL DEFAULT true,
    last_run_at TIMESTAMPTZ,
    last_run_status VARCHAR(50),  -- success, partial, failed
    last_run_stats JSONB DEFAULT '{}',
    -- e.g. {"extracted": 150, "transformed": 148, "loaded": 148, "errors": 2}

    -- Metadata
    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_sync_config_unique ON integration_sync_configs(connection_id, resource_type, target_collection);
CREATE INDEX idx_sync_config_connection ON integration_sync_configs(connection_id);
CREATE INDEX idx_sync_config_enabled ON integration_sync_configs(enabled) WHERE enabled = true;

-- ═══════════════════════════════════════════════════════════════════════════
-- 2. SYNC RECORDS - Deduplication and change tracking
-- ═══════════════════════════════════════════════════════════════════════════
CREATE TABLE integration_sync_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sync_config_id UUID NOT NULL REFERENCES integration_sync_configs(id) ON DELETE CASCADE,
    connection_id UUID NOT NULL REFERENCES integration_connections(id) ON DELETE CASCADE,

    -- External identity
    external_type VARCHAR(255) NOT NULL,
    external_id VARCHAR(500) NOT NULL,

    -- Internal identity (AMOS collection record)
    internal_collection VARCHAR(255),
    internal_record_id UUID,

    -- Change detection
    data_hash VARCHAR(64),       -- SHA-256 of the external record for change detection
    last_external_data JSONB,    -- cached copy of last synced external data

    -- Status
    sync_status VARCHAR(50) NOT NULL DEFAULT 'synced',
    -- synced, pending, error, deleted, orphaned
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,

    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_sync_records_unique ON integration_sync_records(connection_id, external_type, external_id);
CREATE INDEX idx_sync_records_config ON integration_sync_records(sync_config_id);
CREATE INDEX idx_sync_records_internal ON integration_sync_records(internal_collection, internal_record_id);
CREATE INDEX idx_sync_records_status ON integration_sync_records(sync_status);

-- ═══════════════════════════════════════════════════════════════════════════
-- 3. SYNC CURSORS - Incremental pagination state
-- ═══════════════════════════════════════════════════════════════════════════
CREATE TABLE integration_sync_cursors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sync_config_id UUID NOT NULL REFERENCES integration_sync_configs(id) ON DELETE CASCADE,

    -- Cursor state
    cursor_type VARCHAR(50) NOT NULL DEFAULT 'timestamp',
    -- timestamp, offset, token, page
    cursor_value TEXT,           -- the actual cursor/page token/offset
    cursor_field VARCHAR(255),   -- which field the cursor tracks, e.g. "updated_at"

    -- Progress
    records_synced BIGINT NOT NULL DEFAULT 0,
    is_complete BOOLEAN NOT NULL DEFAULT false,  -- true when full sync finished

    -- Timing
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sync_cursors_config ON integration_sync_cursors(sync_config_id);
CREATE UNIQUE INDEX idx_sync_cursors_active ON integration_sync_cursors(sync_config_id) WHERE is_complete = false;
