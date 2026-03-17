-- Update the system-settings canvas to include BYOK LLM provider management.
-- This replaces the basic model selector with a full provider configuration UI
-- that calls /api/v1/llm-providers CRUD endpoints.

INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-settings',
    'Settings',
    'Configure AMOS settings and LLM providers',
    'custom',
    true,
    'settings',
    10,
    -- ═══════════════════════════════════════════════════════════════════
    -- HTML
    -- ═══════════════════════════════════════════════════════════════════
    '<div class="container-fluid p-4" style="max-width:800px">
    <h2 class="mb-4">Settings</h2>

    <!-- ── LLM Provider Section ── -->
    <div class="card mb-4">
        <div class="card-header d-flex justify-content-between align-items-center">
            <h5 class="mb-0"><i data-lucide="brain" style="width:18px;height:18px;display:inline-block;vertical-align:middle;margin-right:6px"></i> AI Provider</h5>
            <button class="btn btn-primary btn-sm" onclick="showAddForm()">
                <i data-lucide="plus" style="width:14px;height:14px;display:inline-block;vertical-align:middle;margin-right:2px"></i> Add Provider
            </button>
        </div>
        <div class="card-body">
            <p class="text-muted small mb-3">Configure which LLM provider AMOS uses for chat. Bring your own API key (BYOK) to use Anthropic, OpenAI, or any compatible provider. Your API key is encrypted and stored securely.</p>
            <div id="providersList">
                <div class="text-center text-muted py-3">
                    <div class="spinner-border spinner-border-sm me-2" role="status"></div>
                    Loading providers...
                </div>
            </div>
        </div>
    </div>

    <!-- ── Add/Edit Provider Form (hidden by default) ── -->
    <div class="card mb-4 d-none" id="providerForm">
        <div class="card-header d-flex justify-content-between align-items-center">
            <h5 class="mb-0" id="formTitle">Add LLM Provider</h5>
            <button class="btn btn-sm btn-outline-secondary" onclick="hideForm()">Cancel</button>
        </div>
        <div class="card-body">
            <input type="hidden" id="editProviderId" value="">

            <div class="mb-3">
                <label class="form-label fw-semibold">Provider Type</label>
                <select id="providerType" class="form-select" onchange="onProviderTypeChange()">
                    <option value="">-- Select --</option>
                    <option value="anthropic">Anthropic (Claude)</option>
                    <option value="openai">OpenAI (GPT)</option>
                    <option value="custom">Custom (OpenAI-compatible)</option>
                </select>
            </div>

            <div class="mb-3">
                <label class="form-label fw-semibold">Display Name</label>
                <input type="text" class="form-control" id="displayName" placeholder="e.g. My Anthropic Account">
            </div>

            <div class="mb-3">
                <label class="form-label fw-semibold">API Base URL</label>
                <input type="text" class="form-control" id="apiBase" placeholder="https://api.anthropic.com/v1">
                <div class="form-text">The base URL for the provider API (without trailing slash).</div>
            </div>

            <div class="mb-3">
                <label class="form-label fw-semibold">API Key</label>
                <div class="input-group">
                    <input type="password" class="form-control font-monospace" id="apiKey" placeholder="sk-..." autocomplete="off">
                    <button class="btn btn-outline-secondary" type="button" onclick="toggleApiKeyVisibility()">
                        <i data-lucide="eye" id="apiKeyVisIcon" style="width:16px;height:16px"></i>
                    </button>
                </div>
                <div class="form-text">Your API key will be encrypted and stored in the credential vault.</div>
            </div>

            <div class="mb-3">
                <label class="form-label fw-semibold">Default Model</label>
                <select id="defaultModel" class="form-select">
                    <option value="">-- Select provider type first --</option>
                </select>
            </div>

            <div id="formError" class="alert alert-danger small d-none"></div>
            <div id="formSuccess" class="alert alert-success small d-none"></div>

            <button class="btn btn-primary" id="saveBtn" onclick="saveProvider()">
                <i data-lucide="save" style="width:16px;height:16px;display:inline-block;vertical-align:middle;margin-right:4px"></i>
                <span id="saveBtnText">Save Provider</span>
            </button>
        </div>
    </div>

    <!-- ── About Section ── -->
    <div class="card mb-3">
        <div class="card-body">
            <h5 class="card-title">About</h5>
            <p class="text-muted mb-0">AMOS Harness v0.5.0</p>
        </div>
    </div>
