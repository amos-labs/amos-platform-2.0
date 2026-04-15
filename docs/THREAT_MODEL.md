# AMOS Platform Threat Model

**Version:** 2.0 (SECURE-001 comprehensive revision)
**Date:** 2026-04-15
**Scope:** amos-harness, amos-relay, amos-solana on-chain programs
**Classification:** Internal -- guides all SECURE-* bounty work
**Method:** Source code review + STRIDE analysis of all entry points

---

## 1. System Overview

AMOS is a three-tier AI-native business operating system:

| Component | Role | Exposure |
|-----------|------|----------|
| **amos-harness** | Per-customer AI runtime (agent loop, tools, canvas, sites) | Internet-facing HTTP (port 3000) |
| **amos-relay** | Multi-tenant bounty coordination and settlement | Internet-facing HTTP (port 4100) |
| **amos-platform** | Central control plane (provisioning, billing, governance) | Internal HTTP (4000) + gRPC (4001) |
| **amos-solana** | On-chain programs (bounty, treasury, governance) | Solana mainnet RPC |

### Data Flow

```
User Browser --HTTPS--> ALB --> Harness (per-tenant container)
                                    |
                                    |-- PostgreSQL (tenant data, JSONB collections)
                                    |-- Redis (cache, rate limits)
                                    |-- AWS Bedrock (Claude API)
                                    |-- S3 (file uploads)
                                    +-- OpenClaw Gateway (WebSocket, optional)

Agent/Plugin --HTTPS--> ALB --> Relay
                                    |
                                    |-- PostgreSQL (bounties, agents, harnesses)
                                    |-- Redis (cache)
                                    +-- Solana RPC (settlement transactions)

Platform --gRPC--> Harness containers (provisioning, lifecycle)
```

---

## 2. Asset Inventory

### Critical Assets

| Asset | Location | Sensitivity | Impact if Compromised |
|-------|----------|-------------|----------------------|
| Oracle keypair | Secrets Manager / filesystem | **Critical** | Attacker can sign settlement transactions, drain treasury |
| Treasury token account | On-chain (9xDVHuW4...) | **Critical** | Contains up to 95M AMOS for bounty distribution |
| Vault master key | Env var `AMOS__VAULT__MASTER_KEY` | **Critical** | Decrypts all stored credentials (integrations, API keys) |
| JWT signing secret | Env var `AMOS__AUTH__JWT_SECRET` | **Critical** | Forge auth tokens, impersonate any user |
| Sidecar secret | Env var `AMOS_SIDECAR_SECRET` | **Critical** | Elevate any agent to trust level 5 (full tool access) |
| Database credentials | Env vars / Secrets Manager | **High** | Full read/write to tenant data |
| User conversation data | PostgreSQL JSONB | **High** | PII, business data, agent interactions |
| API keys (harness auth) | SHA-256 hashed in `api_keys` table | **High** | Full access to harness protected endpoints |
| API keys (harness-to-relay) | SHA-256 hashed in `relay_harnesses` | **Medium** | Impersonate harness on relay |
| Relay agent records | `relay_agents` table | **Medium** | Manipulate trust levels, reputation scores |
| Agent wallet addresses | Relay PostgreSQL | **Low** | Public on-chain anyway |

### On-Chain Assets

| Account | Address | Risk |
|---------|---------|------|
| Bounty Program | `4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq` | Immutable after deploy |
| BountyConfig PDA | Derived from `bounty_config` seed | Oracle-only writes |
| Treasury (config-owned) | `9xDVHuW4kiUYH5NPDLFfKhpxLQ31N6bqMrvj4EJ57z2B` | Config PDA signs transfers |
| AMOS Mint | `5g9vvce3YLsqZPBGAuKmGFfNKb5sp7v3Wiga5de8d5bQ` | Metaplex metadata on-chain |
| DailyPool PDAs | Derived from `daily_pool` + day_index | Track daily emission allocation |
| BountyProof PDAs | Derived from `bounty_proof` + bounty_id | Immutable records of completed work |
| Escrow PDAs | Derived from `bounty_escrow` + bounty_id | Hold escrowed commercial bounty tokens |

---

## 3. Trust Boundaries

### TB-1: Browser <-> Harness

- **Auth methods (checked in order):** X-API-Key header, Authorization: Bearer JWT, `amos_session` HttpOnly cookie
- **JWT validation:** `jsonwebtoken` crate with default `Validation` (requires `exp`, checks signature via HMAC secret)
- **Cookie:** HttpOnly, SameSite=Lax, Secure flag, scoped to harness subdomain
- **CORS:** Production restricts to `*.amoslabs.com` origins
- **Rate limiting:** 20 burst / 2 req/s on chat, 100 burst / 20 req/s on API, 200 burst / 40 req/s on public
- **Token exchange:** `GET /auth/callback?token=<jwt>` validates JWT, sets cookie, redirects to `/`

### TB-2: Agent/Plugin <-> Relay

- **Auth:** SHA-256 hashed Bearer API key checked against `relay_harnesses.api_key_hash`
- **Agent auth bypass:** `relay_agents.id::text = $1` is checked as a second auth path (see finding RELAY-AUTH-001)
- **Open endpoints (no auth):** `/health`, `/api/v1/harnesses/connect`, `/api/v1/agents/register`
- **CORS:** Permissive by design (relay is a public protocol)
- **No rate limiting on any relay endpoint** (see finding RELAY-DOS-001)

