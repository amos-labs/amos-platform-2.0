-- Bounty Management Canvas
-- System canvas for creating, viewing, and managing bounties on the AMOS Network.
-- Talks to the relay via the harness bounty proxy at /api/v1/bounties.

INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-bounties',
    'Bounties',
    'Create and manage bounties on the AMOS Network',
    'custom',
    true,
    'trophy',
    3,
    -- HTML
    '<div class="container-fluid p-4">
    <!-- Header -->
    <div class="d-flex justify-content-between align-items-center mb-4">
        <div>
            <h2 class="mb-0">Bounties</h2>
            <small class="text-muted">AMOS Network Marketplace</small>
        </div>
        <div class="d-flex gap-2">
            <select id="status-filter" class="form-select form-select-sm" style="width:auto" onchange="loadBounties()">
                <option value="">All Statuses</option>
                <option value="open">Open</option>
                <option value="claimed">Claimed</option>
                <option value="submitted">Submitted</option>
                <option value="approved">Approved</option>
                <option value="rejected">Rejected</option>
            </select>
            <button class="btn btn-primary btn-sm" onclick="showCreateModal()">
                <i data-lucide="plus" style="width:16px;height:16px;display:inline-block;vertical-align:middle;margin-right:4px"></i> New Bounty
            </button>
        </div>
    </div>

    <!-- Stats Row -->
    <div class="row g-3 mb-4" id="stats-row">
        <div class="col-6 col-md-3">
            <div class="card text-center">
                <div class="card-body py-2">
                    <div class="text-muted small">Open</div>
                    <div class="fs-4 fw-bold text-primary" id="stat-open">-</div>
                </div>
            </div>
        </div>
        <div class="col-6 col-md-3">
            <div class="card text-center">
                <div class="card-body py-2">
                    <div class="text-muted small">In Progress</div>
                    <div class="fs-4 fw-bold text-warning" id="stat-progress">-</div>
                </div>
            </div>
        </div>
        <div class="col-6 col-md-3">
            <div class="card text-center">
                <div class="card-body py-2">
                    <div class="text-muted small">Completed</div>
                    <div class="fs-4 fw-bold text-success" id="stat-completed">-</div>
                </div>
            </div>
        </div>
        <div class="col-6 col-md-3">
            <div class="card text-center">
                <div class="card-body py-2">
                    <div class="text-muted small">Total Rewards</div>
                    <div class="fs-4 fw-bold text-info" id="stat-rewards">-</div>
                </div>
            </div>
        </div>
    </div>

    <!-- Bounty List -->
    <div id="bounty-list">
        <div class="text-center text-muted py-5">Loading bounties...</div>
    </div>

    <!-- Detail Panel (hidden by default) -->
    <div id="detail-panel" class="offcanvas offcanvas-end" tabindex="-1" style="width:500px">
        <div class="offcanvas-header">
            <h5 class="offcanvas-title" id="detail-title">Bounty Details</h5>
            <button type="button" class="btn-close" data-bs-dismiss="offcanvas"></button>
        </div>
        <div class="offcanvas-body" id="detail-body">
        </div>
    </div>
</div>

<!-- Create Bounty Modal -->
<div class="modal fade" id="create-modal" tabindex="-1">
    <div class="modal-dialog">
        <div class="modal-content">
            <div class="modal-header">
                <h5 class="modal-title">Create Bounty</h5>
                <button type="button" class="btn-close" data-bs-dismiss="modal"></button>
            </div>
            <div class="modal-body">
                <div class="mb-3">
                    <label class="form-label">Title</label>
                    <input type="text" class="form-control" id="bounty-title" placeholder="e.g. Analyze Q1 customer churn data">
                </div>
                <div class="mb-3">
                    <label class="form-label">Description</label>
                    <textarea class="form-control" id="bounty-description" rows="4" placeholder="Detailed description of what needs to be done..."></textarea>
                </div>
                <div class="row g-3">
                    <div class="col-6">
                        <label class="form-label">Reward (points)</label>
                        <input type="number" class="form-control" id="bounty-reward" value="100" min="1" max="2000">
                    </div>
                    <div class="col-6">
                        <label class="form-label">Deadline</label>
                        <input type="datetime-local" class="form-control" id="bounty-deadline">
                    </div>
                </div>
                <div class="mb-3 mt-3">
                    <label class="form-label">Required Capabilities</label>
                    <input type="text" class="form-control" id="bounty-caps" placeholder="web_search, document_processing (comma-separated)">
                </div>
                <div class="mb-3">
                    <label class="form-label">Poster Wallet</label>
                    <input type="text" class="form-control" id="bounty-wallet" placeholder="Solana wallet address">
                </div>
            </div>
            <div class="modal-footer">
                <button type="button" class="btn btn-secondary" data-bs-dismiss="modal">Cancel</button>
                <button type="button" class="btn btn-primary" onclick="createBounty()">Create Bounty</button>
            </div>
        </div>
    </div>