</div>',

    -- ═══════════════════════════════════════════════════════════════════
    -- JS
    -- ═══════════════════════════════════════════════════════════════════
    'var providers = [];
var editingId = null;

var PROVIDER_DEFAULTS = {
    anthropic: {
        displayName: "Anthropic (Claude)",
        apiBase: "https://api.anthropic.com/v1",
        models: [
            "claude-sonnet-4-6",
            "claude-opus-4-6",
            "claude-haiku-4-5"
        ]
    },
    openai: {
        displayName: "OpenAI (GPT)",
        apiBase: "https://api.openai.com/v1",
        models: [
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4-turbo",
            "o1-preview",
            "o1-mini"
        ]
    },
    custom: {
        displayName: "",
        apiBase: "",
        models: []
    }
};

document.addEventListener("DOMContentLoaded", function() {
    loadProviders();
});

async function loadProviders() {
    try {
        var resp = await fetch("/api/v1/llm-providers");
        if (!resp.ok) throw new Error("Failed to load");
        providers = await resp.json();
        renderProviders();
    } catch(err) {
        document.getElementById("providersList").innerHTML =
            "<div class=\"text-center text-muted py-3\">Failed to load providers. <a href=\"#\" onclick=\"loadProviders();return false;\">Retry</a></div>";
    }
}

function renderProviders() {
    var list = document.getElementById("providersList");

    if (providers.length === 0) {
        list.innerHTML = "<div class=\"text-center py-4\"><p class=\"text-muted mb-2\">No LLM providers configured.</p><p class=\"small text-muted\">Add your Anthropic or OpenAI API key to enable AI chat.</p></div>";
        if (typeof lucide !== "undefined") lucide.createIcons();
        return;
    }

    list.innerHTML = providers.map(function(p) {
        var statusBadge = "";
        if (p.is_active) {
            statusBadge = "<span class=\"badge bg-success\">Active</span>";
        } else if (p.is_verified) {
            statusBadge = "<span class=\"badge bg-secondary\">Verified</span>";
        } else {
            statusBadge = "<span class=\"badge bg-warning text-dark\">Unverified</span>";
        }

        var providerIcon = p.name === "anthropic" ? "sparkles" : p.name === "openai" ? "zap" : "cpu";

        var errorHtml = "";
        if (p.last_error) {
            errorHtml = "<div class=\"small text-danger mt-1\" style=\"word-break:break-all;\"><i data-lucide=\"alert-circle\" style=\"width:12px;height:12px;display:inline-block;vertical-align:middle;margin-right:2px\"></i>" + escapeHtml(p.last_error).substring(0, 120) + "</div>";
        }

        var activeBtn = p.is_active
            ? ""
            : "<button class=\"btn btn-outline-success btn-sm\" onclick=\"activateProvider(''" + p.id + "'')\" title=\"Set as active\"><i data-lucide=\"check-circle\" style=\"width:14px;height:14px\"></i></button>";

        return "<div class=\"border rounded p-3 mb-2 " + (p.is_active ? "border-success bg-success bg-opacity-10" : "") + "\">" +
            "<div class=\"d-flex justify-content-between align-items-start\">" +
                "<div class=\"d-flex align-items-center gap-2\">" +
                    "<i data-lucide=\"" + providerIcon + "\" style=\"width:20px;height:20px\" class=\"text-muted\"></i>" +
                    "<div>" +
                        "<strong>" + escapeHtml(p.display_name) + "</strong> " + statusBadge +
                        "<div class=\"small text-muted\">" + escapeHtml(p.default_model) + " &middot; " + escapeHtml(p.api_base) + "</div>" +
                        errorHtml +
                    "</div>" +
                "</div>" +
                "<div class=\"d-flex gap-1\">" +
                    activeBtn +
                    "<button class=\"btn btn-outline-primary btn-sm\" onclick=\"testProvider(''" + p.id + "'')\" title=\"Test connection\"><i data-lucide=\"activity\" style=\"width:14px;height:14px\"></i></button>" +
                    "<button class=\"btn btn-outline-secondary btn-sm\" onclick=\"editProvider(''" + p.id + "'')\" title=\"Edit\"><i data-lucide=\"pencil\" style=\"width:14px;height:14px\"></i></button>" +
                    "<button class=\"btn btn-outline-danger btn-sm\" onclick=\"deleteProvider(''" + p.id + "'')\" title=\"Delete\"><i data-lucide=\"trash-2\" style=\"width:14px;height:14px\"></i></button>" +
                "</div>" +
            "</div>" +
        "</div>";
    }).join("");

    if (typeof lucide !== "undefined") lucide.createIcons();
}

