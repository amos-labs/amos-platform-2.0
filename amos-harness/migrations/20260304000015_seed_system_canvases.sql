-- Seed system canvases for navigation
-- These replace the hardcoded HTML views in index.html

-- ============================================================================
-- 1. Canvases (list of user canvases)
-- ============================================================================
INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-canvases',
    'Canvases',
    'Browse and manage your canvases',
    'custom',
    true,
    'layout-dashboard',
    1,
    -- HTML
    '<div class="container-fluid p-4">
    <div class="d-flex justify-content-between align-items-center mb-4">
        <h2>Canvases</h2>
        <button class="btn btn-primary" onclick="createNewCanvas()">
            <i data-lucide="plus" style="width:16px;height:16px;display:inline-block;vertical-align:middle;margin-right:4px"></i> New Canvas
        </button>
    </div>
    <div id="canvas-grid" class="row g-3">
        <div class="text-center text-muted py-5">Loading canvases...</div>
    </div>
</div>',
    -- JS
    'document.addEventListener("DOMContentLoaded", function() { loadCanvasList(); });

async function loadCanvasList() {
    try {
        var resp = await fetch("/api/v1/canvases");
        var canvases = await resp.json();
        var grid = document.getElementById("canvas-grid");
        // Filter out system canvases
        canvases = canvases.filter(function(c) { return !c.is_system; });
        if (canvases.length === 0) {
            grid.innerHTML = "<div class=\"col-12 text-center text-muted py-5\"><p>No canvases yet.</p><p>Ask AMOS to create one, or click New Canvas.</p></div>";
            return;
        }
        grid.innerHTML = canvases.map(function(c) {
            var typeColor = {"dashboard":"primary","kanban":"success","datagrid":"info","calendar":"warning","form":"secondary"}[c.canvas_type] || "secondary";
            return "<div class=\"col-md-6 col-lg-4\"><div class=\"card h-100 canvas-card\" style=\"cursor:pointer\" onclick=\"openCanvasById(''" + c.id + "'')\"><div class=\"card-body\"><div class=\"d-flex justify-content-between mb-2\"><h5 class=\"card-title mb-0\">" + c.name + "</h5><span class=\"badge bg-" + typeColor + "\">" + c.canvas_type + "</span></div><p class=\"card-text text-muted small\">" + (c.description || "No description") + "</p></div><div class=\"card-footer bg-transparent text-muted small\">Updated " + new Date(c.updated_at).toLocaleDateString() + "</div></div></div>";
        }).join("");
        if (typeof lucide !== "undefined") lucide.createIcons();
    } catch(err) { console.error("Failed to load canvases:", err); }
}

function createNewCanvas() {
    var name = prompt("Canvas name:");
    if (!name) return;
    // Send message to parent to create via chat
    window.parent.postMessage({ type: "amos-chat", message: "Create a new canvas called " + name }, "*");
}

function openCanvasById(id) {
    window.parent.postMessage({ type: "amos-open-canvas", canvasId: id }, "*");
}',
    -- CSS
    '.canvas-card { transition: transform 0.15s, box-shadow 0.15s; }
.canvas-card:hover { transform: translateY(-2px); box-shadow: 0 4px 12px rgba(0,0,0,0.1); }'
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    is_system = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();

-- ============================================================================
-- 2. Bots
-- ============================================================================
INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-bots',
    'Bots',
    'Manage your messaging bots',
    'custom',
    true,
    'bot',
    2,
    '<div class="container-fluid p-4">
    <div class="d-flex justify-content-between align-items-center mb-4">
        <h2>Bots</h2>
        <button class="btn btn-primary" onclick="createNewBot()">
            <i data-lucide="plus" style="width:16px;height:16px;display:inline-block;vertical-align:middle;margin-right:4px"></i> New Bot
        </button>
    </div>
    <div id="bots-list" class="row g-3">
        <div class="text-center text-muted py-5">Loading bots...</div>
    </div>
</div>',
    'document.addEventListener("DOMContentLoaded", function() { loadBotsList(); });

async function loadBotsList() {
    try {
        var resp = await fetch("/api/v1/bots");
        if (!resp.ok) throw new Error("Failed to fetch");
        var bots = await resp.json();
        var list = document.getElementById("bots-list");
        if (bots.length === 0) {
            list.innerHTML = "<div class=\"col-12 text-center text-muted py-5\"><p>No bots configured yet.</p><p>Ask AMOS to create a Telegram or messaging bot.</p></div>";
            return;
        }
        list.innerHTML = bots.map(function(b) {
            var statusClass = b.status === "running" ? "success" : "secondary";
            return "<div class=\"col-md-6 col-lg-4\"><div class=\"card h-100\"><div class=\"card-body\"><div class=\"d-flex justify-content-between mb-2\"><h5 class=\"card-title mb-0\">" + (b.name || b.bot_name || "Unnamed Bot") + "</h5><span class=\"badge bg-" + statusClass + "\">" + (b.status || "stopped") + "</span></div><p class=\"card-text text-muted small\">" + (b.platform || b.bot_type || "unknown") + "</p></div></div></div>";
        }).join("");
        if (typeof lucide !== "undefined") lucide.createIcons();
    } catch(err) {
        document.getElementById("bots-list").innerHTML = "<div class=\"col-12 text-center text-muted py-5\"><p>No bots configured yet.</p><p>Ask AMOS to create a Telegram or messaging bot.</p></div>";
    }
}

function createNewBot() {
    window.parent.postMessage({ type: "amos-chat", message: "Create a new bot" }, "*");
}',
    ''
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    is_system = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();