</div>',
    -- JS
    'var allBounties = [];

document.addEventListener("DOMContentLoaded", function() {
    loadBounties();
    // Set default deadline to 7 days from now
    var d = new Date();
    d.setDate(d.getDate() + 7);
    var el = document.getElementById("bounty-deadline");
    if (el) el.value = d.toISOString().slice(0, 16);
});

async function loadBounties() {
    try {
        var filter = document.getElementById("status-filter").value;
        var url = "/api/v1/bounties";
        if (filter) url += "?status=" + filter;
        var resp = await fetch(url);
        if (!resp.ok) {
            if (resp.status === 502) {
                document.getElementById("bounty-list").innerHTML = "<div class=\"text-center text-muted py-5\"><i data-lucide=\"wifi-off\" style=\"width:48px;height:48px;display:block;margin:0 auto 12px\"></i><p>Cannot reach AMOS Network Relay.</p><p class=\"small\">The relay service may not be running. Bounties require a connected relay.</p></div>";
                updateIcons();
                return;
            }
            throw new Error("HTTP " + resp.status);
        }
        allBounties = await resp.json();
        if (!Array.isArray(allBounties)) allBounties = [];
        renderBounties(allBounties);
        updateStats(allBounties);
    } catch(err) {
        console.error("Failed to load bounties:", err);
        document.getElementById("bounty-list").innerHTML = "<div class=\"text-center text-muted py-5\"><p>Failed to load bounties.</p><p class=\"small\">" + err.message + "</p></div>";
    }
}

function updateStats(bounties) {
    var open = bounties.filter(function(b) { return b.status === "open"; }).length;
    var progress = bounties.filter(function(b) { return b.status === "claimed" || b.status === "submitted"; }).length;
    var completed = bounties.filter(function(b) { return b.status === "approved"; }).length;
    var totalReward = bounties.reduce(function(sum, b) { return sum + (b.reward_tokens || 0); }, 0);
    document.getElementById("stat-open").textContent = open;
    document.getElementById("stat-progress").textContent = progress;
    document.getElementById("stat-completed").textContent = completed;
    document.getElementById("stat-rewards").textContent = totalReward.toLocaleString();
}

function renderBounties(bounties) {
    var list = document.getElementById("bounty-list");
    if (bounties.length === 0) {
        list.innerHTML = "<div class=\"text-center text-muted py-5\"><i data-lucide=\"trophy\" style=\"width:48px;height:48px;display:block;margin:0 auto 12px\"></i><p>No bounties found.</p><p class=\"small\">Create your first bounty to get started.</p></div>";
        updateIcons();
        return;
    }
    list.innerHTML = bounties.map(function(b) {
        var statusColors = {open:"primary",claimed:"warning",submitted:"info",approved:"success",rejected:"danger",expired:"secondary",cancelled:"secondary"};
        var sc = statusColors[b.status] || "secondary";
        var deadline = b.deadline ? new Date(b.deadline).toLocaleDateString() : "No deadline";
        var caps = (b.required_capabilities || []).map(function(c) { return "<span class=\"badge bg-light text-dark me-1\">" + c + "</span>"; }).join("");
        return "<div class=\"card mb-2 bounty-card\" style=\"cursor:pointer\" onclick=\"showDetail(''" + b.id + "'')\"><div class=\"card-body py-3\"><div class=\"d-flex justify-content-between align-items-start\"><div class=\"flex-grow-1\"><div class=\"d-flex align-items-center gap-2 mb-1\"><h6 class=\"mb-0\">" + b.title + "</h6><span class=\"badge bg-" + sc + "\">" + b.status + "</span></div><p class=\"text-muted small mb-1 text-truncate\" style=\"max-width:500px\">" + (b.description || "") + "</p><div>" + caps + "</div></div><div class=\"text-end ms-3\"><div class=\"fw-bold text-primary\">" + (b.reward_tokens || 0).toLocaleString() + " pts</div><div class=\"text-muted small\">" + deadline + "</div></div></div></div></div>";
    }).join("");
    updateIcons();
}

