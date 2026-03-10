-- Seed auth canvases and upgrade settings canvas to full account management
-- Auth canvases are system canvases but NOT sidebar nav items (nav_order = 0).
-- They are served at /login, /register, /forgot-password as public pages.
-- The settings canvas (nav_order 10) is upgraded from a basic model selector
-- to a full account management UI with org settings, users, API keys, etc.

-- ============================================================================
-- 1. Login Canvas (public auth page)
-- ============================================================================
INSERT INTO canvases (slug, name, description, canvas_type, is_system, is_public, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-login',
    'Login',
    'AMOS Login Page',
    'custom',
    true,
    true,
    'log-in',
    0,
    -- HTML
    '<div class="auth-container">
    <div class="auth-card">
        <div class="auth-header">
            <h1 class="auth-logo">AMOS</h1>
            <p class="auth-subtitle">Sign in to your account</p>
        </div>
        <form id="login-form" onsubmit="handleLogin(event)">
            <div class="mb-3">
                <label class="form-label" for="login-email">Email address</label>
                <input type="email" class="form-control" id="login-email" placeholder="you@company.com" required autofocus>
            </div>
            <div class="mb-3">
                <label class="form-label" for="login-password">Password</label>
                <input type="password" class="form-control" id="login-password" placeholder="Enter your password" required minlength="8">
            </div>
            <div id="login-error" class="alert alert-danger d-none" role="alert"></div>
            <button type="submit" class="btn btn-primary w-100 mb-3" id="login-btn">
                Sign In
            </button>
        </form>
        <div class="auth-links">
            <a href="/forgot-password">Forgot password?</a>
            <span class="mx-2">|</span>
            <a href="/register">Create an account</a>
        </div>
    </div>
    <div class="auth-footer">
        <small class="text-muted">Powered by AMOS &mdash; Autonomous Management Operating System</small>
    </div>
</div>',
    -- JS
    'var PLATFORM_URL = window.AMOS_PLATFORM_URL || "";

async function handleLogin(e) {
    e.preventDefault();
    var btn = document.getElementById("login-btn");
    var errEl = document.getElementById("login-error");
    errEl.classList.add("d-none");
    btn.disabled = true;
    btn.textContent = "Signing in...";

    var email = document.getElementById("login-email").value.trim();
    var password = document.getElementById("login-password").value;

    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/auth/login", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ email: email, password: password })
        });
        var data = await resp.json();
        if (!resp.ok) {
            throw new Error(data.error || data.message || "Login failed");
        }
        // Store auth tokens
        localStorage.setItem("amos-token", data.access_token);
        localStorage.setItem("amos-refresh-token", data.refresh_token);
        localStorage.setItem("amos-tenant", data.tenant_slug);
        localStorage.setItem("amos-role", data.role);
        localStorage.setItem("amos-user-id", data.user_id);
        localStorage.setItem("amos-tenant-id", data.tenant_id);
        // Redirect to main app
        window.location.href = "/";
    } catch(err) {
        errEl.textContent = err.message;
        errEl.classList.remove("d-none");
    } finally {
        btn.disabled = false;
        btn.textContent = "Sign In";
    }
}