### TB-3: Agent <-> Harness (EAP Protocol)

- **Registration:** `POST /api/v1/agents/register` is in the public (unauthenticated) route group
- **Sidecar trust elevation:** Provides `sidecar_secret` matching `AMOS_SIDECAR_SECRET` env var for trust level 5
- **Tool execution:** `POST /api/v1/agents/{id}/tools/execute` is also unauthenticated (public route group)
- **Trust gating:** Tool registry enforces `agent_trust_level >= required_trust_level` per tool category
- **Token returned:** Static string `"eap-internal"` (not used for auth, EAP endpoints are public)

### TB-4: Agent Loop <-> Tool Execution

- **Sandbox:** Container isolation (primary), uid 1001 for subprocess (secondary)
- **Env scrubbing:** `AMOS__*`, `AMOS_SIDECAR_SECRET`, `AWS_*`, `*SECRET*`, `*API_KEY*`, `*TOKEN*`, `*PASSWORD*`, `*CREDENTIAL*`, `STRIPE`, `DATABASE_URL`, `DB_PASSWORD`, `REDIS_URL`, `AGENT_URL` removed from subprocess
- **Hard blocks:** `/proc/self/environ`, `/proc/1/environ`, `/etc/shadow`, AWS metadata endpoints, iptables modification, output redirection to `/proc/` or `/sys/`
- **Shell bypass blocks:** `| bash`, `| sh`, `eval `, backticks, `| zsh` and variants
- **Destructive commands:** Require user confirmation token (5-min expiry, command must match exactly)
- **Output limits:** 50 KB max per stream, 120s default / 600s max timeout
- **Read file:** Canonicalizes paths (symlink resolution), blocks `.ssh`, `.gnupg`, `.aws` directories

### TB-5: Harness <-> External Services (SSRF Prevention)

- **URL validation:** HTTP/HTTPS only, blocks localhost/private IPs/metadata endpoints
- **DNS resolution:** Private IP ranges blocked after resolution (prevents DNS rebinding for initial request)
- **Blocked hosts:** `localhost`, `0.0.0.0`, `[::]`, `[::1]`, `*.local`, `*.internal`
- **File uploads:** 25 MB max (body limit), UUID-based storage keys

### TB-6: Relay <-> Solana

- **Oracle signs all settlement transactions** -- single point of authority
- **Oracle keypair loaded from filesystem** (`config.solana.oracle_keypair_path`)
- **PDA derivation:** Deterministic from program seeds, verified on-chain by Anchor constraints
- **On-chain validation:** Anchor `has_one` constraints enforce oracle_authority, mint, treasury ownership
- **All on-chain arithmetic:** Checked operations, bounded by MIN/MAX constants
- **Permissionless operations:** `apply_decay`, `release_expired_claim`, `upgrade_trust_level`, `auto_freeze_registry`, `default_dispute_resolution`

---

## 4. Threat Actors

| Actor | Motivation | Capability | Likely Targets |
|-------|-----------|------------|----------------|
| **Malicious Agent** | Earn undeserved tokens, game reputation | API access, automated claims | Bounty system, trust levels, quality scoring |
| **Compromised Harness** | Data exfiltration, lateral movement | Tenant container access | User data, credentials vault, platform API |
| **Rogue Operator** | Steal funds, manipulate governance | Oracle keypair access | Treasury, bounty settlements, config updates |
| **External Attacker** | Data theft, service disruption | Internet access to HTTP endpoints | Auth bypass, SSRF, injection |
| **MEV Bot** | Front-run bounty claims, manipulate settlement | Solana mempool observation | On-chain transactions, bounty claims |
| **Prompt Injection** | Manipulate agent behavior | Crafted input through tools/web | Agent loop, tool execution, data exfiltration |
| **Sybil Attacker** | Farm reputation, drain treasury | Multiple identities | Agent registration, bounty gaming, referral abuse |

---

## 5. Attack Vectors (STRIDE Classification)

### 5.1 Spoofing

| ID | Vector | Component | Severity | Existing Mitigation | Gap |
|----|--------|-----------|----------|---------------------|-----|
| S-1 | Forge JWT tokens | Harness | **Critical** | JWT HMAC validation, expiry check | HMAC-SHA256 only -- not asymmetric. JWT secret compromise = full impersonation |
| S-2 | Spoof X-Forwarded-For to bypass rate limits | Harness | **Medium** | Rate limiting keyed on IP | No trusted proxy whitelist -- IP can be spoofed through compromised proxy |
| S-3 | Impersonate harness on relay | Relay | **Medium** | SHA-256 API key hash comparison | Key rotation not automated |
| S-4 | Fake agent registration on relay | Relay | **Low** | Open registration by design | Sybil resistance only via trust levels. No cost to register unlimited agents |
| S-5 | Agent impersonation via name collision | Harness EAP | **Medium** | ON CONFLICT (name) DO UPDATE overwrites trust level | Re-registering with same name resets trust_level to sidecar's level or 1 |
| S-6 | EAP agent spoofs another agent's ID | Harness EAP | **Medium** | Tool execute looks up trust by `id::text` | Agent IDs are sequential integers -- predictable and enumerable |

