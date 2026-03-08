-- Add template tracking and revision columns to existing entities.
--
-- Entity coverage:
--   Integrations          -> revision tracking + template sync
--   Integration Operations -> revision tracking + template sync
--   Canvases              -> revision tracking + template sync (already has some version columns)
--   Collections           -> revision tracking + template sync
--   Sites                 -> revision tracking only (no templates)
--   Pages                 -> revision tracking only (no templates)
--
-- The `entity_revisions` table is generic and works with any entity_type.
-- These columns are denormalized hints on the entity itself for fast queries
-- without joining to entity_revisions.

-- ────────────────────────────────────────────────────────────────────────────
-- Integrations: add version tracking + template reference
-- ────────────────────────────────────────────────────────────────────────────
ALTER TABLE integrations
    ADD COLUMN IF NOT EXISTS version             INTEGER     NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS content_hash        VARCHAR(64),
    ADD COLUMN IF NOT EXISTS template_slug       VARCHAR(255),         -- e.g. 'stripe', 'gmail' — links to template_registry
    ADD COLUMN IF NOT EXISTS template_version    INTEGER,              -- which template version this was installed from
    ADD COLUMN IF NOT EXISTS customization_status VARCHAR(50) DEFAULT 'stock';
        -- 'stock', 'customized', 'outdated', 'diverged'

-- ────────────────────────────────────────────────────────────────────────────
-- Integration Operations: add version tracking + template reference
-- ────────────────────────────────────────────────────────────────────────────
ALTER TABLE integration_operations
    ADD COLUMN IF NOT EXISTS version             INTEGER     NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS content_hash        VARCHAR(64),
    ADD COLUMN IF NOT EXISTS template_slug       VARCHAR(255),
    ADD COLUMN IF NOT EXISTS template_version    INTEGER,
    ADD COLUMN IF NOT EXISTS customization_status VARCHAR(50) DEFAULT 'stock';

-- ────────────────────────────────────────────────────────────────────────────
-- Collections: add version tracking + template reference
-- ────────────────────────────────────────────────────────────────────────────
ALTER TABLE collections
    ADD COLUMN IF NOT EXISTS version             INTEGER     NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS content_hash        VARCHAR(64),
    ADD COLUMN IF NOT EXISTS template_slug       VARCHAR(255),
    ADD COLUMN IF NOT EXISTS template_version    INTEGER,
    ADD COLUMN IF NOT EXISTS customization_status VARCHAR(50) DEFAULT 'stock';

-- ────────────────────────────────────────────────────────────────────────────
-- Sites: revision tracking only (no template columns)
-- ────────────────────────────────────────────────────────────────────────────
ALTER TABLE sites
    ADD COLUMN IF NOT EXISTS version             INTEGER     NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS content_hash        VARCHAR(64);

-- ────────────────────────────────────────────────────────────────────────────
-- Pages: revision tracking only (no template columns)
-- ────────────────────────────────────────────────────────────────────────────
ALTER TABLE pages
    ADD COLUMN IF NOT EXISTS version             INTEGER     NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS content_hash        VARCHAR(64);

-- ────────────────────────────────────────────────────────────────────────────
-- Canvases: already has version, template_key, source_template_version, previous_versions.
-- Add content_hash and customization_status to align with the new pattern.
-- Map existing columns: template_key -> template_slug pattern,
--                       source_template_version -> template_version pattern.
-- ────────────────────────────────────────────────────────────────────────────
ALTER TABLE canvases
    ADD COLUMN IF NOT EXISTS content_hash         VARCHAR(64),
    ADD COLUMN IF NOT EXISTS customization_status VARCHAR(50) DEFAULT 'stock';

-- Note: canvases.template_key serves the same purpose as template_slug on other tables,
-- and canvases.source_template_version serves as template_version.
-- We keep the existing column names for backward compatibility; the Rust code
-- will handle the mapping.


-- ────────────────────────────────────────────────────────────────────────────
-- Seed template_registry entries from existing integration seed data.
-- This creates template records for each of the 7 integrations so that
-- customer instances can subscribe to them.
-- ────────────────────────────────────────────────────────────────────────────
INSERT INTO template_registry (entity_type, slug, name, description, current_version, content_hash, snapshot, category, icon_url, metadata, is_published, published_at)
SELECT
    'integration',
    LOWER(REPLACE(i.name, ' ', '_')),               -- slug: 'stripe', 'gmail', 'quickbooks_online', etc.
    i.name,
    (i.metadata->>'description'),
    1,                                                -- initial version
    md5(i.metadata::text),                            -- content_hash (md5 placeholder until Rust computes SHA-256)
    jsonb_build_object(
        'name', i.name,
        'connector_type', i.connector_type,
        'endpoint_url', i.endpoint_url,
        'available_actions', i.available_actions,
        'metadata', i.metadata
    ),
    (i.metadata->>'category'),
    (i.metadata->>'icon_url'),
    jsonb_build_object('auth_type', i.metadata->>'auth_type'),
    true,
    now()
FROM integrations i
ON CONFLICT (entity_type, slug) DO NOTHING;

-- Also seed template_versions for version 1 of each
INSERT INTO template_versions (template_id, version, content_hash, snapshot, release_notes)
SELECT
    tr.id,
    1,
    tr.content_hash,
    tr.snapshot,
    'Initial seed version'
FROM template_registry tr
WHERE tr.entity_type = 'integration'
  AND NOT EXISTS (
      SELECT 1 FROM template_versions tv
      WHERE tv.template_id = tr.id AND tv.version = 1
  );

-- Seed template_subscriptions linking each integration to its template
INSERT INTO template_subscriptions (entity_type, entity_id, template_id, installed_version, customization_status, local_content_hash, template_content_hash, last_synced_at)
SELECT
    'integration',
    i.id,
    tr.id,
    1,
    'stock',
    md5(i.metadata::text),
    tr.content_hash,
    now()
FROM integrations i
JOIN template_registry tr
    ON tr.entity_type = 'integration'
    AND tr.slug = LOWER(REPLACE(i.name, ' ', '_'))
ON CONFLICT (entity_type, entity_id) DO NOTHING;

-- Backfill template_slug on integrations from the template_registry
UPDATE integrations i
SET
    template_slug = tr.slug,
    template_version = 1,
    customization_status = 'stock'
FROM template_registry tr
WHERE tr.entity_type = 'integration'
  AND tr.slug = LOWER(REPLACE(i.name, ' ', '_'))
  AND i.template_slug IS NULL;
