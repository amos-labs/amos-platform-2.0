# Bug Report — Harness ↔ Relay End-to-End Integration

**Submission for:** `AMOS-ONBOARD-003` (bounty `ea3466b2-0cd8-4da9-8615-0d4776f4fcfb`, 500 AMOS)
**Date:** 2026-04-30
**Discovered by:** AMOS harness agent attempting to claim a bounty live, surfaced by founder during dogfood test
**Severity tier requested:** **Critical** (Finding #5 alone qualifies; the other five compound)

---

## How these were found

A harness-resident AMOS agent attempted the standard **Discover → Assess → Claim** path against the live mainnet relay. Each step exposed a distinct integration gap. The findings below are the agent's own diagnostic output, formatted into a structured bug report.

The full session is a self-referential demonstration: the protocol learned about its own bugs by attempting to use itself, exactly the proof-of-concept the autonomous loop is designed to deliver.

---

## Finding #1 — `assess_bounty_fit` uses brittle literal capability matching (Major)

**Symptom:** Agent registered with 7 capabilities including `documentation`. Bounty required `content_generation`. `assess_bounty_fit` returned `fit_score: 0.0` and verdict `poor` despite `documentation` and `content_generation` being functional synonyms for this bounty.

**Reproduction:**
1. Register an agent with capabilities `["documentation", ...]`.
2. Discover a bounty whose `required_capabilities` is `["content_generation"]`.
3. Call `assess_bounty_fit` on that pair.
4. Observe: score 0.0, verdict `poor`.

**Expected:** Semantic / synonym matching of capability strings. Agent declaring `documentation` should match a bounty asking for `content_generation`.

**Actual:** Literal string equality.

**Suggested fix:** Three options, ascending in cost:
- **(a)** Static synonym table maintained in the harness (`documentation ⇔ content_generation ⇔ writing`).
- **(b)** LLM-backed semantic match (call the harness's existing Bedrock client with both lists; cheap because per-claim, not per-tick).
- **(c)** Richer capability declarations on registration (free-text + structured tags).

(a) is shippable in an hour and clears the most common false negatives.

---

## Finding #2 — `claim_bounty` returns 401 because harness agents aren't synced to the relay (Critical)

**Symptom:** `relay claim returned 401 Unauthorized` (with empty body — see Finding #3).

**Root cause:** The harness has its own local `openclaw_agents` table; the relay has its own `relay_agents` table. **They're not connected.** Local `agent_id=1` is meaningless to the relay. The claim handler on the relay can't find a registered agent matching the request, returns 401.

**Why this is a complete blocker:** No harness-spawned agent can ever claim a bounty without an out-of-band manual registration step. The whole "user's harness becomes the intake interface and execution surface for the protocol" design fails at the first claim.

**Suggested fix:** Auto-register on first claim. The `claim_bounty` harness tool should:
1. Detect the agent is not yet registered with the relay (no cached relay UUID).
2. POST `/api/v1/agents` with the agent's wallet, capabilities, harness_id.
3. Cache the returned relay UUID locally.
4. Retry the claim with the correct `agent_id`.

Agents shouldn't have to know about relay-side identity to participate. Identity is plumbing.

---

## Finding #3 — 401 response has no body (Minor)

**Symptom:** `Relay claim returned 401 Unauthorized:` — no JSON body, no diagnostic text. Agent has no way to know whether it's missing a header, an unregistered wallet, or a malformed payload.

**Suggested fix:** Relay's auth-failure handler should return a JSON body identifying *which* check failed. e.g.:
```json
{ "error": "agent_not_registered", "hint": "POST /api/v1/agents first; agent_id from request not found in relay_agents", "status": 401 }
```

Pairs naturally with Finding #2's auto-register flow — a clear `agent_not_registered` code lets the harness tool know exactly when to attempt registration.

---

## Finding #4 — Network egress is constrained but inconsistent (Major)

**Symptom:**
- `harness_view_webpage` blocks `localhost` (sensible — prevents SSRF inside the harness host).
- `harness_bash` *sometimes* successfully reaches `localhost:4100` (the local relay).

**Root cause:** Two different egress policies on two different tools. `view_webpage` is a managed HTTP client with explicit URL allowlisting; `bash` runs arbitrary subprocess via the shell, which has no such filter.

**Suggested fix:** Pick one policy and apply it consistently. For trusted-development mode, allow both. For prod-customer mode, deny both. The current state — a tool that's blocked + a tool that's not — is the worst of both because it lulls the agent into thinking egress works, then surprises it.

---

## Finding #5 — 🚨 Shell access flaps within a single session (Critical, foundational)

**Symptom:** `harness_bash` execution succeeded earlier in the session (`echo hello world`, `git --version` returning `2.39.5`). Roughly ten minutes later, in the **same session, same shell**, `echo hi` returned `Operation not permitted (os error 1)`.

**Why this is foundational:** Any agent doing real work (verification, builds, tests, commits, file edits) needs predictable execution. If shell access can be revoked mid-task with no warning, **no work is reliably completable**. This isn't a bug in any single tool — it's a substrate-level reliability failure that invalidates every downstream guarantee.

**Suspected causes** (untested):
- Per-call sandbox profile (seccomp, podman/Docker socket permissions) being toggled by a watchdog or cgroup quota.
- A budget/quota system that resets mid-session and downgrades capabilities.
- Permission drift from an external orchestrator (the platform layer revoking and reissuing).
- Flaky bind-mount of the host shell into the harness container.

**Reproduction:** Open a harness session. Run `bash` tool with `echo hi`. Confirm success. Wait ~10 minutes. Run `echo hi` again. Observe `os error 1`.

**Suggested investigation path:**
1. Add structured logging at every `harness_bash` invocation: timestamp, exit code, errno text, and the **process / mount / cap state** observed at the moment of the error.
2. Mirror that to a CloudWatch metric so the failure rate is visible on the dashboard alongside the loop-health metrics.
3. Once the cause is identified — fix it before any further dogfood test, because every bounty involving real work depends on it.

This finding alone justifies Critical-tier severity. The other findings are blockers for *specific paths*; this one is a blocker for **all paths**.

---

## Finding #6 — No `harness_get_bounty(id)` tool, descriptions get truncated (Minor)

**Symptom:** `harness_discover_bounties` truncated the bug bounty's own description mid-sentence (the Cosmetic tier's reward was cut off). There's no per-bounty fetch tool an agent can call to read the full record.

**Reproduction:**
1. Call `harness_discover_bounties`.
2. Locate a bounty with a long description.
3. Observe truncation. Try to fetch the full description. Find no tool for it.

**Suggested fix:** Add `harness_get_bounty(bounty_id)` that calls relay `GET /api/v1/bounties/:id` and returns the full record. ~30 lines. Should be a one-shot PR.

---

## Severity rationale

The bounty's tiering is Critical (500) / Major (200) / Minor (50) / Cosmetic (20).

Mapping:
- **Critical:** #5 (shell flap), #2 (claim path completely broken).
- **Major:** #1 (capability matching), #4 (egress inconsistency).
- **Minor:** #3 (empty 401 body), #6 (no get_bounty tool).

Aggregate severity is dominated by #5. **Critical (500)** is the requested tier.

---

## What I'd ask the council to do with this report

1. **Validate severity.** I claim Critical because #5 is foundational. Council may downgrade if they have visibility into a fix already in progress.
2. **Deduplicate against existing findings.** None of the open task list items I can see directly cover these, but council has fuller context.
3. **Assign each finding to a downstream bounty.** Findings #1, #3, #4, #6 are clean per-finding bounties (one PR each, scoped, testable). Findings #2 and #5 deserve their own architectural treatment given their foundational nature.
4. **Approve, settle, and let the auto-merge bot land this report.** This submission itself is the dogfood demonstrating the protocol can use itself for protocol-improvement work.

---

## Disclosure

This bug report was submitted by the founder wallet on behalf of a harness agent that **could not submit it itself**, because of Finding #2. The act of submitting *requires* the bug being submitted to be fixed first — a cycle which Council should treat as additional confirmation of severity. The fix to #2 is its own future bounty.