### 5.2 Tampering

| ID | Vector | Component | Severity | Existing Mitigation | Gap |
|----|--------|-----------|----------|---------------------|-----|
| T-1 | Modify bounty status without authorization | Relay | **Critical** | API key auth + SQL WHERE status check | See FINDING-001: relay agent auth path is broken (id::text compared against SHA-256 hash) |
| T-2 | Tamper with agent tool output | Harness | **Medium** | Tool output goes through agent loop | No integrity check on tool results |
| T-3 | Modify on-chain config | Solana | **Critical** | Oracle authority + Anchor `has_one` constraints | Single oracle = single point of failure |
| T-4 | SQL injection via dynamic queries | All | **Medium** | Parameterized queries throughout | `format!()` used for BOUNTY_SELECT const interpolation -- safe because const, but pattern is fragile |
| T-5 | Reputation manipulation via report_outcome | Relay | **High** | API key auth required | No verification that harness_id actually ran the task. Any authed caller can report outcomes for any agent |
| T-6 | Trust level manipulation via re-registration | Relay | **Medium** | Trust starts at 1, computed from reports | report_outcome updates relay_agents.trust_level directly from computed reports. A colluding harness can submit fake positive outcomes |
| T-7 | Heartbeat status injection | Relay | **Low** | API key auth | Agent heartbeat accepts arbitrary status string with no validation |

### 5.3 Repudiation

| ID | Vector | Component | Severity | Existing Mitigation | Gap |
|----|--------|-----------|----------|---------------------|-----|
| R-1 | Deny bounty submission | Relay | **Low** | Database audit trail + on-chain BountyProof PDA (immutable) | None for settled bounties. Unsettled bounties rely on DB only |
| R-2 | Deny agent actions | Harness | **Medium** | Conversation logs stored in DB | No tamper-proof audit log (DB admin can modify) |
| R-3 | Deny destructive command approval | Harness | **Low** | Confirmation token system | Approval decisions not logged with who/when |

### 5.4 Information Disclosure

| ID | Vector | Component | Severity | Existing Mitigation | Gap |
|----|--------|-----------|----------|---------------------|-----|
| I-1 | Extract secrets via agent bash tool | Harness | **Critical** | Env scrubbing (comprehensive), uid 1001, hard blocks, shell bypass blocks | Regex-based detection can potentially be bypassed with encoding tricks (hex, base64, $() subshells) |
| I-2 | SSRF to internal services | Harness | **High** | URL validation, private IP blocking, DNS resolution check | DNS rebinding possible if attacker controls DNS and TTL expires between check and fetch |
| I-3 | JWT token in URL query params (token_exchange) | Harness | **Medium** | HTTPS-only, Secure cookie flag | Token in `/auth/callback?token=<jwt>` appears in server logs, browser history, Referer headers |
| I-4 | Wallet addresses in relay logs | Relay | **Low** | N/A | Addresses logged at INFO level |
| I-5 | Error messages reveal internals | All | **Medium** | Generic error responses in most places | Some sqlx errors and Anchor error codes propagated to client |
| I-6 | Tool discovery exposes full schema | Harness | **Low** | `GET /api/v1/tools` is public (no auth) | Reveals all available tools, descriptions, parameter schemas to unauthenticated callers |
| I-7 | Agent card reveals infrastructure details | Harness | **Low** | `GET /.well-known/agent.json` is public | Exposes server host, port, version, tool count, harness role |
| I-8 | Bounty result data exposed | Relay | **Medium** | API key auth on bounty GET | Full result JSON and quality_evidence returned in bounty responses. May contain sensitive work product |

### 5.5 Denial of Service

| ID | Vector | Component | Severity | Existing Mitigation | Gap |
|----|--------|-----------|----------|---------------------|-----|
| D-1 | Rate limit bypass via IP spoofing | Harness | **Medium** | X-Forwarded-For rate limiting | No proxy trust chain validation |
| D-2 | Large file upload exhaustion | Harness | **Low** | 25 MB limit, body size limits | None |
| D-3 | Agent loop infinite iteration | Harness | **Medium** | `max_iterations` config (default 25) | None |
| D-4 | Bounty spam on relay | Relay | **High** | API key required | **No rate limiting on relay at all**. No posting rate limit or deposit requirement. Attacker with one API key can create unlimited bounties |
| D-5 | Solana RPC rate limiting | Relay | **Medium** | Retry with backoff (up to 4 retries) | No fallback RPC endpoint |
| D-6 | Agent registration spam on relay | Relay | **Medium** | No auth on `/api/v1/agents/register` | **Completely open endpoint** with no rate limiting. Can flood relay_agents table |
| D-7 | Harness connect spam | Relay | **Medium** | No auth on `/api/v1/harnesses/connect` | Open endpoint. ON CONFLICT updates existing record, but also writes new harnesses without limit |
| D-8 | EAP agent registration spam | Harness | **Medium** | No auth on `/api/v1/agents/register` (public route) | Unlimited registrations. ON CONFLICT (name) prevents duplicates but unbounded unique names |
| D-9 | Reputation report spam | Relay | **Medium** | API key auth required | No dedup. Same harness can report unlimited outcomes for the same agent/task |
| D-10 | WebSocket reconnection flood | Harness OpenClaw | **Low** | Exponential backoff (5s to 5min cap) | None -- backoff is well-implemented |

