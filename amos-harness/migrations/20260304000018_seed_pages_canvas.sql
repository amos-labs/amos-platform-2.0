-- Seed the Landing Pages system canvas
-- Provides a management UI for sites and pages at nav position 5

INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-pages',
    'Pages',
    'Manage landing pages and websites',
    'custom',
    true,
    'globe',
    5,
    -- HTML
    '<div class="container-fluid p-4">
    <!-- Sites list view -->
    <div id="sites-view">
        <div class="d-flex justify-content-between align-items-center mb-4">
            <h2>Landing Pages</h2>
            <button class="btn btn-primary" onclick="showCreateSite()">
                <i data-lucide="plus" style="width:16px;height:16px;display:inline-block;vertical-align:middle;margin-right:4px"></i> New Site
            </button>
        </div>
        <div id="sites-grid" class="row g-3">
            <div class="text-center text-muted py-5">Loading sites...</div>
        </div>
    </div>

    <!-- Site detail view (hidden initially) -->
    <div id="site-detail-view" style="display:none">
        <div class="d-flex align-items-center mb-3">
            <button class="btn btn-outline-secondary btn-sm me-3" onclick="backToSites()">
                <i data-lucide="arrow-left" style="width:14px;height:14px;display:inline-block;vertical-align:middle"></i> Back
            </button>
            <h2 class="mb-0" id="site-title">Site</h2>
            <span id="site-badge" class="badge ms-2">draft</span>
        </div>
        <div class="row mb-3">
            <div class="col-md-8">
                <p class="text-muted mb-1" id="site-description"></p>
                <small class="text-muted" id="site-url"></small>
            </div>
            <div class="col-md-4 text-end">
                <button class="btn btn-outline-success btn-sm me-1" id="btn-publish" onclick="togglePublish()">Publish</button>
                <button class="btn btn-outline-primary btn-sm me-1" onclick="editSiteSettings()">
                    <i data-lucide="settings" style="width:14px;height:14px;display:inline-block;vertical-align:middle"></i> Settings
                </button>
                <button class="btn btn-outline-danger btn-sm" onclick="deleteSite()">
                    <i data-lucide="trash-2" style="width:14px;height:14px;display:inline-block;vertical-align:middle"></i>
                </button>
            </div>
        </div>
        <hr>
        <div class="d-flex justify-content-between align-items-center mb-3">
            <h5 class="mb-0">Pages</h5>
            <button class="btn btn-sm btn-primary" onclick="showAddPage()">
                <i data-lucide="plus" style="width:14px;height:14px;display:inline-block;vertical-align:middle;margin-right:2px"></i> Add Page
            </button>
        </div>
        <div id="pages-list">
            <div class="text-muted py-3">Loading pages...</div>
        </div>
    </div>

    <!-- Create/Edit Site Modal -->
    <div id="site-modal" class="modal-overlay" style="display:none">
        <div class="modal-box">
            <h4 id="site-modal-title">New Site</h4>
            <div class="mb-3">
                <label class="form-label small">Name</label>
                <input type="text" class="form-control form-control-sm" id="site-name-input" placeholder="My Landing Page">
            </div>
            <div class="mb-3">
                <label class="form-label small">Slug (URL path)</label>
                <input type="text" class="form-control form-control-sm" id="site-slug-input" placeholder="my-landing-page">
            </div>
            <div class="mb-3">
                <label class="form-label small">Description</label>
                <textarea class="form-control form-control-sm" id="site-desc-input" rows="2"></textarea>
            </div>
            <div class="d-flex justify-content-end gap-2">
                <button class="btn btn-sm btn-outline-secondary" onclick="closeSiteModal()">Cancel</button>
                <button class="btn btn-sm btn-primary" onclick="saveSite()">Save</button>
            </div>
        </div>
    </div>

    <!-- Add/Edit Page Modal -->
    <div id="page-modal" class="modal-overlay" style="display:none">
        <div class="modal-box modal-box-lg">
            <h4 id="page-modal-title">Add Page</h4>
            <div class="row mb-2">
                <div class="col-md-6 mb-2">
                    <label class="form-label small">Path</label>
                    <input type="text" class="form-control form-control-sm" id="page-path-input" placeholder="/" value="/">
                </div>
                <div class="col-md-6 mb-2">
                    <label class="form-label small">Title</label>
                    <input type="text" class="form-control form-control-sm" id="page-title-input" placeholder="Home">
                </div>
            </div>
            <div class="mb-2">
                <label class="form-label small">HTML Content</label>
                <textarea class="form-control form-control-sm font-monospace" id="page-html-input" rows="8" placeholder="&lt;h1&gt;Hello World&lt;/h1&gt;"></textarea>
            </div>
            <div class="row mb-2">
                <div class="col-md-6 mb-2">
                    <label class="form-label small">CSS (optional)</label>
                    <textarea class="form-control form-control-sm font-monospace" id="page-css-input" rows="4"></textarea>
                </div>
                <div class="col-md-6 mb-2">
                    <label class="form-label small">JS (optional)</label>
                    <textarea class="form-control form-control-sm font-monospace" id="page-js-input" rows="4"></textarea>
                </div>
            </div>
            <div class="row mb-2">
                <div class="col-md-4 mb-2">
                    <label class="form-label small">Meta Title (SEO)</label>
                    <input type="text" class="form-control form-control-sm" id="page-meta-title-input">
                </div>
                <div class="col-md-4 mb-2">
                    <label class="form-label small">Meta Description (SEO)</label>
                    <input type="text" class="form-control form-control-sm" id="page-meta-desc-input">
                </div>
                <div class="col-md-4 mb-2">
                    <label class="form-label small">Form Collection</label>
                    <input type="text" class="form-control form-control-sm" id="page-form-input" placeholder="e.g. leads">
                </div>
            </div>
            <div class="d-flex justify-content-end gap-2">
                <button class="btn btn-sm btn-outline-secondary" onclick="closePageModal()">Cancel</button>
                <button class="btn btn-sm btn-primary" onclick="savePage()">Save Page</button>
            </div>
        </div>
    </div>

    <!-- Settings Modal -->
    <div id="settings-modal" class="modal-overlay" style="display:none">
        <div class="modal-box">
            <h4>Site Settings</h4>
            <div class="mb-3">
                <label class="form-label small">Site Name</label>
                <input type="text" class="form-control form-control-sm" id="settings-name-input">
            </div>
            <div class="mb-3">
                <label class="form-label small">Description</label>
                <textarea class="form-control form-control-sm" id="settings-desc-input" rows="2"></textarea>
            </div>
            <div class="mb-3">
                <label class="form-label small">Custom Domain</label>
                <input type="text" class="form-control form-control-sm" id="settings-domain-input" placeholder="www.example.com">
            </div>
            <div class="mb-3">
                <label class="form-label small">Analytics ID</label>
                <input type="text" class="form-control form-control-sm" id="settings-analytics-input" placeholder="G-XXXXXXXXXX">
            </div>
            <div class="mb-3">
                <label class="form-label small">Theme Color</label>
                <input type="color" class="form-control form-control-sm form-control-color" id="settings-color-input" value="#000000">
            </div>
            <div class="d-flex justify-content-end gap-2">
                <button class="btn btn-sm btn-outline-secondary" onclick="closeSettingsModal()">Cancel</button>
                <button class="btn btn-sm btn-primary" onclick="saveSettings()">Save Settings</button>
            </div>
        </div>
    </div>