-- ============================================================================
-- 3. Integrations
-- ============================================================================
INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-integrations',
    'Integrations',
    'Connect external services and APIs',
    'custom',
    true,
    'plug',
    3,
    '<div class="container-fluid p-4">
    <div class="d-flex justify-content-between align-items-center mb-4">
        <h2>Integrations</h2>
    </div>
    <div id="integrations-list" class="row g-3">
        <div class="text-center text-muted py-5">Loading integrations...</div>
    </div>
</div>',
    'document.addEventListener("DOMContentLoaded", function() { loadIntegrationsList(); });

async function loadIntegrationsList() {
    try {
        var resp = await fetch("/api/v1/integrations");
        if (!resp.ok) throw new Error("Failed to fetch");
        var integrations = await resp.json();
        var list = document.getElementById("integrations-list");
        if (!integrations || integrations.length === 0) {
            list.innerHTML = "<div class=\"col-12 text-center text-muted py-5\"><p>No integrations configured.</p><p>Ask AMOS to connect a service like a CRM, email, or database.</p></div>";
            return;
        }
        list.innerHTML = integrations.map(function(i) {
            var statusClass = i.enabled ? "success" : "secondary";
            return "<div class=\"col-md-6 col-lg-4\"><div class=\"card h-100\"><div class=\"card-body\"><div class=\"d-flex justify-content-between mb-2\"><h5 class=\"card-title mb-0\">" + (i.name || "Integration") + "</h5><span class=\"badge bg-" + statusClass + "\">" + (i.enabled ? "Active" : "Inactive") + "</span></div><p class=\"card-text text-muted small\">" + (i.integration_type || "custom") + "</p></div></div></div>";
        }).join("");
    } catch(err) {
        document.getElementById("integrations-list").innerHTML = "<div class=\"col-12 text-center text-muted py-5\"><p>No integrations configured.</p><p>Ask AMOS to connect a service.</p></div>";
    }
}',
    ''
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    is_system = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();

-- ============================================================================
-- 4. External Agents
-- ============================================================================
INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-agents',
    'External Agents',
    'Manage OpenClaw external agents',
    'custom',
    true,
    'users',
    4,
    '<div class="container-fluid p-4">
    <div class="d-flex justify-content-between align-items-center mb-4">
        <h2>External Agents</h2>
    </div>
    <div id="agents-list" class="row g-3">
        <div class="text-center text-muted py-5">Loading agents...</div>
    </div>
</div>',
    'document.addEventListener("DOMContentLoaded", function() { loadAgentsList(); });

async function loadAgentsList() {
    try {
        var resp = await fetch("/api/v1/agents");
        if (!resp.ok) throw new Error("Failed to fetch");
        var agents = await resp.json();
        var list = document.getElementById("agents-list");
        if (!agents || agents.length === 0) {
            list.innerHTML = "<div class=\"col-12 text-center text-muted py-5\"><p>No external agents registered.</p><p>External agents connect via the OpenClaw protocol to contribute work.</p></div>";
            return;
        }
        list.innerHTML = agents.map(function(a) {
            var statusClass = a.status === "active" ? "success" : "secondary";
            return "<div class=\"col-md-6 col-lg-4\"><div class=\"card h-100\"><div class=\"card-body\"><div class=\"d-flex justify-content-between mb-2\"><h5 class=\"card-title mb-0\">" + (a.name || "Agent") + "</h5><span class=\"badge bg-" + statusClass + "\">" + (a.status || "inactive") + "</span></div><p class=\"card-text text-muted small\">" + (a.description || "No description") + "</p></div></div></div>";
        }).join("");
    } catch(err) {
        document.getElementById("agents-list").innerHTML = "<div class=\"col-12 text-center text-muted py-5\"><p>No external agents registered.</p></div>";
    }
}',
    ''
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    is_system = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();

-- ============================================================================
-- 5. Settings
-- ============================================================================
INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-settings',
    'Settings',
    'Configure AMOS settings',
    'custom',
    true,
    'settings',
    10,
    '<div class="container-fluid p-4" style="max-width:720px">
    <h2 class="mb-4">Settings</h2>
    <div class="card mb-3">
        <div class="card-body">
            <h5 class="card-title">AI Model</h5>
            <select id="modelSelect" class="form-select" onchange="saveModelSetting(this.value)">
                <option value="us.anthropic.claude-sonnet-4-20250514-v1:0">Claude Sonnet 4</option>
                <option value="us.anthropic.claude-opus-4-20250514-v1:0">Claude Opus 4</option>
                <option value="us.anthropic.claude-3-5-haiku-20241022-v1:0">Claude 3.5 Haiku</option>
            </select>
        </div>
    </div>
    <div class="card mb-3">
        <div class="card-body">
            <h5 class="card-title">Platform Connection</h5>
            <label class="form-label">Platform URL</label>
            <input type="text" class="form-control" value="http://localhost:4000" readonly>
        </div>
    </div>
    <div class="card mb-3">
        <div class="card-body">
            <h5 class="card-title">About</h5>
            <p class="text-muted mb-0">AMOS Harness v0.1.0</p>
        </div>
    </div>
</div>',
    'document.addEventListener("DOMContentLoaded", function() {
    var saved = localStorage.getItem("amos-model");
    if (saved) {
        var sel = document.getElementById("modelSelect");
        if (sel) sel.value = saved;
    }
});

function saveModelSetting(value) {
    localStorage.setItem("amos-model", value);
    window.parent.postMessage({ type: "amos-setting", key: "model", value: value }, "*");
}',
    ''
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    is_system = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();