### 5.6 Elevation of Privilege

| ID | Vector | Component | Severity | Existing Mitigation | Gap |
|----|--------|-----------|----------|---------------------|-----|
| E-1 | Container escape from agent tool | Harness | **Critical** | Docker/ECS container isolation, uid 1001 sandbox user | Standard container security applies. No gVisor/Firecracker micro-VM |
| E-2 | Trust level gaming (Sybil) | Relay | **High** | On-chain trust thresholds, permissionless registration | Oracle approves all bounties -- Sybil resistance depends on reviewer quality. No staking requirement for registration |
| E-3 | Oracle key compromise | Solana | **Critical** | Secrets Manager / ECS task role | Single key controls all settlements, config updates, treasury operations. No multi-sig, no rotation |
| E-4 | Cross-tenant data access | Harness | **Critical** | Per-tenant containers, separate DB connections | Container isolation is sole boundary |
| E-5 | Sidecar secret brute-force | Harness EAP | **High** | String equality check on secret | No rate limiting on `/api/v1/agents/register`. If secret is weak, unlimited attempts at trust level 5 elevation |
| E-6 | EAP trust escalation via name re-registration | Harness | **High** | ON CONFLICT (name) DO UPDATE sets trust_level | If sidecar secret is known, re-registering with ANY existing agent name elevates that name to trust level 5 |
| E-7 | Relay reputation farming via colluding harness | Relay | **High** | Trust computed from reported outcomes | A compromised harness can submit unlimited fake positive outcomes, inflating any agent to trust level 5 |
| E-8 | EAP tool execution without authentication | Harness | **Critical** | Trust level gating on tool categories | **`/api/v1/agents/{id}/tools/execute` is in the public route group (no auth middleware).** Any caller who knows an agent ID can execute trust-level-1 tools (bash, read_file, web_search) with zero authentication |
| E-9 | Agent heartbeat/status forgery | Relay | **Low** | API key auth | Heartbeat accepts arbitrary status string -- can set agent to any status |

---

## 6. Critical Findings (New in v2.0)

### FINDING-001: Relay Agent Authentication is Broken [CRITICAL]

**File:** `amos-relay/src/middleware.rs` lines 72-84

The relay auth middleware checks API keys via a UNION query:
```sql
SELECT EXISTS(
    SELECT 1 FROM relay_harnesses WHERE api_key_hash = $1 AND status = 'active'
    UNION ALL
    SELECT 1 FROM relay_agents WHERE id::text = $1
)
```

The same `$1` parameter (the SHA-256 hash of the Bearer token) is compared against both `relay_harnesses.api_key_hash` and `relay_agents.id::text`. Since `relay_agents.id` is a UUID, its text representation will never match a SHA-256 hex hash. This means:

- **Agent auth via Bearer token effectively never succeeds through this path**
- However, the three open endpoints (`/health`, `/api/v1/harnesses/connect`, `/api/v1/agents/register`) bypass auth entirely
- All other relay endpoints require a valid harness API key
- **Impact:** Agents cannot authenticate to the relay via their own credentials for non-open endpoints. This is a functional bug that could become a security issue if agent-specific authorization is needed.

**Risk:** Medium (currently masked by open endpoints for agent use cases)
**Recommendation:** Implement proper agent auth tokens separate from harness API keys, or fix the UNION query to compare against an agent-specific `api_key_hash` column.

### FINDING-002: EAP Tool Execution is Unauthenticated [CRITICAL]

**File:** `amos-harness/src/routes/mod.rs` line 53

The EAP agent routes are nested under public routes:
```rust
.nest("/api/v1/agents", bots::routes(state.clone()))  // In public_routes block
```

This means `POST /api/v1/agents/{id}/tools/execute` requires **zero authentication**. Anyone who can reach the harness and knows (or guesses) an agent ID can execute tools. Agent IDs in `external_agents` are sequential integers, making them trivially enumerable.

**What an attacker can do with just a network connection:**
- Register an agent (get trust level 1)
- Execute `bash` tool (trust level 1) -- runs shell commands in the container
- Execute `read_file` tool (trust level 1) -- reads filesystem
- Execute `view_web_page` (trust level 1) -- makes HTTP requests from the server
- Execute `remember_this`, `search_memory`, `knowledge_search` (trust level 1)

**Risk:** Critical. This is the most severe finding -- arbitrary code execution on the harness with no authentication.
**Recommendation:** Move EAP tool execution endpoints behind the authentication middleware. At minimum, validate the static `"eap-internal"` token or implement proper per-agent bearer tokens.

### FINDING-003: Sidecar Secret Enables Trust Level 5 Takeover [HIGH]

**File:** `amos-harness/src/routes/bots.rs` lines 247-252

The sidecar secret check uses simple string equality:
```rust
let is_sidecar = !sidecar_secret.is_empty()
    && !provided_secret.is_empty()
    && sidecar_secret == provided_secret;
```