function showCreateModal() {
    var modal = new bootstrap.Modal(document.getElementById("create-modal"));
    modal.show();
}

async function createBounty() {
    var title = document.getElementById("bounty-title").value.trim();
    var description = document.getElementById("bounty-description").value.trim();
    var reward = parseInt(document.getElementById("bounty-reward").value) || 100;
    var deadline = document.getElementById("bounty-deadline").value;
    var caps = document.getElementById("bounty-caps").value.split(",").map(function(s) { return s.trim(); }).filter(Boolean);
    var wallet = document.getElementById("bounty-wallet").value.trim();

    if (!title) { alert("Title is required"); return; }
    if (!description) { alert("Description is required"); return; }
    if (!wallet) { alert("Poster wallet is required"); return; }
    if (!deadline) { alert("Deadline is required"); return; }

    try {
        var resp = await fetch("/api/v1/bounties", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                title: title,
                description: description,
                reward_tokens: reward,
                deadline: new Date(deadline).toISOString(),
                required_capabilities: caps,
                poster_wallet: wallet
            })
        });
        if (!resp.ok) {
            var err = await resp.text();
            alert("Failed to create bounty: " + err);
            return;
        }
        // Close modal and reload
        bootstrap.Modal.getInstance(document.getElementById("create-modal")).hide();
        document.getElementById("bounty-title").value = "";
        document.getElementById("bounty-description").value = "";
        document.getElementById("bounty-reward").value = "100";
        document.getElementById("bounty-caps").value = "";
        document.getElementById("bounty-wallet").value = "";
        loadBounties();
    } catch(err) {
        alert("Error: " + err.message);
    }
}

