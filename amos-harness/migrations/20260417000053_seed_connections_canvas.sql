-- Seed the Connections canvas — visual management of all integration
-- credentials (OAuth2, API keys, bearer tokens). Creation is handled via
-- Amos (the agent drives the conversational setup flow), so this canvas
-- is primarily for review, status, test, and revoke.

INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-connections',
    'Connections',
    'Manage OAuth and API credentials for external services',
    'custom',
    true,
    'link-2',
    12,
    '<div class="container-fluid p-4" style="max-width:1100px">
    <h2 class="mb-4"><i data-lucide="link-2" style="width:22px;height:22px;display:inline-block;vertical-align:middle;margin-right:8px"></i>Connections</h2>

    <div class="alert alert-info small" role="alert">
        <strong>Ask Amos to connect a service.</strong> Say
        <em>"connect my Google Calendar"</em> or
        <em>"hook up Slack"</em> and Amos will walk you through creating
        an OAuth app in the provider''s developer console and pasting in
        your credentials. You can also set up custom providers.
    </div>

    <div class="card mb-4">
        <div class="card-header d-flex justify-content-between align-items-center">
            <h5 class="mb-0">Active Connections</h5>
            <button class="btn btn-sm btn-outline-secondary" onclick="loadAll()"><i data-lucide="refresh-cw" style="width:14px;height:14px"></i> Refresh</button>
        </div>
        <div class="card-body">
            <div id="connectionsList"><div class="text-muted small">Loading...</div></div>
        </div>
    </div>

    <div class="card mb-4">
        <div class="card-header">
            <h5 class="mb-0">Supported OAuth Providers</h5>
        </div>
        <div class="card-body">
            <p class="text-muted small mb-3">Services Amos knows about. Custom providers are also supported — just ask Amos to connect one.</p>
            <div id="providersList"><div class="text-muted small">Loading...</div></div>
        </div>
    </div>
</div>',

    'async function loadConnections() {
    var list = document.getElementById("connectionsList");
    try {
        var resp = await fetch("/api/v1/connections");
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        var rows = await resp.json();
        if (rows.length === 0) {
            list.innerHTML = "<div class=\"text-muted small py-3\">No connections yet. Ask Amos to hook up a service.</div>";
            return;
        }
        list.innerHTML = rows.map(function(c) {
            var statusClass = c.status === "active" ? "success" : c.status === "pending" ? "warning text-dark" : c.status === "expired" ? "warning text-dark" : "secondary";
            var statusBadge = "<span class=\"badge bg-" + statusClass + "\">" + escapeHtml(c.status) + "</span>";
            var typeBadge = "<span class=\"badge bg-light text-dark ms-1\">" + escapeHtml(c.auth_type) + "</span>";
            var label = escapeHtml(c.label || "(unnamed)");
            var integration = c.integration_name ? " &middot; " + escapeHtml(c.integration_name) : "";
            var scopes = c.oauth_scopes ? "<div class=\"small text-muted\" style=\"word-break:break-all\">Scopes: " + escapeHtml(c.oauth_scopes) + "</div>" : "";
            var expires = "";
            if (c.token_expires_at) {
                var when = new Date(c.token_expires_at);
                var past = when < new Date();
                expires = "<div class=\"small " + (past ? "text-danger" : "text-muted") + "\">Token " + (past ? "expired" : "expires") + " " + when.toLocaleString() + "</div>";
            }
            var lastUsed = c.last_used_at ? "<div class=\"small text-muted\">Last used " + new Date(c.last_used_at).toLocaleString() + "</div>" : "";
            var revokeBtn = c.status !== "revoked"
                ? "<button class=\"btn btn-sm btn-outline-danger\" onclick=\"revoke(''" + c.id + "'')\">Revoke</button>"
                : "<span class=\"badge bg-secondary\">revoked</span>";
            return "<div class=\"border rounded p-3 mb-2\">" +
                "<div class=\"d-flex justify-content-between align-items-start gap-2\">" +
                    "<div style=\"flex:1;min-width:0\">" +
                        "<strong>" + label + "</strong> " + statusBadge + typeBadge +
                        "<div class=\"small text-muted\">" + escapeHtml(c.id.substring(0,8)) + integration + "</div>" +
                        scopes + expires + lastUsed +
                    "</div>" +
                    "<div>" + revokeBtn + "</div>" +
                "</div>" +
            "</div>";
        }).join("");
        if (typeof lucide !== "undefined") lucide.createIcons();
    } catch(err) {
        list.innerHTML = "<div class=\"text-danger small\">Failed to load: " + escapeHtml(err.message) + "</div>";
    }
}

async function loadProviders() {
    var list = document.getElementById("providersList");
    try {
        var resp = await fetch("/api/v1/connections/providers");
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        var rows = await resp.json();
        if (rows.length === 0) {
            list.innerHTML = "<div class=\"text-muted small\">No providers in directory.</div>";
            return;
        }
        list.innerHTML = "<div class=\"row g-2\">" + rows.map(function(p) {
            var docs = p.docs_url ? "<a href=\"" + escapeHtml(p.docs_url) + "\" target=\"_blank\" class=\"small\">docs</a>" : "";
            var console = p.app_creation_url ? "<a href=\"" + escapeHtml(p.app_creation_url) + "\" target=\"_blank\" class=\"small ms-2\">console</a>" : "";
            return "<div class=\"col-md-6\"><div class=\"border rounded p-2 h-100\">" +
                "<div><strong>" + escapeHtml(p.name) + "</strong> <code class=\"small text-muted\">" + escapeHtml(p.slug) + "</code></div>" +
                "<div class=\"small text-muted\" style=\"word-break:break-all\">" + escapeHtml(p.default_scopes || "") + "</div>" +
                "<div class=\"mt-1\">" + docs + console + "</div>" +
            "</div></div>";
        }).join("") + "</div>";
    } catch(err) {
        list.innerHTML = "<div class=\"text-danger small\">Failed to load providers: " + escapeHtml(err.message) + "</div>";
    }
}

async function revoke(id) {
    if (!confirm("Revoke this connection? The credential will be marked revoked and tokens cleared.")) return;
    try {
        var resp = await fetch("/api/v1/connections/" + id + "/revoke", { method: "POST" });
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        await loadAll();
    } catch(err) {
        alert("Revoke failed: " + err.message);
    }
}

function escapeHtml(s) {
    if (!s) return "";
    var d = document.createElement("div");
    d.textContent = s;
    return d.innerHTML;
}

function loadAll() {
    loadConnections();
    loadProviders();
}

document.addEventListener("DOMContentLoaded", loadAll);',

    '.card-header { background-color: rgba(0,0,0,0.03); }
code.small { font-size: 0.8em; }'
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    description = EXCLUDED.description,
    is_system = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();