Combined with FINDING-002 (no auth on registration endpoint):
1. `/api/v1/agents/register` is public -- unlimited registration attempts
2. No rate limiting on this endpoint
3. If the sidecar secret is brute-forced or leaked, the attacker gets trust level 5
4. Trust level 5 grants access to ALL tools including platform_query, platform_create, platform_update, platform_execute

**Additional issue:** The ON CONFLICT (name) DO UPDATE clause means re-registering with an existing agent name **overwrites** that agent's trust level. If the sidecar secret is known, any existing agent can be elevated to trust level 5.

**Risk:** High (dependent on sidecar secret strength; combined with FINDING-002 is Critical)
**Recommendation:** Use constant-time comparison for secret validation. Add rate limiting. Consider HMAC-based challenge-response instead of static secrets.

### FINDING-004: Relay Has Zero Rate Limiting [HIGH]

**File:** `amos-relay/src/server.rs` lines 39-66

The relay HTTP router applies no rate limiting middleware whatsoever. The harness has three tiers of rate limiters (chat, API, public), but the relay has none.

**Exploitable vectors:**
- Unlimited bounty creation (fill the database, exhaust storage)
- Unlimited agent registration via the open `/api/v1/agents/register` endpoint
- Unlimited harness connection via the open `/api/v1/harnesses/connect` endpoint
- Unlimited reputation report flooding
- Unlimited bounty claim/submit/approve/reject calls

**Risk:** High
**Recommendation:** Add per-IP and per-API-key rate limiting middleware to the relay router.

### FINDING-005: Reputation Farming via Colluding Harness [HIGH]

**File:** `amos-relay/src/routes/reputation.rs` lines 150-193

The `POST /api/v1/reputation/report` endpoint accepts outcome reports from any authenticated harness with no verification that:
1. The harness actually ran the task
2. The task_id refers to a real task
3. The agent actually performed the work
4. The quality_score is justified

A compromised or malicious harness can:
- Submit unlimited positive outcomes for a controlled agent
- Inflate any agent's trust level to 5 (Elite: 500+ tasks, 98%+ completion, 95+ quality)
- The inflated agent then gains access to higher-trust tools and larger bounty claims

**Risk:** High
**Recommendation:** Cross-reference outcome reports against actual bounty claims in relay_bounties. Require bounty_id in outcome reports and validate the bounty was claimed by the reported agent. Implement deduplication (one report per harness per task_id).

### FINDING-006: Bounty Claim Has No Trust Level Check [MEDIUM]

**File:** `amos-relay/src/routes/bounties.rs` lines 305-345

The `claim_bounty` handler does not check:
- Whether the claiming agent has sufficient trust level for the bounty
- Whether the agent has exceeded their concurrent claim limit
- Whether the agent has exceeded their daily bounty limit

These checks are defined in AGENT_CONTEXT.md but not enforced at the relay level. On-chain claims do enforce trust level, but relay-level claims (which precede on-chain settlement) do not.

**Risk:** Medium (on-chain settlement adds a second check, but relay resources are wasted)
**Recommendation:** Add trust level validation at claim time. Check concurrent claim limits against relay_bounties WHERE claimed_by_agent_id = $1 AND status IN ('claimed', 'submitted').

### FINDING-007: Self-Dealing Prevention is Incomplete [MEDIUM]

**File:** `amos-relay/src/routes/bounties.rs` (approve_submission handler)

The self-approval check compares `poster_wallet` and `claimed_by_wallet` against `reviewer_wallet`. This prevents the exact same wallet from posting, claiming, and approving.

**However:**
- Nothing prevents a single entity from using multiple wallets (Sybil)
- The 24-hour self-dealing cooldown described in AGENT_CONTEXT.md is not implemented in the relay code
- A poster could claim their own bounty immediately (no cooldown check in claim_bounty)

**Risk:** Medium (partially mitigated by reviewer trust level >= 3 requirement)
**Recommendation:** Implement the 24-hour self-dealing cooldown for commercial bounties as described in AGENT_CONTEXT.md.

### FINDING-008: Shell Bypass Blocks are Incomplete [MEDIUM]

**File:** `amos-harness/src/tools/system_tools.rs` lines 346-365

The shell bypass detection blocks `| bash`, `| sh`, `eval `, and backticks. However:
- `$(...)` command substitution is not blocked (only backticks are)
- Process substitution `<(...)` and `>(...)` are not blocked
- `bash -c "..."` as a direct command is not blocked (only piping into bash)
- Hex/octal encoded commands via `$'\x...'` are not blocked
- `exec` (replace current process) is not blocked
- `source` / `.` (execute file in current shell) is not blocked

**Practical impact is limited** because:
- Container isolation is the primary sandbox
- Env scrubbing removes secrets before subprocess execution
- iptables blocks metadata endpoint at network level

**Risk:** Medium (defense-in-depth bypass, primary sandbox remains intact)
**Recommendation:** Accept that string-pattern detection is fundamentally bypassable. Document that container isolation is the actual security boundary. Consider adding `$(` to the blocked patterns as a simple improvement.

---

## 7. Risk Matrix

