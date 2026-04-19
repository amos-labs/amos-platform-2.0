-- Rebuild the system-settings canvas with conditional sections per provider mode.
--
-- Migration 54 introduced a mode toggle but left the BYOK provider card
-- visible in both modes, with no way to actually pick a Bedrock model.
-- This migration reseeds the canvas so:
--   * Mode toggle at the top (Shared Bedrock / BYOK).
--   * When Shared Bedrock is selected: show a model picker with the
--     available Bedrock models and their pricing. Hide the BYOK card.
--   * When BYOK is selected: show the BYOK provider management card.
--     Hide the Bedrock model picker.
--   * If shared Bedrock isn't available on this harness (no billing /
--     self-hosted), show the option but disable it with a link to the
--     platform billing page.
--
-- Full reseed via ON CONFLICT DO UPDATE — simpler to review than more
-- surgical REPLACEs, and the canvas is config not customer data.

INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-settings',
    'Settings',
    'Configure AMOS settings and LLM providers',
    'custom',
    true,
    'settings',
    10,
    '<div class="container-fluid p-4" style="max-width:800px">
    <h2 class="mb-4">Settings</h2>

    <!-- ── Provider Mode Toggle ── -->
    <div class="card mb-4">
        <div class="card-header">
            <h5 class="mb-0"><i data-lucide="settings" style="width:18px;height:18px;display:inline-block;vertical-align:middle;margin-right:6px"></i> AI Provider Mode</h5>
        </div>
        <div class="card-body">
            <p class="text-muted small mb-3">Choose which AI backs your chats. You can switch anytime.</p>
            <div class="form-check mb-2">
                <input class="form-check-input" type="radio" name="providerMode" id="modeSharedBedrock" value="shared_bedrock" onchange="onModeChange(this.value)">
                <label class="form-check-label" for="modeSharedBedrock">
                    <strong>Shared Bedrock (AMOS-hosted)</strong>
                    <div class="small text-muted">AMOS provides AWS Bedrock access. Billed at Bedrock pricing + 3% markup.</div>
                </label>
            </div>
            <div class="form-check">
                <input class="form-check-input" type="radio" name="providerMode" id="modeByok" value="byok" onchange="onModeChange(this.value)">
                <label class="form-check-label" for="modeByok">
                    <strong>Bring Your Own Key (BYOK)</strong>
                    <div class="small text-muted">Use your own Anthropic, OpenAI, or custom API key. Billed directly to your provider.</div>
                </label>
            </div>
            <div id="modeStatus" class="small mt-3"></div>
            <div id="billingGate" class="alert alert-warning small mt-3 d-none">
                Shared Bedrock requires an active subscription.
                <a id="billingUpgradeLink" href="#" target="_blank">Set up billing on the platform</a> to enable it.
            </div>
        </div>
    </div>

    <!-- ── Shared Bedrock Model Picker (shown when mode=shared_bedrock) ── -->
    <div class="card mb-4 d-none" id="bedrockModelCard">
        <div class="card-header">
            <h5 class="mb-0"><i data-lucide="cpu" style="width:18px;height:18px;display:inline-block;vertical-align:middle;margin-right:6px"></i> Bedrock Model</h5>
        </div>
        <div class="card-body">
            <p class="text-muted small mb-3">Pick the Claude model used for your chats. Prices shown include AMOS''s 3% markup.</p>
            <div id="bedrockModelList"></div>
        </div>
    </div>

    <!-- ── BYOK Provider Section (shown when mode=byok) ── -->
    <div class="card mb-4 d-none" id="byokCard">
        <div class="card-header d-flex justify-content-between align-items-center">
            <h5 class="mb-0"><i data-lucide="brain" style="width:18px;height:18px;display:inline-block;vertical-align:middle;margin-right:6px"></i> AI Provider</h5>
            <button class="btn btn-primary btn-sm" onclick="showAddForm()">
                <i data-lucide="plus" style="width:14px;height:14px;display:inline-block;vertical-align:middle;margin-right:2px"></i> Add Provider
            </button>
        </div>
        <div class="card-body">
            <p class="text-muted small mb-3">Add your Anthropic or OpenAI API key to enable AI chat. Your key is encrypted and stored in the harness credential vault.</p>
            <div id="providersList">
                <div class="text-center text-muted py-3">
                    <div class="spinner-border spinner-border-sm me-2" role="status"></div>
                    Loading providers...
                </div>
            </div>
        </div>
    </div>

    <!-- ── Add/Edit BYOK Provider Form ── -->
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

    <!-- ── About ── -->
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
var currentSettings = null;