</div>',
    -- JS
    'var currentSite = null;
var editingPage = null;

document.addEventListener("DOMContentLoaded", function() { loadSites(); });

async function loadSites() {
    try {
        var resp = await fetch("/api/v1/sites");
        var data = await resp.json();
        var sites = data.sites || [];
        var grid = document.getElementById("sites-grid");
        if (sites.length === 0) {
            grid.innerHTML = "<div class=\"col-12 text-center text-muted py-5\"><p>No sites yet.</p><p>Create one here or ask AMOS to build a landing page for you.</p></div>";
            if (typeof lucide !== "undefined") lucide.createIcons();
            return;
        }
        grid.innerHTML = sites.map(function(s) {
            var statusBadge = s.is_published
                ? "<span class=\"badge bg-success\">published</span>"
                : "<span class=\"badge bg-secondary\">draft</span>";
            var pageCount = "—";
            var desc = s.description || "No description";
            return "<div class=\"col-md-6 col-lg-4\"><div class=\"card h-100 site-card\" style=\"cursor:pointer\" onclick=\"openSite(''" + s.slug + "'')\"><div class=\"card-body\"><div class=\"d-flex justify-content-between mb-2\"><h5 class=\"card-title mb-0\">" + escH(s.name) + "</h5>" + statusBadge + "</div><p class=\"card-text text-muted small\">" + escH(desc) + "</p><div class=\"small text-muted\"><code>/s/" + escH(s.slug) + "</code></div></div><div class=\"card-footer bg-transparent text-muted small d-flex justify-content-between\"><span>Updated " + new Date(s.updated_at).toLocaleDateString() + "</span></div></div></div>";
        }).join("");
        if (typeof lucide !== "undefined") lucide.createIcons();
    } catch(err) { console.error("Failed to load sites:", err); }
}