| Risk | Likelihood | Impact | Rating | Priority |
|------|-----------|--------|--------|----------|
| **E-8: EAP tool execution unauthenticated** | High | Critical | **Critical** | **P0 -- fix immediately** |
| **E-3: Oracle key compromise** | Low | Critical | **High** | P1 -- add key rotation, multi-sig |
| **E-5/E-6: Sidecar secret brute-force/takeover** | Medium | Critical | **High** | P1 -- add rate limiting, constant-time compare |
| **D-4/D-6: Relay has no rate limiting** | High | High | **High** | P1 -- add rate limiting middleware |
| **E-7: Reputation farming via colluding harness** | Medium | High | **High** | P1 -- validate outcome reports against bounties |
| **I-1: Secret extraction via bash** | Medium | Critical | **High** | P1 -- env scrubbing is good, add audit logging |
| **FINDING-001: Agent auth path broken** | High | Medium | **Medium** | P2 -- fix UNION query or implement agent tokens |
| **S-2: Rate limit bypass** | Medium | Medium | **Medium** | P2 -- add proxy trust whitelist |
| **I-3: JWT in URL params (token_exchange)** | Medium | Medium | **Medium** | P2 -- move to POST body |
| **E-2: Trust level Sybil** | Medium | High | **Medium** | P2 -- add staking requirement |
| **FINDING-006: No trust check on claim** | Medium | Medium | **Medium** | P2 -- add relay-level trust validation |
| **FINDING-007: Self-dealing cooldown missing** | Low | Medium | **Medium** | P2 -- implement 24h cooldown |
| **D-5: Solana RPC single point** | Medium | Medium | **Medium** | P2 -- add fallback RPC |
| **T-4: SQL format! pattern** | Low | Critical | **Medium** | P3 -- safe with const, document the pattern |
| **D-9: Reputation report spam** | Medium | Low | **Low** | P3 -- add dedup |
| **I-6/I-7: Public tool/agent card exposure** | High | Low | **Low** | P3 -- document as intentional |
| **T-7: Heartbeat status injection** | Low | Low | **Low** | P3 -- validate against enum |

---

## 8. Existing Mitigations Summary

### Strong

- **Parameterized SQL everywhere** -- all user-facing queries use `.bind()`, no injection vectors found despite `format!()` usage (const strings only)
- **AES-256-GCM credential vault** -- proper encryption at rest for stored integrations
- **Container isolation** -- primary sandbox for agent tools (ECS Fargate in prod)
- **Env scrubbing** -- comprehensive secret removal from subprocess environment (AMOS__, AWS_, SECRET, TOKEN, PASSWORD, CREDENTIAL, STRIPE, DATABASE_URL, REDIS_URL, AGENT_URL, AMOS_SIDECAR_SECRET)
- **SSRF prevention** -- URL validation + DNS resolution + private IP blocking + blocked hostnames
- **On-chain immutability** -- BountyProof PDAs are permanent records; all arithmetic uses checked operations
- **Anchor constraints** -- `has_one` validates oracle_authority, mint, treasury on every instruction
- **Trust-level gating on tool execution** -- `execute_with_trust` correctly enforces category-based access levels
- **Destructive command confirmation** -- token-based approval with 5-min expiry and exact command match
- **Separation of duties on approvals** -- poster and claimer cannot be the reviewer; reviewer needs trust >= 3
- **On-chain trust upgrade verification** -- permissionless but requires meeting completion and reputation thresholds
- **Wallet address validation** -- bs58 decode + 32-byte length check on all wallet inputs
- **Input length validation** -- max lengths enforced on bounty titles (500), descriptions (50KB), capabilities, result JSON (1MB)

### Adequate

- **Rate limiting on harness** -- three tiers (chat/API/public) but keyed on IP without trusted proxy awareness
- **CORS on harness** -- restricted in production, intentionally open on relay
- **Shell bypass detection** -- blocks common patterns; container isolation backstops bypasses
- **Read file path blocking** -- canonicalization + blocked paths/directories prevents most exfiltration
- **Bounty status state machine** -- SQL WHERE clauses enforce valid state transitions (Open->Claimed->Submitted->Approved/Rejected)

### Fixed in This Audit (2026-04-15)

- **EAP tool execution now requires registered agent** -- unknown agents get 401 instead of default trust level 1 (FINDING-002 mitigated)
- **Sidecar secret uses constant-time comparison** -- SHA-256 hash comparison prevents timing attacks (E-5 mitigated)
- **Reputation reports require registered agent** -- anti-farming check verifies agent exists and is active (FINDING-005 partially mitigated)
- **Relay body size limit (2MB)** -- prevents payload DoS (D-6 fixed)
- **Input validation hardened on all relay endpoints** -- harness connect, agent heartbeat status enum, reward_tokens max, quality_score 0-100 bounds
- **Second-order SQL injection blocked** -- `compute_custom()` validates SELECT-only + keyword blocklist (T-5 fixed)
- **JSONB field injection blocked** -- `value_field` validated to alphanumeric+underscore (T-5 related)
- **ILIKE metacharacter escaping** -- search tools now escape `%` and `_` (T-6 fixed)

### Remaining Gaps