// If already logged in, redirect
document.addEventListener("DOMContentLoaded", function() {
    var token = localStorage.getItem("amos-token");
    if (token) {
        window.location.href = "/";
    }
});',
    -- CSS
    '.auth-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    background: linear-gradient(135deg, #1a1a2e 0%, #16213e 50%, #0f3460 100%);
    padding: 2rem;
}
.auth-card {
    background: var(--bs-body-bg, #fff);
    border-radius: 12px;
    padding: 2.5rem;
    width: 100%;
    max-width: 420px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.3);
}
.auth-header { text-align: center; margin-bottom: 2rem; }
.auth-logo { font-size: 2rem; font-weight: 800; letter-spacing: 0.15em; color: var(--bs-primary, #6366f1); margin-bottom: 0.25rem; }
.auth-subtitle { color: var(--bs-secondary-color, #6c757d); font-size: 0.95rem; }
.auth-links { text-align: center; font-size: 0.875rem; }
.auth-links a { color: var(--bs-primary, #6366f1); text-decoration: none; }
.auth-links a:hover { text-decoration: underline; }
.auth-footer { margin-top: 2rem; text-align: center; }
.auth-footer small { color: rgba(255,255,255,0.5); }'
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    is_system = true,
    is_public = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();

-- ============================================================================
-- 2. Register Canvas (public auth page)
-- ============================================================================
INSERT INTO canvases (slug, name, description, canvas_type, is_system, is_public, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-register',
    'Register',
    'AMOS Registration Page',
    'custom',
    true,
    true,
    'user-plus',
    0,
    -- HTML
    '<div class="auth-container">
    <div class="auth-card auth-card-wide">
        <div class="auth-header">
            <h1 class="auth-logo">AMOS</h1>
            <p class="auth-subtitle">Create your account</p>
        </div>
        <form id="register-form" onsubmit="handleRegister(event)">
            <div class="mb-3">
                <label class="form-label" for="reg-org">Organization Name</label>
                <input type="text" class="form-control" id="reg-org" placeholder="Acme Inc." required autofocus>
            </div>
            <div class="row">
                <div class="col-md-6 mb-3">
                    <label class="form-label" for="reg-name">Your Name</label>
                    <input type="text" class="form-control" id="reg-name" placeholder="Jane Smith" required>
                </div>
                <div class="col-md-6 mb-3">
                    <label class="form-label" for="reg-email">Email Address</label>
                    <input type="email" class="form-control" id="reg-email" placeholder="you@company.com" required>
                </div>
            </div>
            <div class="row">
                <div class="col-md-6 mb-3">
                    <label class="form-label" for="reg-password">Password</label>
                    <input type="password" class="form-control" id="reg-password" placeholder="Min 8 characters" required minlength="8">
                </div>
                <div class="col-md-6 mb-3">
                    <label class="form-label" for="reg-confirm">Confirm Password</label>
                    <input type="password" class="form-control" id="reg-confirm" placeholder="Confirm password" required minlength="8">
                </div>
            </div>
            <div class="mb-3">
                <label class="form-label" for="reg-plan">Plan</label>
                <select class="form-select" id="reg-plan">
                    <option value="free">Free &mdash; Get started at no cost</option>
                    <option value="starter" selected>Starter &mdash; $99/mo</option>
                    <option value="growth">Growth &mdash; $499/mo</option>
                    <option value="enterprise">Enterprise &mdash; Custom pricing</option>
                </select>
            </div>
            <div id="register-error" class="alert alert-danger d-none" role="alert"></div>
            <div id="register-success" class="alert alert-success d-none" role="alert"></div>
            <button type="submit" class="btn btn-primary w-100 mb-3" id="register-btn">
                Create Account
            </button>
        </form>
        <div class="auth-links">
            <span>Already have an account?</span>
            <a href="/login">Sign in</a>
        </div>
    </div>
    <div class="auth-footer">
        <small class="text-muted">Powered by AMOS &mdash; Autonomous Management Operating System</small>
    </div>
</div>',
    -- JS
    'var PLATFORM_URL = window.AMOS_PLATFORM_URL || "";

async function handleRegister(e) {
    e.preventDefault();
    var btn = document.getElementById("register-btn");
    var errEl = document.getElementById("register-error");
    var successEl = document.getElementById("register-success");
    errEl.classList.add("d-none");
    successEl.classList.add("d-none");

    var password = document.getElementById("reg-password").value;
    var confirm = document.getElementById("reg-confirm").value;
    if (password !== confirm) {
        errEl.textContent = "Passwords do not match";
        errEl.classList.remove("d-none");
        return;
    }

    btn.disabled = true;
    btn.textContent = "Creating account...";

    var payload = {
        organization_name: document.getElementById("reg-org").value.trim(),
        name: document.getElementById("reg-name").value.trim(),
        email: document.getElementById("reg-email").value.trim(),
        password: password,
        plan: document.getElementById("reg-plan").value
    };

    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/auth/register", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(payload)
        });
        var data = await resp.json();
        if (!resp.ok) {
            var msg = data.error || data.message || "Registration failed";
            if (data.hint) msg += " (" + data.hint + ")";
            throw new Error(msg);
        }
        // Store auth tokens
        localStorage.setItem("amos-token", data.access_token);
        localStorage.setItem("amos-refresh-token", data.refresh_token);
        localStorage.setItem("amos-tenant", data.slug);
        localStorage.setItem("amos-user-id", data.user_id);
        localStorage.setItem("amos-tenant-id", data.tenant_id);
        // Show success briefly, then redirect
        successEl.textContent = "Account created! Redirecting...";
        successEl.classList.remove("d-none");
        setTimeout(function() { window.location.href = "/"; }, 1000);
    } catch(err) {
        errEl.textContent = err.message;
        errEl.classList.remove("d-none");
    } finally {
        btn.disabled = false;
        btn.textContent = "Create Account";
    }
}

document.addEventListener("DOMContentLoaded", function() {
    var token = localStorage.getItem("amos-token");
    if (token) {
        window.location.href = "/";
    }
});',
    -- CSS (reuses .auth-container etc from login; canvas CSS is scoped)
    '.auth-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    background: linear-gradient(135deg, #1a1a2e 0%, #16213e 50%, #0f3460 100%);
    padding: 2rem;
}
.auth-card {
    background: var(--bs-body-bg, #fff);
    border-radius: 12px;
    padding: 2.5rem;
    width: 100%;
    max-width: 420px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.3);
}
.auth-card-wide { max-width: 540px; }
.auth-header { text-align: center; margin-bottom: 2rem; }
.auth-logo { font-size: 2rem; font-weight: 800; letter-spacing: 0.15em; color: var(--bs-primary, #6366f1); margin-bottom: 0.25rem; }
.auth-subtitle { color: var(--bs-secondary-color, #6c757d); font-size: 0.95rem; }
.auth-links { text-align: center; font-size: 0.875rem; }
.auth-links a { color: var(--bs-primary, #6366f1); text-decoration: none; margin-left: 0.25rem; }
.auth-links a:hover { text-decoration: underline; }
.auth-footer { margin-top: 2rem; text-align: center; }
.auth-footer small { color: rgba(255,255,255,0.5); }'
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    is_system = true,
    is_public = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();

-- ============================================================================
-- 3. Forgot Password Canvas (public auth page)
-- NOTE: Backend /api/v1/auth/forgot-password endpoint does not exist yet.
-- This canvas is a placeholder that collects the email; the actual reset
-- flow will be wired once the backend endpoint is implemented.
-- ============================================================================
INSERT INTO canvases (slug, name, description, canvas_type, is_system, is_public, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-forgot-password',
    'Forgot Password',
    'AMOS Password Reset Page',
    'custom',
    true,
    true,
    'key-round',
    0,
    -- HTML
    '<div class="auth-container">
    <div class="auth-card">
        <div class="auth-header">
            <h1 class="auth-logo">AMOS</h1>
            <p class="auth-subtitle">Reset your password</p>
        </div>
        <div id="reset-form-view">
            <p class="text-muted mb-3" style="font-size:0.9rem">Enter your email address and we will send you a link to reset your password.</p>
            <form id="reset-form" onsubmit="handleResetRequest(event)">
                <div class="mb-3">
                    <label class="form-label" for="reset-email">Email address</label>
                    <input type="email" class="form-control" id="reset-email" placeholder="you@company.com" required autofocus>
                </div>
                <div id="reset-error" class="alert alert-danger d-none" role="alert"></div>
                <button type="submit" class="btn btn-primary w-100 mb-3" id="reset-btn">
                    Send Reset Link
                </button>
            </form>
        </div>
        <div id="reset-success-view" style="display:none">
            <div class="text-center py-3">
                <div style="font-size:3rem;margin-bottom:1rem">&#9993;</div>
                <h5>Check your email</h5>
                <p class="text-muted">If an account exists for <strong id="reset-email-display"></strong>, we have sent password reset instructions.</p>
            </div>
        </div>
        <div class="auth-links">
            <a href="/login">Back to sign in</a>
        </div>
    </div>
    <div class="auth-footer">
        <small class="text-muted">Powered by AMOS &mdash; Autonomous Management Operating System</small>
    </div>
</div>',
    -- JS
    'var PLATFORM_URL = window.AMOS_PLATFORM_URL || "";

async function handleResetRequest(e) {
    e.preventDefault();
    var btn = document.getElementById("reset-btn");
    var errEl = document.getElementById("reset-error");
    errEl.classList.add("d-none");
    btn.disabled = true;
    btn.textContent = "Sending...";

    var email = document.getElementById("reset-email").value.trim();

    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/auth/forgot-password", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ email: email })
        });
        // Always show success (do not reveal whether account exists)
        document.getElementById("reset-email-display").textContent = email;
        document.getElementById("reset-form-view").style.display = "none";
        document.getElementById("reset-success-view").style.display = "block";
    } catch(err) {
        // Even on network error, show success to avoid info leakage
        document.getElementById("reset-email-display").textContent = email;
        document.getElementById("reset-form-view").style.display = "none";
        document.getElementById("reset-success-view").style.display = "block";
    } finally {
        btn.disabled = false;
        btn.textContent = "Send Reset Link";
    }
}',
    -- CSS
    '.auth-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    background: linear-gradient(135deg, #1a1a2e 0%, #16213e 50%, #0f3460 100%);
    padding: 2rem;
}
.auth-card {
    background: var(--bs-body-bg, #fff);
    border-radius: 12px;
    padding: 2.5rem;
    width: 100%;
    max-width: 420px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.3);
}
.auth-header { text-align: center; margin-bottom: 2rem; }
.auth-logo { font-size: 2rem; font-weight: 800; letter-spacing: 0.15em; color: var(--bs-primary, #6366f1); margin-bottom: 0.25rem; }
.auth-subtitle { color: var(--bs-secondary-color, #6c757d); font-size: 0.95rem; }
.auth-links { text-align: center; font-size: 0.875rem; margin-top: 1.5rem; }
.auth-links a { color: var(--bs-primary, #6366f1); text-decoration: none; }
.auth-links a:hover { text-decoration: underline; }
.auth-footer { margin-top: 2rem; text-align: center; }
.auth-footer small { color: rgba(255,255,255,0.5); }'
)
ON CONFLICT (slug) DO UPDATE SET
    html_content = EXCLUDED.html_content,
    js_content = EXCLUDED.js_content,
    css_content = EXCLUDED.css_content,
    is_system = true,
    is_public = true,
    nav_icon = EXCLUDED.nav_icon,
    nav_order = EXCLUDED.nav_order,
    updated_at = NOW();

-- ============================================================================
-- 4. Upgraded Settings / Account Management Canvas
-- Replaces the basic model-selector-only settings canvas with full account
-- management: org info, AI model, users, API keys, password change, billing.
-- ============================================================================
INSERT INTO canvases (slug, name, description, canvas_type, is_system, nav_icon, nav_order, html_content, js_content, css_content)
VALUES (
    'system-settings',
    'Settings',
    'Account settings and management',
    'custom',
    true,
    'settings',
    10,
    -- HTML
    '<div class="container-fluid p-4" style="max-width:900px">
    <h2 class="mb-4">Account Settings</h2>

    <!-- Tab navigation -->
    <ul class="nav nav-tabs mb-4" id="settings-tabs">
        <li class="nav-item"><a class="nav-link active" href="#" onclick="showTab(''general'', event)">General</a></li>
        <li class="nav-item"><a class="nav-link" href="#" onclick="showTab(''users'', event)">Users</a></li>
        <li class="nav-item"><a class="nav-link" href="#" onclick="showTab(''apikeys'', event)">API Keys</a></li>
        <li class="nav-item"><a class="nav-link" href="#" onclick="showTab(''billing'', event)">Billing</a></li>
        <li class="nav-item"><a class="nav-link" href="#" onclick="showTab(''security'', event)">Security</a></li>
    </ul>

    <!-- General tab -->
    <div id="tab-general" class="tab-pane">
        <div class="card mb-3">
            <div class="card-body">
                <h5 class="card-title">Organization</h5>
                <div class="row g-3">
                    <div class="col-md-6">
                        <label class="form-label small">Organization Name</label>
                        <input type="text" class="form-control form-control-sm" id="org-name" placeholder="Loading...">
                    </div>
                    <div class="col-md-6">
                        <label class="form-label small">Tenant Slug</label>
                        <input type="text" class="form-control form-control-sm" id="org-slug" readonly>
                    </div>
                </div>
                <div class="mt-2 text-end">
                    <button class="btn btn-sm btn-primary" onclick="saveOrgSettings()">Save Changes</button>
                </div>
            </div>
        </div>
        <div class="card mb-3">
            <div class="card-body">
                <h5 class="card-title">AI Model</h5>
                <p class="text-muted small mb-2">Default model used for conversations and canvas generation.</p>
                <select id="modelSelect" class="form-select form-select-sm" style="max-width:400px" onchange="saveModelSetting(this.value)">
                    <option value="us.anthropic.claude-sonnet-4-20250514-v1:0">Claude Sonnet 4</option>
                    <option value="us.anthropic.claude-opus-4-20250514-v1:0">Claude Opus 4</option>
                    <option value="us.anthropic.claude-3-5-haiku-20241022-v1:0">Claude 3.5 Haiku</option>
                </select>
            </div>
        </div>
        <div class="card mb-3">
            <div class="card-body">
                <h5 class="card-title">Platform Connection</h5>
                <label class="form-label small">Platform URL</label>
                <input type="text" class="form-control form-control-sm" id="platform-url" readonly style="max-width:400px">
            </div>
        </div>
        <div class="card mb-3">
            <div class="card-body">
                <h5 class="card-title">About</h5>
                <p class="text-muted mb-0">AMOS Harness v0.1.0</p>
            </div>
        </div>
    </div>

    <!-- Users tab -->
    <div id="tab-users" class="tab-pane" style="display:none">
        <div class="card">
            <div class="card-body">
                <div class="d-flex justify-content-between align-items-center mb-3">
                    <h5 class="card-title mb-0">Team Members</h5>
                    <button class="btn btn-sm btn-primary" onclick="showInviteUser()">
                        <i data-lucide="user-plus" style="width:14px;height:14px;display:inline-block;vertical-align:middle;margin-right:2px"></i> Invite User
                    </button>
                </div>
                <div id="users-list">
                    <div class="text-muted py-3">Loading users...</div>
                </div>
            </div>
        </div>
        <!-- Invite user modal -->
        <div id="invite-modal" class="settings-modal-overlay" style="display:none">
            <div class="settings-modal-box">
                <h5>Invite Team Member</h5>
                <div class="mb-3">
                    <label class="form-label small">Email</label>
                    <input type="email" class="form-control form-control-sm" id="invite-email" placeholder="colleague@company.com">
                </div>
                <div class="mb-3">
                    <label class="form-label small">Role</label>
                    <select class="form-select form-select-sm" id="invite-role">
                        <option value="member">Member</option>
                        <option value="admin">Admin</option>
                    </select>
                </div>
                <div id="invite-error" class="alert alert-danger d-none small" role="alert"></div>
                <div class="d-flex justify-content-end gap-2">
                    <button class="btn btn-sm btn-outline-secondary" onclick="closeInviteModal()">Cancel</button>
                    <button class="btn btn-sm btn-primary" onclick="sendInvite()">Send Invite</button>
                </div>
            </div>
        </div>
    </div>

    <!-- API Keys tab -->
    <div id="tab-apikeys" class="tab-pane" style="display:none">
        <div class="card">
            <div class="card-body">
                <div class="d-flex justify-content-between align-items-center mb-3">
                    <div>
                        <h5 class="card-title mb-1">API Keys</h5>
                        <p class="text-muted small mb-0">Use API keys to access AMOS programmatically.</p>
                    </div>
                    <button class="btn btn-sm btn-primary" onclick="createApiKey()">
                        <i data-lucide="key" style="width:14px;height:14px;display:inline-block;vertical-align:middle;margin-right:2px"></i> Create Key
                    </button>
                </div>
                <div id="apikeys-list">
                    <div class="text-muted py-3">Loading API keys...</div>
                </div>
                <!-- New key display (shown once after creation) -->
                <div id="new-key-banner" class="alert alert-warning d-none mt-3" role="alert">
                    <strong>Save this key now!</strong> It will not be shown again.<br>
                    <code id="new-key-value" style="word-break:break-all"></code>
                    <button class="btn btn-sm btn-outline-dark ms-2" onclick="copyNewKey()">Copy</button>
                </div>
            </div>
        </div>
    </div>

    <!-- Billing tab -->
    <div id="tab-billing" class="tab-pane" style="display:none">
        <div class="card mb-3">
            <div class="card-body">
                <h5 class="card-title">Current Plan</h5>
                <div class="d-flex align-items-center mb-2">
                    <span class="badge bg-primary me-2 fs-6" id="billing-plan-name">Loading...</span>
                    <span class="text-muted" id="billing-plan-price"></span>
                </div>
                <p class="text-muted small mb-0" id="billing-plan-desc"></p>
            </div>
        </div>
        <div class="card mb-3">
            <div class="card-body">
                <h5 class="card-title">Available Plans</h5>
                <div id="billing-plans-list" class="row g-3">
                    <div class="text-muted py-3">Loading plans...</div>
                </div>
            </div>
        </div>
    </div>

    <!-- Security tab -->
    <div id="tab-security" class="tab-pane" style="display:none">
        <div class="card mb-3">
            <div class="card-body">
                <h5 class="card-title">Change Password</h5>
                <form id="password-form" onsubmit="changePassword(event)">
                    <div class="row g-3 mb-2">
                        <div class="col-md-6">
                            <label class="form-label small">New Password</label>
                            <input type="password" class="form-control form-control-sm" id="new-password" minlength="8" required>
                        </div>
                        <div class="col-md-6">
                            <label class="form-label small">Confirm New Password</label>
                            <input type="password" class="form-control form-control-sm" id="confirm-password" minlength="8" required>
                        </div>
                    </div>
                    <div id="password-error" class="alert alert-danger d-none small" role="alert"></div>
                    <div id="password-success" class="alert alert-success d-none small" role="alert"></div>
                    <button type="submit" class="btn btn-sm btn-primary">Update Password</button>
                </form>
            </div>
        </div>
        <div class="card mb-3">
            <div class="card-body">
                <h5 class="card-title">Sessions</h5>
                <p class="text-muted small">Sign out of all other devices.</p>
                <button class="btn btn-sm btn-outline-danger" onclick="logoutAll()">Sign Out Everywhere</button>
            </div>
        </div>
        <div class="card border-danger">
            <div class="card-body">
                <h5 class="card-title text-danger">Danger Zone</h5>
                <p class="text-muted small">Sign out of this session.</p>
                <button class="btn btn-sm btn-danger" onclick="logoutCurrent()">Sign Out</button>
            </div>
        </div>
    </div>
</div>',
    -- JS
    'var PLATFORM_URL = window.AMOS_PLATFORM_URL || "";
var currentTab = "general";

function getToken() { return localStorage.getItem("amos-token") || ""; }
function authHeaders() { return { "Content-Type": "application/json", "Authorization": "Bearer " + getToken() }; }

document.addEventListener("DOMContentLoaded", function() {
    loadGeneralSettings();
});

function showTab(tab, e) {
    if (e) e.preventDefault();
    // Update tab nav
    document.querySelectorAll("#settings-tabs .nav-link").forEach(function(a) { a.classList.remove("active"); });
    if (e) e.target.classList.add("active");
    // Hide all panes, show selected
    document.querySelectorAll(".tab-pane").forEach(function(p) { p.style.display = "none"; });
    document.getElementById("tab-" + tab).style.display = "block";
    currentTab = tab;
    // Load data for tab
    if (tab === "users") loadUsers();
    else if (tab === "apikeys") loadApiKeys();
    else if (tab === "billing") loadBilling();
}

// ---- General ----
async function loadGeneralSettings() {
    // Model preference
    var saved = localStorage.getItem("amos-model");
    if (saved) {
        var sel = document.getElementById("modelSelect");
        if (sel) sel.value = saved;
    }
    // Platform URL
    var urlEl = document.getElementById("platform-url");
    if (urlEl) urlEl.value = PLATFORM_URL || window.location.origin;
    // Org info
    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/tenants/me", { headers: authHeaders() });
        if (resp.ok) {
            var data = await resp.json();
            document.getElementById("org-name").value = data.name || "";
            document.getElementById("org-slug").value = data.slug || "";
        }
    } catch(err) { console.error("Failed to load org info:", err); }
}

function saveModelSetting(value) {
    localStorage.setItem("amos-model", value);
    window.parent.postMessage({ type: "amos-setting", key: "model", value: value }, "*");
}

async function saveOrgSettings() {
    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/tenants/me", {
            method: "PUT",
            headers: authHeaders(),
            body: JSON.stringify({ name: document.getElementById("org-name").value.trim() })
        });
        if (!resp.ok) throw new Error("Failed to update");
        alert("Organization settings saved.");
    } catch(err) { alert("Failed to save: " + err.message); }
}

// ---- Users ----
async function loadUsers() {
    var list = document.getElementById("users-list");
    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/tenants/me/users", { headers: authHeaders() });
        if (!resp.ok) throw new Error("Failed to load");
        var users = await resp.json();
        if (!users || users.length === 0) {
            list.innerHTML = "<p class=\"text-muted\">No users found.</p>";
            return;
        }
        list.innerHTML = "<div class=\"list-group\">" + users.map(function(u) {
            var roleBadge = u.role === "owner" ? "bg-warning text-dark" : (u.role === "admin" ? "bg-info" : "bg-secondary");
            return "<div class=\"list-group-item d-flex justify-content-between align-items-center\"><div><strong>" + escH(u.name || u.email) + "</strong><br><small class=\"text-muted\">" + escH(u.email) + "</small></div><span class=\"badge " + roleBadge + "\">" + u.role + "</span></div>";
        }).join("") + "</div>";
    } catch(err) {
        list.innerHTML = "<p class=\"text-muted\">Failed to load users.</p>";
    }
}

function showInviteUser() { document.getElementById("invite-modal").style.display = "flex"; }
function closeInviteModal() {
    document.getElementById("invite-modal").style.display = "none";
    document.getElementById("invite-error").classList.add("d-none");
}

async function sendInvite() {
    var email = document.getElementById("invite-email").value.trim();
    var role = document.getElementById("invite-role").value;
    if (!email) return;
    var errEl = document.getElementById("invite-error");
    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/tenants/me/users", {
            method: "POST",
            headers: authHeaders(),
            body: JSON.stringify({ email: email, role: role })
        });
        if (!resp.ok) {
            var data = await resp.json().catch(function() { return {}; });
            throw new Error(data.error || "Failed to invite");
        }
        closeInviteModal();
        loadUsers();
    } catch(err) {
        errEl.textContent = err.message;
        errEl.classList.remove("d-none");
    }
}

// ---- API Keys ----
async function loadApiKeys() {
    var list = document.getElementById("apikeys-list");
    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/tenants/me/api-keys", { headers: authHeaders() });
        if (!resp.ok) throw new Error("Failed");
        var keys = await resp.json();
        if (!keys || keys.length === 0) {
            list.innerHTML = "<p class=\"text-muted\">No API keys. Create one to get started.</p>";
            return;
        }
        list.innerHTML = "<div class=\"list-group\">" + keys.map(function(k) {
            var status = k.revoked_at ? "<span class=\"badge bg-danger\">revoked</span>" : "<span class=\"badge bg-success\">active</span>";
            var revokeBtn = k.revoked_at ? "" : "<button class=\"btn btn-sm btn-outline-danger\" onclick=\"revokeKey(''" + k.id + "'')\">Revoke</button>";
            return "<div class=\"list-group-item d-flex justify-content-between align-items-center\"><div><code>" + escH(k.key_prefix || k.id.substring(0, 8)) + "...</code> " + status + "<br><small class=\"text-muted\">Created " + new Date(k.created_at).toLocaleDateString() + "</small></div>" + revokeBtn + "</div>";
        }).join("") + "</div>";
    } catch(err) {
        list.innerHTML = "<p class=\"text-muted\">Failed to load API keys.</p>";
    }
}

async function createApiKey() {
    var name = prompt("API key name (optional):");
    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/tenants/me/api-keys", {
            method: "POST",
            headers: authHeaders(),
            body: JSON.stringify({ name: name || "default" })
        });
        if (!resp.ok) throw new Error("Failed to create");
        var data = await resp.json();
        // Show the full key once
        if (data.key) {
            document.getElementById("new-key-value").textContent = data.key;
            document.getElementById("new-key-banner").classList.remove("d-none");
        }
        loadApiKeys();
    } catch(err) { alert("Failed to create API key: " + err.message); }
}