function escH(s) { var d = document.createElement("div"); d.textContent = s; return d.innerHTML; }

async function openSite(slug) {
    try {
        var resp = await fetch("/api/v1/sites/" + encodeURIComponent(slug));
        if (!resp.ok) throw new Error("Not found");
        var data = await resp.json();
        currentSite = data.site;
        var pages = data.pages || [];
        document.getElementById("sites-view").style.display = "none";
        document.getElementById("site-detail-view").style.display = "block";
        document.getElementById("site-title").textContent = currentSite.name;
        var badge = document.getElementById("site-badge");
        if (currentSite.is_published) {
            badge.textContent = "published";
            badge.className = "badge ms-2 bg-success";
            document.getElementById("btn-publish").textContent = "Unpublish";
            document.getElementById("btn-publish").className = "btn btn-outline-warning btn-sm me-1";
        } else {
            badge.textContent = "draft";
            badge.className = "badge ms-2 bg-secondary";
            document.getElementById("btn-publish").textContent = "Publish";
            document.getElementById("btn-publish").className = "btn btn-outline-success btn-sm me-1";
        }
        document.getElementById("site-description").textContent = currentSite.description || "";
        var origin = window.location.origin;
        document.getElementById("site-url").innerHTML = "URL: <a href=\"" + origin + "/s/" + escH(currentSite.slug) + "\" target=\"_blank\">" + origin + "/s/" + escH(currentSite.slug) + "</a>";
        renderPages(pages);
        if (typeof lucide !== "undefined") lucide.createIcons();
    } catch(err) { console.error("Failed to open site:", err); alert("Failed to load site"); }
}

function renderPages(pages) {
    var list = document.getElementById("pages-list");
    if (!pages || pages.length === 0) {
        list.innerHTML = "<div class=\"text-center text-muted py-3\"><p>No pages yet. Add one or ask AMOS to generate content.</p></div>";
        return;
    }
    list.innerHTML = "<div class=\"list-group\">" + pages.map(function(p) {
        var pubIcon = p.is_published ? "<span class=\"badge bg-success\">live</span>" : "<span class=\"badge bg-secondary\">draft</span>";
        return "<div class=\"list-group-item d-flex justify-content-between align-items-center page-item\"><div><strong>" + escH(p.path) + "</strong> <span class=\"text-muted\">— " + escH(p.title) + "</span> " + pubIcon + "</div><div><button class=\"btn btn-outline-primary btn-sm me-1\" onclick=\"editPage(''" + p.id + "'', event)\">Edit</button><button class=\"btn btn-outline-danger btn-sm\" onclick=\"deletePage(''" + p.id + "'', event)\"><i data-lucide=\"trash-2\" style=\"width:12px;height:12px;display:inline-block;vertical-align:middle\"></i></button></div></div>";
    }).join("") + "</div>";
    if (typeof lucide !== "undefined") lucide.createIcons();
}

