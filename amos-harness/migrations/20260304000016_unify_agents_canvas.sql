-- Unify agent terminology: remove "Bots" system canvas, update "External Agents" to "Agents"
-- Agents now covers both internal (background tasks) and external (OpenClaw) agents.

-- Remove the old Bots system canvas
DELETE FROM canvases WHERE slug = 'system-bots';

-- Update the Agents system canvas: rename, new content, move to nav_order 2 (where Bots was)
UPDATE canvases
SET
    name = 'Agents',
    description = 'View internal and external agents',
    nav_icon = 'bot',
    nav_order = 2,
    html_content = '<div class="container-fluid p-4">
    <div class="d-flex justify-content-between align-items-center mb-4">
        <h2>Agents</h2>
    </div>

    <!-- Internal Agents (Background Tasks) -->
    <h5 class="text-muted mb-3">Internal Agents (Background Tasks)</h5>
    <div id="tasks-list" class="row g-3 mb-4">
        <div class="text-center text-muted py-3">Loading...</div>
    </div>

    <!-- External Agents (OpenClaw) -->
    <h5 class="text-muted mb-3">External Agents (OpenClaw)</h5>
    <div id="agents-list" class="row g-3">
        <div class="text-center text-muted py-3">Loading...</div>
    </div>
</div>',
    js_content = 'document.addEventListener("DOMContentLoaded", function() {
    loadTasks();
    loadExternalAgents();
});

async function loadTasks() {
    var list = document.getElementById("tasks-list");
    try {
        var resp = await fetch("/api/v1/tasks");
        if (!resp.ok) throw new Error("Failed to fetch");
        var tasks = await resp.json();
        if (!tasks || tasks.length === 0) {
            list.innerHTML = "<div class=\"col-12 text-center text-muted py-3\"><p class=\"mb-0\">No background tasks running.</p></div>";
            return;
        }
        list.innerHTML = tasks.map(function(t) {
            var statusColors = {"pending":"warning","running":"primary","completed":"success","failed":"danger","cancelled":"secondary"};
            var statusClass = statusColors[t.status] || "secondary";
            return "<div class=\"col-md-6 col-lg-4\"><div class=\"card h-100\"><div class=\"card-body\"><div class=\"d-flex justify-content-between mb-2\"><h6 class=\"card-title mb-0\">" + (t.description || t.task_type || "Task") + "</h6><span class=\"badge bg-" + statusClass + "\">" + (t.status || "unknown") + "</span></div><p class=\"card-text text-muted small mb-0\">" + (t.task_type === "bounty" ? "External bounty" : "Internal task") + "</p></div></div></div>";
        }).join("");
        if (typeof lucide !== "undefined") lucide.createIcons();
    } catch(err) {
        list.innerHTML = "<div class=\"col-12 text-center text-muted py-3\"><p class=\"mb-0\">No background tasks running.</p></div>";
    }
}

async function loadExternalAgents() {
    var list = document.getElementById("agents-list");
    try {
        var resp = await fetch("/api/v1/agents");
        if (!resp.ok) throw new Error("Failed to fetch");
        var agents = await resp.json();
        if (!agents || agents.length === 0) {
            list.innerHTML = "<div class=\"col-12 text-center text-muted py-3\"><p class=\"mb-0\">No external agents registered.</p><p class=\"small\">External agents connect via the OpenClaw protocol.</p></div>";
            return;
        }
        list.innerHTML = agents.map(function(a) {
            var statusClass = a.status === "active" ? "success" : "secondary";
            return "<div class=\"col-md-6 col-lg-4\"><div class=\"card h-100\"><div class=\"card-body\"><div class=\"d-flex justify-content-between mb-2\"><h6 class=\"card-title mb-0\">" + (a.name || "Agent") + "</h6><span class=\"badge bg-" + statusClass + "\">" + (a.status || "inactive") + "</span></div><p class=\"card-text text-muted small mb-0\">" + (a.description || "No description") + "</p></div></div></div>";
        }).join("");
        if (typeof lucide !== "undefined") lucide.createIcons();
    } catch(err) {
        list.innerHTML = "<div class=\"col-12 text-center text-muted py-3\"><p class=\"mb-0\">No external agents registered.</p></div>";
    }
}',
    css_content = '',
    updated_at = NOW()
WHERE slug = 'system-agents';