function copyNewKey() {
    var key = document.getElementById("new-key-value").textContent;
    navigator.clipboard.writeText(key).then(function() { alert("Copied!"); });
}

async function revokeKey(id) {
    if (!confirm("Revoke this API key? This cannot be undone.")) return;
    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/tenants/me/api-keys/" + id, {
            method: "DELETE",
            headers: authHeaders()
        });
        if (!resp.ok) throw new Error("Failed");
        loadApiKeys();
    } catch(err) { alert("Failed to revoke key"); }
}

// ---- Billing ----
async function loadBilling() {
    try {
        // Load tenant info for current plan
        var tenantResp = await fetch(PLATFORM_URL + "/api/v1/tenants/me", { headers: authHeaders() });
        if (tenantResp.ok) {
            var tenant = await tenantResp.json();
            var plan = tenant.plan || "free";
            document.getElementById("billing-plan-name").textContent = plan.charAt(0).toUpperCase() + plan.slice(1);
        }
        // Load available plans
        var plansResp = await fetch(PLATFORM_URL + "/api/v1/billing/plans");
        if (plansResp.ok) {
            var plans = await plansResp.json();
            var container = document.getElementById("billing-plans-list");
            container.innerHTML = plans.map(function(p) {
                var priceText = p.price_monthly === 0 ? "Free" : "$" + p.price_monthly + "/mo";
                return "<div class=\"col-md-6 col-lg-3\"><div class=\"card h-100\"><div class=\"card-body text-center\"><h6>" + escH(p.name) + "</h6><div class=\"fs-4 fw-bold my-2\">" + priceText + "</div><ul class=\"list-unstyled text-muted small text-start\">" + (p.features || []).map(function(f) { return "<li>&#10003; " + escH(f) + "</li>"; }).join("") + "</ul></div></div></div>";
            }).join("");
        }
    } catch(err) { console.error("Failed to load billing:", err); }
}

