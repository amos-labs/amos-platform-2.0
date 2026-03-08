-- Entity Revisions: Generic change tracking for any entity in the system.
--
-- This is the foundation for:
-- 1. Tracking every change made to templated entities (integrations, canvases, collections, sites, etc.)
-- 2. Allowing users to revert to any previous version
-- 3. Supporting the template sync protocol (diff local changes against upstream updates)
--
-- Design decisions:
-- - Single polymorphic table rather than per-entity revision tables (simpler, unified API)
-- - Full JSON snapshots for each revision (enables point-in-time restore without complex diff replay)
-- - Optional JSON diff stored alongside snapshot (enables efficient change viewing)
-- - SHA-256 content hash for fast equality checks during sync

CREATE TABLE IF NOT EXISTS entity_revisions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Polymorphic reference: which entity table + which row
    entity_type     VARCHAR(100)    NOT NULL,   -- e.g. 'integration', 'canvas', 'collection', 'site', 'page'
    entity_id       UUID            NOT NULL,   -- FK to the entity's primary key

    -- Version tracking
    version         INTEGER         NOT NULL,   -- 1-based, monotonically increasing per entity
    content_hash    VARCHAR(64)     NOT NULL,   -- SHA-256 of the canonical JSON snapshot

    -- Snapshot: full state of the entity at this version
    snapshot        JSONB           NOT NULL,   -- complete serialized entity

    -- Diff: what changed from the previous version (RFC 6902 JSON Patch format)
    -- NULL for the initial version (version=1)
    diff_from_prev  JSONB,

    -- Who/what made the change
    change_type     VARCHAR(50)     NOT NULL DEFAULT 'manual',
        -- 'manual'        = user made the change via UI/API
        -- 'ai_agent'      = AI agent made the change via tool
        -- 'template_sync' = synced from upstream template
        -- 'revert'        = reverted to a previous version
        -- 'system'        = system migration or seed
    changed_by      VARCHAR(255),               -- user ID, agent name, or 'system'
    change_summary  TEXT,                       -- human-readable description of what changed

    -- Template lineage (if this entity was forked from a template)
    template_id     UUID,                       -- upstream template entity ID (NULL if not from template)
    template_version INTEGER,                   -- which template version this was based on

    created_at      TIMESTAMPTZ     NOT NULL DEFAULT now()
);

-- Fast lookups: all revisions for a specific entity, ordered by version
CREATE INDEX idx_entity_revisions_entity
    ON entity_revisions (entity_type, entity_id, version DESC);

-- Fast lookup: find the latest revision for each entity
CREATE INDEX idx_entity_revisions_latest
    ON entity_revisions (entity_type, entity_id, version DESC)
    INCLUDE (content_hash);

-- Find all revisions by a specific actor
CREATE INDEX idx_entity_revisions_changed_by
    ON entity_revisions (changed_by, created_at DESC)
    WHERE changed_by IS NOT NULL;

-- Find all revisions of a specific change type
CREATE INDEX idx_entity_revisions_change_type
    ON entity_revisions (change_type, created_at DESC);

-- Uniqueness: one version number per entity
CREATE UNIQUE INDEX idx_entity_revisions_unique_version
    ON entity_revisions (entity_type, entity_id, version);


-- Template Registry: Tracks which templates are available and their versions.
-- In a multi-tenant deployment, the admin instance populates this; customer instances
-- sync from it. In single-instance mode, the seed data acts as the initial template set.

CREATE TABLE IF NOT EXISTS template_registry (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What kind of entity this template defines
    entity_type     VARCHAR(100)    NOT NULL,   -- 'integration', 'canvas', 'collection', 'site'

    -- Template identity
    slug            VARCHAR(255)    NOT NULL,   -- unique within entity_type, e.g. 'stripe', 'gmail'
    name            VARCHAR(500)    NOT NULL,   -- human-readable name
    description     TEXT,

    -- Current published version
    current_version INTEGER         NOT NULL DEFAULT 1,
    content_hash    VARCHAR(64)     NOT NULL,   -- SHA-256 of current snapshot

    -- The full snapshot of the current template version
    snapshot        JSONB           NOT NULL,

    -- Template metadata
    category        VARCHAR(100),               -- e.g. 'payment', 'communication', 'productivity'
    icon_url        VARCHAR(500),
    tags            JSONB           DEFAULT '[]'::jsonb,
    metadata        JSONB           DEFAULT '{}'::jsonb,

    -- Publishing
    is_published    BOOLEAN         NOT NULL DEFAULT true,
    published_at    TIMESTAMPTZ,
    deprecated_at   TIMESTAMPTZ,                -- soft-deprecate without removing

    created_at      TIMESTAMPTZ     NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_template_registry_slug
    ON template_registry (entity_type, slug);

CREATE INDEX idx_template_registry_category
    ON template_registry (entity_type, category)
    WHERE is_published = true;


-- Template Versions: Historical versions of each template for rollback and diffing.

CREATE TABLE IF NOT EXISTS template_versions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id     UUID            NOT NULL REFERENCES template_registry(id) ON DELETE CASCADE,

    version         INTEGER         NOT NULL,
    content_hash    VARCHAR(64)     NOT NULL,
    snapshot        JSONB           NOT NULL,
    diff_from_prev  JSONB,                      -- JSON Patch from previous version

    -- Release info
    release_notes   TEXT,
    is_breaking     BOOLEAN         NOT NULL DEFAULT false,

    created_at      TIMESTAMPTZ     NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_template_versions_unique
    ON template_versions (template_id, version);


-- Template Subscriptions: Tracks which entities on this instance are linked to
-- upstream templates and their sync status.

CREATE TABLE IF NOT EXISTS template_subscriptions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The local entity that was forked from a template
    entity_type     VARCHAR(100)    NOT NULL,
    entity_id       UUID            NOT NULL,

    -- The template it's subscribed to
    template_id     UUID            NOT NULL REFERENCES template_registry(id) ON DELETE SET NULL,

    -- Sync state
    installed_version   INTEGER     NOT NULL,       -- which template version was installed
    latest_version      INTEGER,                    -- latest available from registry (updated on check)
    customization_status VARCHAR(50) NOT NULL DEFAULT 'stock',
        -- 'stock'      = entity matches the template exactly
        -- 'customized' = user has made local changes
        -- 'outdated'   = new template version available, no local changes
        -- 'diverged'   = new template version available AND user has local changes
    local_content_hash  VARCHAR(64),                -- hash of entity's current state
    template_content_hash VARCHAR(64),              -- hash of template at installed_version

    -- Sync preferences
    auto_update     BOOLEAN         NOT NULL DEFAULT false,  -- auto-apply non-breaking updates?
    pin_version     BOOLEAN         NOT NULL DEFAULT false,  -- ignore all updates?

    last_checked_at TIMESTAMPTZ,
    last_synced_at  TIMESTAMPTZ,

    created_at      TIMESTAMPTZ     NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT now()
);

-- One subscription per entity
CREATE UNIQUE INDEX idx_template_subscriptions_entity
    ON template_subscriptions (entity_type, entity_id);

-- Find all entities subscribed to a template
CREATE INDEX idx_template_subscriptions_template
    ON template_subscriptions (template_id, customization_status);