function escapeHtml(text) {
    if (!text) return "";
    var div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
}

function showAddForm() {
    editingId = null;
    document.getElementById("formTitle").textContent = "Add LLM Provider";
    document.getElementById("editProviderId").value = "";
    document.getElementById("providerType").value = "";
    document.getElementById("displayName").value = "";
    document.getElementById("apiBase").value = "";
    document.getElementById("apiKey").value = "";
    document.getElementById("apiKey").placeholder = "sk-...";
    document.getElementById("defaultModel").innerHTML = "<option value=\"\">-- Select provider type first --</option>";
    document.getElementById("saveBtnText").textContent = "Save Provider";
    document.getElementById("formError").classList.add("d-none");
    document.getElementById("formSuccess").classList.add("d-none");
    document.getElementById("providerForm").classList.remove("d-none");
    if (typeof lucide !== "undefined") lucide.createIcons();
}

function hideForm() {
    document.getElementById("providerForm").classList.add("d-none");
    editingId = null;
}

function onProviderTypeChange() {
    var type = document.getElementById("providerType").value;
    var defaults = PROVIDER_DEFAULTS[type];
    if (!defaults) return;

    if (!editingId) {
        document.getElementById("displayName").value = defaults.displayName;
        document.getElementById("apiBase").value = defaults.apiBase;
    }

    var modelSelect = document.getElementById("defaultModel");
    if (defaults.models.length > 0) {
        modelSelect.innerHTML = defaults.models.map(function(m) {
            return "<option value=\"" + m + "\">" + m + "</option>";
        }).join("");
    } else {
        modelSelect.innerHTML = "<option value=\"\">-- Enter model ID below --</option>";
        // For custom, also add a text input
        if (type === "custom") {
            modelSelect.outerHTML = "<input type=\"text\" class=\"form-control\" id=\"defaultModel\" placeholder=\"model-name\">";
        }
    }
}

function editProvider(id) {
    var p = providers.find(function(x) { return x.id === id; });
    if (!p) return;

    editingId = id;
    document.getElementById("formTitle").textContent = "Edit Provider";
    document.getElementById("editProviderId").value = id;
    document.getElementById("providerType").value = p.name;
    document.getElementById("displayName").value = p.display_name;
    document.getElementById("apiBase").value = p.api_base;
    document.getElementById("apiKey").value = "";
    document.getElementById("apiKey").placeholder = "(unchanged - enter new key to update)";
    document.getElementById("saveBtnText").textContent = "Update Provider";
    document.getElementById("formError").classList.add("d-none");
    document.getElementById("formSuccess").classList.add("d-none");

    // Populate model list and select current
    onProviderTypeChange();
    var modelEl = document.getElementById("defaultModel");
    if (modelEl) {
        if (modelEl.tagName === "SELECT") {
            // Check if current model is in the list, if not add it
            var found = false;
            for (var i = 0; i < modelEl.options.length; i++) {
                if (modelEl.options[i].value === p.default_model) { found = true; break; }
            }
            if (!found) {
                var opt = document.createElement("option");
                opt.value = p.default_model;
                opt.textContent = p.default_model;
                modelEl.appendChild(opt);
            }
            modelEl.value = p.default_model;
        } else {
            modelEl.value = p.default_model;
        }
    }

    document.getElementById("providerForm").classList.remove("d-none");
    if (typeof lucide !== "undefined") lucide.createIcons();
}