async function showDetail(id) {
    var bounty = allBounties.find(function(b) { return b.id === id; });
    if (!bounty) return;

    var statusColors = {open:"primary",claimed:"warning",submitted:"info",approved:"success",rejected:"danger",expired:"secondary",cancelled:"secondary"};
    var sc = statusColors[bounty.status] || "secondary";

    var html = "<div class=\"mb-3\"><span class=\"badge bg-" + sc + " mb-2\">" + bounty.status + "</span></div>";
    html += "<div class=\"mb-3\"><strong>Description</strong><p class=\"mt-1\">" + (bounty.description || "No description") + "</p></div>";
    html += "<div class=\"row g-3 mb-3\"><div class=\"col-6\"><strong>Reward</strong><div class=\"fs-5 text-primary\">" + (bounty.reward_tokens || 0).toLocaleString() + " pts</div></div><div class=\"col-6\"><strong>Deadline</strong><div>" + (bounty.deadline ? new Date(bounty.deadline).toLocaleString() : "None") + "</div></div></div>";

    if (bounty.required_capabilities && bounty.required_capabilities.length > 0) {
        html += "<div class=\"mb-3\"><strong>Required Capabilities</strong><div class=\"mt-1\">" + bounty.required_capabilities.map(function(c) { return "<span class=\"badge bg-light text-dark me-1\">" + c + "</span>"; }).join("") + "</div></div>";
    }

    html += "<div class=\"mb-3\"><strong>Poster Wallet</strong><div class=\"text-muted small font-monospace\">" + (bounty.poster_wallet || "Unknown") + "</div></div>";

    if (bounty.claimed_by_agent_id) {
        html += "<div class=\"mb-3\"><strong>Claimed By</strong><div class=\"text-muted small\">" + bounty.claimed_by_agent_id + "</div></div>";
    }
    if (bounty.quality_score !== null && bounty.quality_score !== undefined) {
        html += "<div class=\"mb-3\"><strong>Quality Score</strong><div>" + bounty.quality_score + " / 100</div></div>";
    }
    if (bounty.settlement_tx) {
        html += "<div class=\"mb-3\"><strong>Settlement TX</strong><div class=\"text-muted small font-monospace\">" + bounty.settlement_tx + "</div></div>";
    }
    if (bounty.rejection_reason) {
        html += "<div class=\"mb-3\"><strong>Rejection Reason</strong><div class=\"text-danger\">" + bounty.rejection_reason + "</div></div>";
    }

    // Action buttons based on status
    html += "<hr>";
    if (bounty.status === "submitted") {
        html += "<div class=\"d-flex gap-2\"><button class=\"btn btn-success flex-grow-1\" onclick=\"approveBounty(''" + id + "'')\"><i data-lucide=\"check\" style=\"width:16px;height:16px;display:inline-block;vertical-align:middle;margin-right:4px\"></i> Approve</button><button class=\"btn btn-danger flex-grow-1\" onclick=\"rejectBounty(''" + id + "'')\"><i data-lucide=\"x\" style=\"width:16px;height:16px;display:inline-block;vertical-align:middle;margin-right:4px\"></i> Reject</button></div>";
    }

    html += "<div class=\"mt-3\"><button class=\"btn btn-outline-secondary btn-sm w-100\" onclick=\"askAgentAbout(''" + id + "'')\"><i data-lucide=\"message-square\" style=\"width:14px;height:14px;display:inline-block;vertical-align:middle;margin-right:4px\"></i> Ask AMOS Agent about this bounty</button></div>";

    document.getElementById("detail-title").textContent = bounty.title;
    document.getElementById("detail-body").innerHTML = html;
    var offcanvas = new bootstrap.Offcanvas(document.getElementById("detail-panel"));
    offcanvas.show();
    updateIcons();
}

async function approveBounty(id) {
    var wallet = prompt("Enter reviewer wallet address:");
    if (!wallet) return;
    var score = prompt("Quality score (1-100):", "80");
    if (!score) return;
    try {
        var resp = await fetch("/api/v1/bounties/" + id + "/approve", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ reviewer_wallet: wallet, quality_score: parseInt(score) })
        });
        if (!resp.ok) { alert("Failed to approve: " + (await resp.text())); return; }
        bootstrap.Offcanvas.getInstance(document.getElementById("detail-panel")).hide();
        loadBounties();
    } catch(err) { alert("Error: " + err.message); }
}

async function rejectBounty(id) {
    var wallet = prompt("Enter reviewer wallet address:");
    if (!wallet) return;
    var reason = prompt("Rejection reason:");
    if (!reason) return;
    try {
        var resp = await fetch("/api/v1/bounties/" + id + "/reject", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ reviewer_wallet: wallet, reason: reason })
        });
        if (!resp.ok) { alert("Failed to reject: " + (await resp.text())); return; }
        bootstrap.Offcanvas.getInstance(document.getElementById("detail-panel")).hide();
        loadBounties();
    } catch(err) { alert("Error: " + err.message); }
}

function askAgentAbout(id) {
    var bounty = allBounties.find(function(b) { return b.id === id; });
    if (!bounty) return;
    window.parent.postMessage({
        type: "amos-chat",
        message: "Tell me about bounty \"" + bounty.title + "\" (ID: " + id + "). Status: " + bounty.status + ". What actions can I take?"
    }, "*");
}

function updateIcons() {
    if (typeof lucide !== "undefined") lucide.createIcons();
}',
    -- CSS
    '.bounty-card { transition: transform 0.15s, box-shadow 0.15s; border-left: 3px solid transparent; }
.bounty-card:hover { transform: translateY(-1px); box-shadow: 0 2px 8px rgba(0,0,0,0.08); border-left-color: var(--bs-primary); }
#stats-row .card { border: none; background: var(--bs-light, #f8f9fa); }'
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    is_system = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();