// ---- Security ----
async function changePassword(e) {
    e.preventDefault();
    var errEl = document.getElementById("password-error");
    var successEl = document.getElementById("password-success");
    errEl.classList.add("d-none");
    successEl.classList.add("d-none");
    var newPw = document.getElementById("new-password").value;
    var confirmPw = document.getElementById("confirm-password").value;
    if (newPw !== confirmPw) {
        errEl.textContent = "Passwords do not match";
        errEl.classList.remove("d-none");
        return;
    }
    try {
        var resp = await fetch(PLATFORM_URL + "/api/v1/tenants/me/password", {
            method: "PUT",
            headers: authHeaders(),
            body: JSON.stringify({ new_password: newPw })
        });
        if (!resp.ok) {
            var data = await resp.json().catch(function() { return {}; });
            throw new Error(data.error || "Failed to change password");
        }
        successEl.textContent = "Password updated successfully.";
        successEl.classList.remove("d-none");
        document.getElementById("new-password").value = "";
        document.getElementById("confirm-password").value = "";
    } catch(err) {
        errEl.textContent = err.message;
        errEl.classList.remove("d-none");
    }
}

function logoutCurrent() {
    var refreshToken = localStorage.getItem("amos-refresh-token");
    if (refreshToken) {
        fetch(PLATFORM_URL + "/api/v1/auth/logout", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ refresh_token: refreshToken })
        }).catch(function() {});
    }
    localStorage.removeItem("amos-token");
    localStorage.removeItem("amos-refresh-token");
    localStorage.removeItem("amos-tenant");
    localStorage.removeItem("amos-role");
    localStorage.removeItem("amos-user-id");
    localStorage.removeItem("amos-tenant-id");
    window.location.href = "/login";
}

function logoutAll() {
    if (!confirm("Sign out of all sessions? You will need to sign in again.")) return;
    logoutCurrent();
}

function escH(s) { var d = document.createElement("div"); d.textContent = s; return d.innerHTML; }',
    -- CSS
    '.tab-pane { animation: fadeIn 0.15s; }
@keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
.settings-modal-overlay { position: fixed; top: 0; left: 0; width: 100%; height: 100%; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.settings-modal-box { background: var(--bs-body-bg, #fff); border-radius: 8px; padding: 24px; width: 420px; max-width: 95%; max-height: 90vh; overflow-y: auto; box-shadow: 0 8px 32px rgba(0,0,0,0.3); }
.list-group-item { transition: background 0.1s; }
.list-group-item:hover { background: rgba(99, 102, 241, 0.04); }'
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