var PROVIDER_DEFAULTS = {
    anthropic: {
        displayName: "Anthropic (Claude)",
        apiBase: "https://api.anthropic.com/v1",
        models: [
            "claude-sonnet-4-6",
            "claude-opus-4-6",
            "claude-opus-4-7",
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
    loadSettings().then(loadProviders);
});

// ─── Settings + mode handling ─────────────────────────────────────────

async function loadSettings() {
    try {
        var resp = await fetch("/api/v1/settings");
        if (!resp.ok) return;
        currentSettings = await resp.json();
        var mode = currentSettings.llm_provider_mode || (currentSettings.shared_bedrock_available ? "shared_bedrock" : "byok");

        // Set radio state
        var radio = document.getElementById(mode === "byok" ? "modeByok" : "modeSharedBedrock");
        if (radio) radio.checked = true;

        // Billing gate for Shared Bedrock when unavailable
        var sbRadio = document.getElementById("modeSharedBedrock");
        var gate = document.getElementById("billingGate");
        if (!currentSettings.shared_bedrock_available) {
            if (sbRadio) sbRadio.disabled = true;
            if (gate) {
                gate.classList.remove("d-none");
                var link = document.getElementById("billingUpgradeLink");
                if (link) link.href = (currentSettings.platform_url || "https://app.amoslabs.com") + "/billing";
            }
            // Force to BYOK visually if Bedrock isn''t available but somehow was selected
            if (mode === "shared_bedrock") {
                mode = "byok";
                var byokRadio = document.getElementById("modeByok");
                if (byokRadio) byokRadio.checked = true;
            }
        } else {
            if (sbRadio) sbRadio.disabled = false;
            if (gate) gate.classList.add("d-none");
        }

        showSection(mode);
        renderBedrockModels();
        updateModeStatus(mode);
    } catch(e) { /* non-fatal */ }
}

async function onModeChange(mode) {
    // Guard: can''t pick Bedrock if unavailable
    if (mode === "shared_bedrock" && currentSettings && !currentSettings.shared_bedrock_available) {
        alert("Shared Bedrock is not available on this harness. Set up billing on the platform to enable it.");
        await loadSettings();
        return;
    }
    try {
        var resp = await fetch("/api/v1/settings", {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ llm_provider_mode: mode })
        });
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        showSection(mode);
        updateModeStatus(mode);
        if (mode === "byok") loadProviders();
    } catch(err) {
        alert("Failed to change provider mode: " + err.message);
        await loadSettings();
    }
}

function showSection(mode) {
    var bedrockCard = document.getElementById("bedrockModelCard");
    var byokCard = document.getElementById("byokCard");
    if (mode === "shared_bedrock") {
        if (bedrockCard) bedrockCard.classList.remove("d-none");
        if (byokCard) byokCard.classList.add("d-none");
        hideForm();
    } else {
        if (bedrockCard) bedrockCard.classList.add("d-none");
        if (byokCard) byokCard.classList.remove("d-none");
    }
    if (typeof lucide !== "undefined") lucide.createIcons();
}

function updateModeStatus(mode) {
    var el = document.getElementById("modeStatus");
    if (!el) return;
    if (mode === "byok") {
        var hasActive = providers.some(function(p) { return p.is_active; });
        if (!hasActive) {
            el.innerHTML = "<span class=\"text-warning\">⚠ BYOK is selected but no provider is activated. Add an API key below and click the green check to activate it, or chats will fail.</span>";
        } else {
            el.innerHTML = "<span class=\"text-success\">✓ Using BYOK — chats route through your active provider.</span>";
        }
    } else {
        el.innerHTML = "<span class=\"text-success\">✓ Using shared Bedrock — AMOS handles the AI access.</span>";
    }
}

function renderBedrockModels() {
    var list = document.getElementById("bedrockModelList");
    if (!list || !currentSettings || !currentSettings.available_models) return;
    var current = currentSettings.llm_model;
    list.innerHTML = currentSettings.available_models.map(function(m) {
        var selected = m.id === current;
        return "<div class=\"form-check mb-2\">" +
            "<input class=\"form-check-input\" type=\"radio\" name=\"bedrockModel\" id=\"bm-" + m.id + "\" value=\"" + escapeHtml(m.id) + "\" " + (selected ? "checked" : "") + " onchange=\"onBedrockModelChange(this.value)\">" +
            "<label class=\"form-check-label\" for=\"bm-" + m.id + "\">" +
                "<strong>" + escapeHtml(m.display_name) + "</strong> " +
                "<span class=\"badge bg-light text-dark ms-1\">" + escapeHtml(m.tier) + "</span>" +
                "<div class=\"small text-muted\">$" + m.input_price_per_mtok.toFixed(2) + " input / $" + m.output_price_per_mtok.toFixed(2) + " output per MTok</div>" +
            "</label>" +
        "</div>";
    }).join("");
}

async function onBedrockModelChange(modelId) {
    try {
        var resp = await fetch("/api/v1/settings", {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ llm_model: modelId })
        });
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        currentSettings.llm_model = modelId;
    } catch(err) {
        alert("Failed to change model: " + err.message);
        await loadSettings();
    }
}