function backToSites() {
    currentSite = null;
    document.getElementById("site-detail-view").style.display = "none";
    document.getElementById("sites-view").style.display = "block";
    loadSites();
}

function showCreateSite() {
    document.getElementById("site-modal-title").textContent = "New Site";
    document.getElementById("site-name-input").value = "";
    document.getElementById("site-slug-input").value = "";
    document.getElementById("site-desc-input").value = "";
    document.getElementById("site-slug-input").disabled = false;
    document.getElementById("site-modal").style.display = "flex";
}

function closeSiteModal() { document.getElementById("site-modal").style.display = "none"; }

async function saveSite() {
    var name = document.getElementById("site-name-input").value.trim();
    var slug = document.getElementById("site-slug-input").value.trim();
    if (!name || !slug) { alert("Name and slug are required"); return; }
    try {
        var resp = await fetch("/api/v1/sites", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name: name, slug: slug, description: document.getElementById("site-desc-input").value.trim() || null })
        });
        if (!resp.ok) { var err = await resp.text(); throw new Error(err); }
        closeSiteModal();
        loadSites();
    } catch(err) { alert("Failed to create site: " + err.message); }
}

async function togglePublish() {
    if (!currentSite) return;
    var newState = !currentSite.is_published;
    try {
        var resp = await fetch("/api/v1/sites/" + encodeURIComponent(currentSite.slug) + "/publish", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ publish: newState })
        });
        if (!resp.ok) throw new Error("Failed");
        openSite(currentSite.slug);
    } catch(err) { alert("Failed to update publish state"); }
}

async function deleteSite() {
    if (!currentSite) return;
    if (!confirm("Delete site \"" + currentSite.name + "\" and all its pages? This cannot be undone.")) return;
    try {
        var resp = await fetch("/api/v1/sites/" + encodeURIComponent(currentSite.slug), { method: "DELETE" });
        if (!resp.ok) throw new Error("Failed");
        backToSites();
    } catch(err) { alert("Failed to delete site"); }
}

function editSiteSettings() {
    if (!currentSite) return;
    document.getElementById("settings-name-input").value = currentSite.name || "";
    document.getElementById("settings-desc-input").value = currentSite.description || "";
    document.getElementById("settings-domain-input").value = currentSite.domain || "";
    var settings = currentSite.settings || {};
    document.getElementById("settings-analytics-input").value = settings.analytics_id || "";
    document.getElementById("settings-color-input").value = settings.theme_color || "#000000";
    document.getElementById("settings-modal").style.display = "flex";
}

function closeSettingsModal() { document.getElementById("settings-modal").style.display = "none"; }

async function saveSettings() {
    if (!currentSite) return;
    var payload = {
        name: document.getElementById("settings-name-input").value.trim() || null,
        description: document.getElementById("settings-desc-input").value.trim() || null,
        domain: document.getElementById("settings-domain-input").value.trim() || null,
        settings: {
            analytics_id: document.getElementById("settings-analytics-input").value.trim() || null,
            theme_color: document.getElementById("settings-color-input").value || "#000000"
        }
    };
    try {
        var resp = await fetch("/api/v1/sites/" + encodeURIComponent(currentSite.slug), {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(payload)
        });
        if (!resp.ok) throw new Error("Failed");
        closeSettingsModal();
        openSite(currentSite.slug);
    } catch(err) { alert("Failed to save settings: " + err.message); }
}

function showAddPage() {
    editingPage = null;
    document.getElementById("page-modal-title").textContent = "Add Page";
    document.getElementById("page-path-input").value = "/";
    document.getElementById("page-title-input").value = "";
    document.getElementById("page-html-input").value = "";
    document.getElementById("page-css-input").value = "";
    document.getElementById("page-js-input").value = "";
    document.getElementById("page-meta-title-input").value = "";
    document.getElementById("page-meta-desc-input").value = "";
    document.getElementById("page-form-input").value = "";
    document.getElementById("page-modal").style.display = "flex";
}

function closePageModal() { document.getElementById("page-modal").style.display = "none"; }

