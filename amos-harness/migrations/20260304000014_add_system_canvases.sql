-- Add system canvas support
-- System canvases are pre-built views that appear as navigation items (Canvases, Bots, Integrations, etc.)
-- They are DB-backed just like regular canvases but marked with is_system = true

-- Add is_system flag and nav metadata to canvases table
ALTER TABLE canvases ADD COLUMN IF NOT EXISTS is_system BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE canvases ADD COLUMN IF NOT EXISTS nav_icon VARCHAR(100);
ALTER TABLE canvases ADD COLUMN IF NOT EXISTS nav_order INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_canvases_system ON canvases(nav_order) WHERE is_system = true;
