-- Sites: standalone multi-page websites and landing pages
-- The AI generates full HTML pages served at public URLs.
-- Forms on these pages submit data into collections via a POST endpoint.

CREATE TABLE sites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(500) NOT NULL,                    -- "Acme Marketing Site"
    slug VARCHAR(255) NOT NULL UNIQUE,             -- URL path segment: "acme-marketing"
    description TEXT,
    domain VARCHAR(500),                           -- optional custom domain
    is_published BOOLEAN NOT NULL DEFAULT false,
    published_at TIMESTAMPTZ,

    -- Site-wide settings
    settings JSONB NOT NULL DEFAULT '{}',          -- {analytics_id, favicon_url, og_image, theme_color, etc.}
    metadata JSONB NOT NULL DEFAULT '{}',          -- {seo_title, seo_description, social_image, etc.}

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_sites_slug ON sites(slug);
CREATE INDEX idx_sites_domain ON sites(domain) WHERE domain IS NOT NULL;

CREATE TABLE pages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    path VARCHAR(500) NOT NULL DEFAULT '/',         -- URL path: "/", "/about", "/pricing"
    title VARCHAR(500) NOT NULL,
    description TEXT,

    -- Content
    html_content TEXT NOT NULL,
    css_content TEXT,
    js_content TEXT,

    -- Page-level SEO & metadata
    meta_title VARCHAR(500),                        -- <title> override (falls back to title)
    meta_description TEXT,                          -- <meta name="description">
    og_image VARCHAR(1000),                         -- Open Graph image URL

    -- Form configuration — which collection receives form submissions
    form_collection VARCHAR(255),                   -- collection slug for form data (e.g. "leads")

    -- Ordering
    sort_order INTEGER NOT NULL DEFAULT 0,
    is_published BOOLEAN NOT NULL DEFAULT true,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(site_id, path)
);

CREATE INDEX idx_pages_site ON pages(site_id);
CREATE INDEX idx_pages_site_path ON pages(site_id, path);