// ─── BYOK provider management (unchanged behavior) ────────────────────

async function loadProviders() {
    try {
        var resp = await fetch("/api/v1/llm-providers");
        if (!resp.ok) throw new Error("Failed to load");
        providers = await resp.json();
        renderProviders();
        var activeMode = document.querySelector(''input[name="providerMode"]:checked'');
        if (activeMode) updateModeStatus(activeMode.value);
    } catch(err) {
        document.getElementById("providersList").innerHTML =
            "<div class=\"text-center text-muted py-3\">Failed to load providers. <a href=\"#\" onclick=\"loadProviders();return false;\">Retry</a></div>";
    }
}

function renderProviders() {
    var list = document.getElementById("providersList");
    if (!list) return;
    if (providers.length === 0) {
        list.innerHTML = "<div class=\"text-center py-4\"><p class=\"text-muted mb-2\">No LLM providers configured.</p><p class=\"small text-muted\">Add your Anthropic or OpenAI API key above to enable AI chat.</p></div>";
        if (typeof lucide !== "undefined") lucide.createIcons();
        return;
    }
    list.innerHTML = providers.map(function(p) {
        var statusBadge = p.is_active
            ? "<span class=\"badge bg-success\">Active</span>"
            : p.is_verified ? "<span class=\"badge bg-secondary\">Verified</span>"
            : "<span class=\"badge bg-warning text-dark\">Unverified</span>";
        var providerIcon = p.name === "anthropic" ? "sparkles" : p.name === "openai" ? "zap" : "cpu";
        var errorHtml = p.last_error ? "<div class=\"small text-danger mt-1\" style=\"word-break:break-all;\"><i data-lucide=\"alert-circle\" style=\"width:12px;height:12px;display:inline-block;vertical-align:middle;margin-right:2px\"></i>" + escapeHtml(p.last_error).substring(0, 120) + "</div>" : "";
        var activeBtn = p.is_active ? "" : "<button class=\"btn btn-outline-success btn-sm\" onclick=\"activateProvider(''" + p.id + "'')\" title=\"Set as active\"><i data-lucide=\"check-circle\" style=\"width:14px;height:14px\"></i></button>";
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
    var el = document.getElementById("providerForm");
    if (el) el.classList.add("d-none");
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
    onProviderTypeChange();
    var modelEl = document.getElementById("defaultModel");
    if (modelEl) {
        if (modelEl.tagName === "SELECT") {
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
            if (!resp.ok) throw new Error("HTTP " + resp.status + ": " + (await resp.text()));
            succEl.textContent = "Provider updated successfully.";
            succEl.classList.remove("d-none");
            await loadProviders();
            setTimeout(hideForm, 1200);
        } catch(err) { showFormError(err.message); }
        finally { btn.disabled = false; }
    } else {
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
        if (defaults && defaults.models.length > 0) createBody.available_models = defaults.models;
        btn.disabled = true;
        try {
            var resp = await fetch("/api/v1/llm-providers", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(createBody)
            });
            if (!resp.ok) throw new Error("HTTP " + resp.status + ": " + (await resp.text()));
            succEl.textContent = "Provider created and activated!";
            succEl.classList.remove("d-none");
            await loadSettings();  // Activation flips mode; resync UI.
            await loadProviders();
            setTimeout(hideForm, 1500);
        } catch(err) { showFormError(err.message); }
        finally { btn.disabled = false; }
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
        await loadSettings();   // activation now also flips mode → refresh
        await loadProviders();
    } catch(err) { alert("Failed to activate provider: " + err.message); }
}

async function testProvider(id) {
    var p = providers.find(function(x) { return x.id === id; });
    if (!p) return;
    document.querySelectorAll("[onclick*=testProvider]").forEach(function(b) { b.disabled = true; });
    try {
        var resp = await fetch("/api/v1/llm-providers/" + id + "/test", { method: "POST" });
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        var result = await resp.json();
        alert(result.status === "ok" ? "Connection successful! Provider verified." : "Connection failed: " + (result.message || "Unknown error"));
        await loadProviders();
    } catch(err) { alert("Test failed: " + err.message); }
    finally { document.querySelectorAll("[onclick*=testProvider]").forEach(function(b) { b.disabled = false; }); }
}

async function deleteProvider(id) {
    var p = providers.find(function(x) { return x.id === id; });
    if (!p) return;
    if (!confirm("Delete provider \"" + p.display_name + "\"? This will also revoke its API key.")) return;
    try {
        var resp = await fetch("/api/v1/llm-providers/" + id, { method: "DELETE" });
        if (!resp.ok && resp.status !== 204) throw new Error("HTTP " + resp.status);
        await loadProviders();
    } catch(err) { alert("Failed to delete: " + err.message); }
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
.gap-2 { gap: 0.5rem; }
.form-check-input:disabled + .form-check-label { opacity: 0.5; }'
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
