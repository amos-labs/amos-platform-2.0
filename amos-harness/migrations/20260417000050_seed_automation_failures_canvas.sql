-- Seed the Automation Failures canvas — displays recent failed runs and the
-- dead-letter retry queue with requeue buttons.

INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-automation-failures',
    'Automation Failures',
    'Review failed automation runs and dead-letter webhook retries',
    'custom',
    true,
    'alert-triangle',
    11,
    '<div class="container-fluid p-4" style="max-width:1100px">
    <h2 class="mb-4"><i data-lucide="alert-triangle" style="width:22px;height:22px;display:inline-block;vertical-align:middle;margin-right:8px"></i>Automation Failures</h2>

    <!-- ── Dead Letters ── -->
    <div class="card mb-4">
        <div class="card-header d-flex justify-content-between align-items-center">
            <h5 class="mb-0">Dead-Letter Queue</h5>
            <button class="btn btn-sm btn-outline-secondary" onclick="loadAll()"><i data-lucide="refresh-cw" style="width:14px;height:14px"></i> Refresh</button>
        </div>
        <div class="card-body">
            <p class="text-muted small mb-3">Webhook retries that have permanently failed after 3 attempts. Inspect the error, fix the target endpoint, then requeue to retry.</p>
            <div id="deadLettersList"><div class="text-muted small">Loading...</div></div>
        </div>
    </div>

    <!-- ── Recent Failures ── -->
    <div class="card mb-4">
        <div class="card-header">
            <h5 class="mb-0">Recent Failed Runs</h5>
        </div>
        <div class="card-body">
            <p class="text-muted small mb-3">Automation runs that errored in the last 50 executions (across all action types).</p>
            <div id="failuresList"><div class="text-muted small">Loading...</div></div>
        </div>
    </div>
</div>',

    'async function loadDeadLetters() {
    var list = document.getElementById("deadLettersList");
    try {
        var resp = await fetch("/api/v1/automations/dead-letters");
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        var rows = await resp.json();
        if (rows.length === 0) {
            list.innerHTML = "<div class=\"text-muted small py-2\">No dead-letter entries. Webhook retries are healthy.</div>";
            return;
        }
        list.innerHTML = rows.map(function(r) {
            var err = escapeHtml((r.last_error || "").substring(0, 300));
            var name = escapeHtml(r.automation_name || "(unnamed)");
            var when = new Date(r.updated_at).toLocaleString();
            return "<div class=\"border rounded p-3 mb-2\">" +
                "<div class=\"d-flex justify-content-between align-items-start gap-2\">" +
                    "<div style=\"flex:1;min-width:0\">" +
                        "<strong>" + name + "</strong> " +
                        "<span class=\"badge bg-danger ms-1\">dead</span> " +
                        "<span class=\"badge bg-secondary ms-1\">" + escapeHtml(r.action_type) + "</span>" +
                        "<div class=\"small text-muted\">" + r.attempt + "/" + r.max_attempts + " attempts &middot; " + when + "</div>" +
                        "<div class=\"small text-danger mt-1\" style=\"word-break:break-all\">" + err + "</div>" +
                    "</div>" +
                    "<button class=\"btn btn-sm btn-outline-primary\" onclick=\"requeue(''" + r.id + "'')\">Requeue</button>" +
                "</div>" +
            "</div>";
        }).join("");
        if (typeof lucide !== "undefined") lucide.createIcons();
    } catch(err) {
        list.innerHTML = "<div class=\"text-danger small\">Failed to load dead letters: " + escapeHtml(err.message) + "</div>";
    }
}

async function loadFailures() {
    var list = document.getElementById("failuresList");
    try {
        var resp = await fetch("/api/v1/automations/failures");
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        var rows = await resp.json();
        if (rows.length === 0) {
            list.innerHTML = "<div class=\"text-muted small py-2\">No failed runs — nice.</div>";
            return;
        }
        list.innerHTML = rows.map(function(r) {
            var err = escapeHtml((r.error || "").substring(0, 300));
            var name = escapeHtml(r.automation_name || "(unnamed)");
            var when = new Date(r.created_at).toLocaleString();
            var dur = r.duration_ms != null ? r.duration_ms + "ms &middot; " : "";
            return "<div class=\"border rounded p-2 mb-2\">" +
                "<strong>" + name + "</strong> " +
                "<span class=\"small text-muted\">" + dur + when + "</span>" +
                "<div class=\"small text-danger\" style=\"word-break:break-all\">" + err + "</div>" +
            "</div>";
        }).join("");
    } catch(err) {
        list.innerHTML = "<div class=\"text-danger small\">Failed to load failures: " + escapeHtml(err.message) + "</div>";
    }
}

async function requeue(id) {
    try {
        var resp = await fetch("/api/v1/automations/dead-letters/" + id + "/requeue", { method: "POST" });
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        await loadAll();
    } catch(err) {
        alert("Requeue failed: " + err.message);
    }
}

function escapeHtml(s) {
    if (!s) return "";
    var d = document.createElement("div");
    d.textContent = s;
    return d.innerHTML;
}

function loadAll() {
    loadDeadLetters();
    loadFailures();
}

document.addEventListener("DOMContentLoaded", loadAll);',

    '.card-header { background-color: rgba(0,0,0,0.03); }
.badge.bg-danger { background-color: #dc3545 !important; color: #fff; }
.badge.bg-secondary { background-color: #6c757d !important; color: #fff; }'
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