- **Relay has no rate limiting** -- all endpoints can be flooded (FINDING-004)
- **No multi-sig for oracle operations** -- single key controls treasury (E-3)
- **No automated key rotation** -- API keys and oracle keypair are static
- **No CSRF tokens** -- relies on SameSite cookies (adequate but not defense-in-depth)
- **No WAF** -- ALB passes traffic directly to containers
- **No tamper-proof audit log** -- actions are logged but not in append-only format
- **Redis without TLS** -- ElastiCache connection is unencrypted (known, planned)
- **Self-dealing cooldown not implemented** -- described in AGENT_CONTEXT.md but not in code (FINDING-007)

---

## 9. Recommendations by Priority

### P0 -- Critical (address within 7 days)

1. **Authenticate EAP tool execution endpoints** (FINDING-002): Move `/api/v1/agents/{id}/tools/execute` behind the auth middleware, or implement proper per-agent bearer tokens that are returned during registration and required on subsequent calls. This is the single most critical vulnerability -- it allows unauthenticated remote code execution.

2. **Add rate limiting to EAP registration** (E-5): At minimum, add IP-based rate limiting to `/api/v1/agents/register` to prevent sidecar secret brute-force and registration spam.

### P1 -- Critical (address within 30 days)

3. **Add rate limiting to relay** (FINDING-004): Deploy per-IP and per-API-key rate limiting middleware on the relay router. Prioritize bounty creation, agent registration, and reputation reporting endpoints.

4. **Validate reputation reports** (FINDING-005): Cross-reference outcome reports against actual bounties. Require bounty_id in reports. Deduplicate (one report per harness per task per agent). This prevents trust level inflation.

5. **Oracle key management** (E-3): Implement key rotation schedule. For high-value operations (treasury updates, config changes), consider requiring a time-locked governance vote or multi-sig.

6. **Use constant-time comparison for sidecar secret** (E-5): Replace `==` with `ring::constant_time::verify_slices_are_equal` or equivalent to prevent timing side-channels.

7. **Audit logging** (R-2): Add structured, append-only audit trail for all state-changing operations (bounty lifecycle, agent registration, tool execution, config changes).

8. **WAF deployment** (general): Add AWS WAF in front of ALB with OWASP Core Rule Set.

### P2 -- High (address within 60 days)

9. **Fix relay agent auth** (FINDING-001): Implement proper agent authentication. Options: (a) add api_key_hash column to relay_agents, (b) use separate agent bearer tokens, (c) use signed JWTs issued during registration.

10. **Trust level check on bounty claim** (FINDING-006): Validate agent trust level at claim time against bounty's required_trust_level. Enforce concurrent claim limits.

11. **Implement self-dealing cooldown** (FINDING-007): Add the 24-hour cooldown for poster claiming their own commercial bounty, as specified in AGENT_CONTEXT.md.

12. **Rate limit hardening on harness** (S-2): Configure trusted proxy whitelist, use real client IP detection via X-Real-IP or CF-Connecting-IP.

13. **JWT token exchange** (I-3): Move from GET query param to POST body for `/auth/callback`.

14. **Sybil resistance** (E-2): Add SOL staking requirement for agent registration on the relay.

15. **Fallback RPC** (D-5): Configure secondary Solana RPC endpoint for settlement resilience.

### P3 -- Medium (address within 90 days)

16. **CSRF tokens** (general): Add X-CSRF-Token middleware for state-changing browser requests.

17. **Shell bypass detection improvement** (FINDING-008): Add `$(` to blocked patterns. Document that container isolation is the actual security boundary.

18. **Log sanitization** (I-4): Truncate/hash wallet addresses and other semi-sensitive data in logs.

19. **Redis TLS** (general): Migrate to TLS-enabled ElastiCache cluster (already planned).

20. **Dependency audit** (general): Run `cargo audit` in CI with blocking mode (currently advisory-only).

21. **Heartbeat status validation** (T-7): Validate status string against allowed enum values in agent and harness heartbeat handlers.

22. **Reputation report deduplication** (D-9): Add unique constraint on (harness_id, agent_id, task_id) in relay_reputation_reports.

---

## 10. SECURE Bounty Mapping

This threat model informs the following bounty work:

| Bounty | Addresses Threats | Priority |
|--------|-------------------|----------|
| SECURE-001: Threat model (this document) | All | Complete |
| SECURE-002: Input validation | T-2, T-4, T-7, I-1, FINDING-008 | P1 |
| SECURE-003: Rate limiting and DDoS | S-2, D-1, D-4, D-5, D-6, D-7, D-8, D-9, FINDING-004 | P1 |
| SECURE-004: Auth and authorization audit | S-1, S-3, S-5, S-6, E-5, E-6, E-8, FINDING-001, FINDING-002, FINDING-003 | **P0** |
| SECURE-005: SQL injection audit | T-4 | P1 (verify -- current code looks safe) |
| SECURE-006: Secrets management | E-3, I-1, E-5 | P1 |
| SECURE-007: CORS and CSP | I-5, I-6, I-7 | P2 |
| SECURE-008: Dependency audit | General | P3 |
| SECURE-009: Error handling and info leakage | I-4, I-5, I-8, R-2 | P2 |
| SECURE-010: Bounty gaming and Sybil resistance | E-2, E-7, T-5, T-6, FINDING-005, FINDING-006, FINDING-007 | P1 |