async function saveProvider() {
    var errEl = document.getElementById("formError");
    var succEl = document.getElementById("formSuccess");
    var btn = document.getElementById("saveBtn");
    errEl.classList.add("d-none");
    succEl.classList.add("d-none");

    var type = document.getElementById("providerType").value;
    var displayName = document.getElementById("displayName").value.trim();
    var apiBase = document.getElementById("apiBase").value.trim();
    var apiKey = document.getElementById("apiKey").value.trim();
    var modelEl = document.getElementById("defaultModel");
    var defaultModel = modelEl ? (modelEl.value || "").trim() : "";

    if (!type) { showFormError("Please select a provider type."); return; }
    if (!displayName) { showFormError("Please enter a display name."); return; }
    if (!apiBase) { showFormError("Please enter an API base URL."); return; }
    if (!defaultModel) { showFormError("Please select or enter a default model."); return; }

    if (editingId) {
        // Update existing
        var updateBody = {
            display_name: displayName,
            api_base: apiBase,
            default_model: defaultModel
        };
        if (apiKey) updateBody.api_key = apiKey;

        btn.disabled = true;
        try {
            var resp = await fetch("/api/v1/llm-providers/" + editingId, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(updateBody)
            });
            if (!resp.ok) {
                var errData = await resp.text();
                throw new Error("HTTP " + resp.status + ": " + errData);
            }
            succEl.textContent = "Provider updated successfully.";
            succEl.classList.remove("d-none");
            await loadProviders();
            setTimeout(hideForm, 1200);
        } catch(err) {
            showFormError(err.message);
        } finally {
            btn.disabled = false;
        }
    } else {
        // Create new
        if (!apiKey) { showFormError("Please enter an API key."); return; }

        var createBody = {
            name: type,
            display_name: displayName,
            api_base: apiBase,
            api_key: apiKey,
            default_model: defaultModel,
            available_models: []
        };

        var defaults = PROVIDER_DEFAULTS[type];
        if (defaults && defaults.models.length > 0) {
            createBody.available_models = defaults.models;
        }

        btn.disabled = true;
        try {
            var resp = await fetch("/api/v1/llm-providers", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(createBody)
            });
            if (!resp.ok) {
                var errData = await resp.text();
                throw new Error("HTTP " + resp.status + ": " + errData);
            }
            succEl.textContent = "Provider created and activated!";
            succEl.classList.remove("d-none");
            await loadProviders();
            setTimeout(hideForm, 1500);
        } catch(err) {
            showFormError(err.message);
        } finally {
            btn.disabled = false;
        }
    }
}

function showFormError(msg) {
    var errEl = document.getElementById("formError");
    errEl.textContent = msg;
    errEl.classList.remove("d-none");
}

async function activateProvider(id) {
    try {
        var resp = await fetch("/api/v1/llm-providers/" + id + "/activate", { method: "POST" });
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        await loadProviders();
    } catch(err) {
        alert("Failed to activate provider: " + err.message);
    }
}

async function testProvider(id) {
    // Find the button and show spinner
    var p = providers.find(function(x) { return x.id === id; });
    if (!p) return;

    // Temporarily disable all test buttons
    document.querySelectorAll("[onclick*=testProvider]").forEach(function(b) { b.disabled = true; });

    try {
        var resp = await fetch("/api/v1/llm-providers/" + id + "/test", { method: "POST" });
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        var result = await resp.json();
        if (result.status === "ok") {
            alert("Connection successful! Provider verified.");
        } else {
            alert("Connection failed: " + (result.message || "Unknown error"));
        }
        await loadProviders();
    } catch(err) {
        alert("Test failed: " + err.message);
    } finally {
        document.querySelectorAll("[onclick*=testProvider]").forEach(function(b) { b.disabled = false; });
    }
}

async function deleteProvider(id) {
    var p = providers.find(function(x) { return x.id === id; });
    if (!p) return;
    if (!confirm("Delete provider \"" + p.display_name + "\"? This will also revoke its API key.")) return;

    try {
        var resp = await fetch("/api/v1/llm-providers/" + id, { method: "DELETE" });
        if (!resp.ok && resp.status !== 204) throw new Error("HTTP " + resp.status);
        await loadProviders();
    } catch(err) {
        alert("Failed to delete: " + err.message);
    }
}

function toggleApiKeyVisibility() {
    var input = document.getElementById("apiKey");
    var icon = document.getElementById("apiKeyVisIcon");
    if (input.type === "password") {
        input.type = "text";
        icon.setAttribute("data-lucide", "eye-off");
    } else {
        input.type = "password";
        icon.setAttribute("data-lucide", "eye");
    }
    if (typeof lucide !== "undefined") lucide.createIcons();
}',

    -- ═══════════════════════════════════════════════════════════════════
    -- CSS
    -- ═══════════════════════════════════════════════════════════════════
    '.font-monospace { font-family: SFMono-Regular, Menlo, Monaco, Consolas, monospace; letter-spacing: 0.03em; }
.border-success { border-color: #198754 !important; }
.bg-success.bg-opacity-10 { background-color: rgba(25, 135, 84, 0.1) !important; }
.card-header { background-color: rgba(0,0,0,0.03); }
.gap-1 { gap: 0.25rem; }
.gap-2 { gap: 0.5rem; }'
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
