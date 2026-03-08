-- Canvas system: the dynamic UI that AMOS generates and manages
CREATE TABLE canvas_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key VARCHAR(255) NOT NULL UNIQUE,
    name VARCHAR(500) NOT NULL,
    canvas_type VARCHAR(100) NOT NULL, -- dynamic_canvas, freeform_canvas, dashboard, data_grid, form, detail, kanban, calendar, report, wizard, custom
    html_content TEXT,
    js_content TEXT,
    css_content TEXT,
    metadata JSONB DEFAULT '{}',
    version INTEGER NOT NULL DEFAULT 1,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_canvas_templates_key ON canvas_templates(key) WHERE active = true;

CREATE TABLE canvases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(255) NOT NULL,
    name VARCHAR(500) NOT NULL,
    description TEXT,

    -- Content
    html_content TEXT,
    js_content TEXT,
    css_content TEXT,

    -- Canvas configuration
    canvas_type VARCHAR(100) NOT NULL DEFAULT 'custom',
    ui_mode VARCHAR(50) NOT NULL DEFAULT 'simple', -- simple or advanced
    data_sources JSONB DEFAULT '[]', -- [{type, model, scope, limit, filters}]
    actions JSONB DEFAULT '[]', -- [{name, tool, icon, label}]
    layout_config JSONB DEFAULT '{}', -- {columns, show_header, show_search, etc.}

    -- Template linkage
    template_key VARCHAR(255) REFERENCES canvas_templates(key),
    source_template_version INTEGER,

    -- Versioning
    version INTEGER NOT NULL DEFAULT 1,
    previous_versions JSONB DEFAULT '[]',

    -- Publishing
    is_public BOOLEAN NOT NULL DEFAULT false,
    public_slug VARCHAR(255) UNIQUE,
    published_at TIMESTAMPTZ,
    view_count INTEGER NOT NULL DEFAULT 0,

    -- Locking
    is_locked BOOLEAN NOT NULL DEFAULT false,
    locked_at TIMESTAMPTZ,
    locked_by VARCHAR(255),
    lock_reason VARCHAR(500),

    -- Scope (for template overrides)
    is_override BOOLEAN NOT NULL DEFAULT false,
    scope_type VARCHAR(50), -- 'system', 'entity', 'user'
    scope_id VARCHAR(255),

    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_canvases_slug ON canvases(slug);
CREATE INDEX idx_canvases_type ON canvases(canvas_type);
CREATE INDEX idx_canvases_public ON canvases(public_slug) WHERE is_public = true;
CREATE INDEX idx_canvases_template ON canvases(template_key) WHERE template_key IS NOT NULL;
CREATE INDEX idx_canvases_scope ON canvases(scope_type, scope_id) WHERE is_override = true;