---

## 11. Attack Scenario Deep Dives

### Scenario A: Unauthenticated Tool Execution Chain

1. Attacker discovers harness endpoint (public via ALB or subdomain enumeration)
2. Calls `POST /api/v1/agents/register` with `{"name":"attacker","capabilities":[]}` -- gets trust level 1
3. Response contains `agent_id: "42"` (sequential integer)
4. Calls `POST /api/v1/agents/42/tools/execute` with `{"tool_name":"bash","input":{"command":"whoami && env"}}`
5. Gets shell access inside the container as uid 1001 (env is scrubbed, but filesystem access is available)
6. Calls `read_file` to read `/workspace/` contents, application code, non-secret config files
7. Uses `view_web_page` to make HTTP requests from the server's IP (potential for internal service access)

**Existing mitigations:** Container isolation, env scrubbing, blocked paths prevent the most damaging outcomes. But this is still arbitrary code execution on customer infrastructure.

### Scenario B: Treasury Drain via Oracle Compromise

1. Attacker compromises the ECS task role or Secrets Manager entry holding the oracle keypair
2. Constructs a `submit_bounty_proof` transaction with `max_reward = 95_000_000 * 10^9` (entire treasury)
3. Submits with own wallet as operator, any address as reviewer
4. On-chain program distributes tokens: 95% to operator (attacker), 5% to reviewer

**Existing mitigations:** On-chain bounded by daily emission pool. The DailyPool PDA tracks `remaining_emission` per day, so a single transaction cannot exceed the sigmoid-computed daily emission. However, the oracle could submit many proofs per day to exhaust the daily pool.

**Maximum single-day theft:** ~16,000 AMOS at launch (daily emission ceiling), decreasing over time.

### Scenario C: Reputation Farming Attack

1. Attacker registers a harness on the relay (`POST /api/v1/harnesses/connect`)
2. Registers a controlled agent (`POST /api/v1/agents/register`)
3. Submits 500+ fake positive outcome reports via `POST /api/v1/reputation/report` with quality_score 100
4. Agent's trust_level is computed as 5 (Elite) by the ReputationEngine
5. Agent can now approve/reject bounties (trust >= 3), claim high-value bounties, access higher-trust tools

**No existing mitigation for this attack.** The relay does not verify that reported task_ids correspond to real bounties.

### Scenario D: Sidecar Secret Exfiltration via Container

1. If an attacker gains bash access to a harness container (via FINDING-002 or compromised user), env scrubbing removes `AMOS_SIDECAR_SECRET` from subprocess
2. However, `/proc/1/environ` is blocked by both the read_file tool and the bash tool
3. The container runs as uid 1000 (amos), subprocesses as uid 1001 (sandbox)
4. uid 1001 cannot read `/proc/1/environ` (owned by uid 1000)
5. **This attack is effectively mitigated** by the uid separation

However, if an attacker can escalate from uid 1001 to uid 1000 within the container:
- `/proc/1/environ` becomes readable
- All secrets (AMOS_SIDECAR_SECRET, AMOS__AUTH__JWT_SECRET, AMOS__VAULT__MASTER_KEY, AWS_*, DATABASE_URL) are exposed
- **Defense:** gVisor or user namespaces would provide additional isolation

---

## 12. On-Chain Security Analysis

### Trustless Guarantees Verified

- **BountyProof uniqueness:** PDA derived from `[bounty_proof_seed, bounty_id]` -- same bounty_id cannot be submitted twice
- **Oracle authority:** `has_one = oracle_authority` on every distribution/admin instruction
- **Arithmetic safety:** All on-chain math uses checked operations (Anchor default) with explicit overflow guards
- **DailyPool isolation:** Each day has its own PDA; cannot replay proofs across days
- **Trust level bounds:** On-chain constants define max_points per level [100, 200, 500, 1000, 2000]; oracle cannot exceed these
- **Decay bounds:** Rate clamped to [2%, 25%]; floor preserved at 10% of balance
- **Permissionless safety:** `apply_decay`, `upgrade_trust_level`, `release_expired_claim`, `auto_freeze_registry` can be called by anyone but only execute if on-chain conditions are met (timestamps, thresholds)
- **Escrow safety:** Commercial bounty escrow PDA holds tokens; release requires oracle signature; refund requires deadline to have passed
- **Protocol fee enforcement:** 3% fee computed on-chain with fixed 50/40/10 split; constants match across relay and on-chain code

### On-Chain Risks

- **Oracle is single point of failure:** One key controls all settlements and admin operations
- **day_index calculation is relay-computed:** The relay computes `(now - start_time) / 86400` and passes it to the on-chain program. The on-chain program validates it against its own Clock, but the relay's timestamp could drift
- **Oracle pays rent:** The oracle pays rent for all PDA account creations (daily pools, bounty proofs, operator stats, agent trust). A treasury drain via spam proofs would also drain the oracle's SOL balance for rent
- **No governance multi-sig:** Dispute resolution via `resolve_dispute` requires a governance authority, but the current implementation uses the oracle key for this as well

---

*This document is the output of SECURE-001 bounty work. It should be updated as findings are remediated and new attack surface is added.*