async function editPage(pageId, evt) {
    if (evt) evt.stopPropagation();
    if (!currentSite) return;
    try {
        var resp = await fetch("/api/v1/sites/" + encodeURIComponent(currentSite.slug) + "/pages/" + pageId);
        if (!resp.ok) throw new Error("Not found");
        var page = await resp.json();
        editingPage = page;
        document.getElementById("page-modal-title").textContent = "Edit Page";
        document.getElementById("page-path-input").value = page.path || "/";
        document.getElementById("page-title-input").value = page.title || "";
        document.getElementById("page-html-input").value = page.html_content || "";
        document.getElementById("page-css-input").value = page.css_content || "";
        document.getElementById("page-js-input").value = page.js_content || "";
        document.getElementById("page-meta-title-input").value = page.meta_title || "";
        document.getElementById("page-meta-desc-input").value = page.meta_description || "";
        document.getElementById("page-form-input").value = page.form_collection || "";
        document.getElementById("page-modal").style.display = "flex";
    } catch(err) { alert("Failed to load page: " + err.message); }
}

async function savePage() {
    if (!currentSite) return;
    var path = document.getElementById("page-path-input").value.trim();
    var title = document.getElementById("page-title-input").value.trim();
    var html = document.getElementById("page-html-input").value;
    if (!path || !title || !html) { alert("Path, title, and HTML content are required"); return; }
    var payload = {
        path: path,
        title: title,
        html_content: html,
        css_content: document.getElementById("page-css-input").value || null,
        js_content: document.getElementById("page-js-input").value || null,
        meta_title: document.getElementById("page-meta-title-input").value || null,
        meta_description: document.getElementById("page-meta-desc-input").value || null,
        form_collection: document.getElementById("page-form-input").value || null
    };
    try {
        var url = "/api/v1/sites/" + encodeURIComponent(currentSite.slug) + "/pages";
        if (editingPage) {
            url = "/api/v1/sites/" + encodeURIComponent(currentSite.slug) + "/pages/" + editingPage.id;
        }
        var resp = await fetch(url, {
            method: editingPage ? "PUT" : "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(payload)
        });
        if (!resp.ok) { var err = await resp.text(); throw new Error(err); }
        closePageModal();
        openSite(currentSite.slug);
    } catch(err) { alert("Failed to save page: " + err.message); }
}

async function deletePage(pageId, evt) {
    if (evt) evt.stopPropagation();
    if (!currentSite) return;
    if (!confirm("Delete this page? This cannot be undone.")) return;
    try {
        var resp = await fetch("/api/v1/sites/" + encodeURIComponent(currentSite.slug) + "/pages/" + pageId, { method: "DELETE" });
        if (!resp.ok) throw new Error("Failed");
        openSite(currentSite.slug);
    } catch(err) { alert("Failed to delete page"); }
}

// Auto-generate slug from name
document.addEventListener("DOMContentLoaded", function() {
    var nameInput = document.getElementById("site-name-input");
    var slugInput = document.getElementById("site-slug-input");
    if (nameInput && slugInput) {
        nameInput.addEventListener("input", function() {
            if (!slugInput.disabled) {
                slugInput.value = nameInput.value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
            }
        });
    }
});',
    -- CSS
    '.site-card { transition: transform 0.15s, box-shadow 0.15s; }
.site-card:hover { transform: translateY(-2px); box-shadow: 0 4px 12px rgba(0,0,0,0.1); }
.page-item { transition: background 0.1s; }
.page-item:hover { background: rgba(99, 102, 241, 0.04); }
.modal-overlay { position: fixed; top: 0; left: 0; width: 100%; height: 100%; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.modal-box { background: var(--bs-body-bg, #fff); border-radius: 8px; padding: 24px; width: 480px; max-width: 95%; max-height: 90vh; overflow-y: auto; box-shadow: 0 8px 32px rgba(0,0,0,0.3); }
.modal-box-lg { width: 720px; }
.font-monospace { font-family: "SF Mono", "Cascadia Code", "Fira Code", monospace; font-size: 0.85em; }'
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    is_system = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();
