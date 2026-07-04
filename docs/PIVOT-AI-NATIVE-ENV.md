# AMOS-PIVOT-001 — AI-Native Governed Environment

> **STATUS (July 2026): SUPERSEDED IN PART by `amos-managed-platform/docs/COMPANY-BRAIN.md`.**
> The commercial framing moved from *"the governed environment Claude Code operates"* (a devtools/infra play, this doc) to *"the company brain"* (a business OS operated by the customer's own AI — see COMPANY-BRAIN). The governance/proof content here (proofgate → Plumbline, receipts, guardrails, the entity firewall, §7's north star) remains valid; the positioning and workstream framing do not.
> **Do NOT cite this doc's positioning in `policy_refs` — cite COMPANY-BRAIN.** Kept as the June 2026 decision record.

**Status:** DIRECTION + KEY DECISIONS LOCKED (Rick, June 2026). See §6. *(Superseded in part — see banner.)*
**Authors:** Rick (framing), Claude (drafting)
**Audience:** AMOS coding agent (+ human reviewers)

**Reads first (cite in `policy_refs`):**
- `docs/core/thesis.md` — the protocol-organism thesis (what we are *decoupling from*, not deleting)
- `docs/core/business-plan.md` — the commercial wedge (SMB automation, public-safety/eLearning beachhead)
- `docs/AMOS_PROOF_CARRYING_DEV_PIPELINE.md` — META-007; the governance contract this pivot productizes
- `amos-managed-platform/README.md` — the control plane that becomes the core product

---

## 1. Thesis

AMOS is repositioning from **"a Solana-settled autonomous economic protocol with its own harness and agent"** to **"the AI-native, governed environment that Claude Code operates."**

This is a **subtraction, not a new build.** We keep the parts of AMOS that are already real, already have customers, and are least contested. We bury the parts that compete with everything at once and depend on token volume materializing.

The sharp positioning:

> **Governed, AI-native environments where Claude Code operates your stack via MCP — and every change ships with a proof receipt. Let agents do real work without losing the audit trail.**

*The long-arc reason this matters — governance is the runway to autonomy — is §7. That's the north star; everything above is how we earn it.*

Three commitments hold this together:

1. **Don't build the agent.** Claude Code is the agent. AMOS provides the environment it operates in. (`amos-agent` becomes an optional default/fallback, not the headline.)
2. **Don't build the forge.** GitHub stays the operational surface (this is already META-007's stance). AMOS owns the contract/governance layer above it via proofgate.
3. **Don't depend on the token.** The commercial product must run with zero on-chain dependency. Protocol mode (relay/oracle/solana/RSI) becomes optional, decoupled — not deleted.

**Why now:** the original thesis competes with frontier labs (on the agent), crypto-agent projects (on the protocol), and devtools (on the forge), and can't self-fund until token volume appears. This pivot picks **one defensible layer** and rides Claude Code instead of fighting it — an Anthropic-ecosystem / picks-and-shovels play. The moat is **proof-carrying governance + isolated provisioning + MCP-native resource access**, not the hosting (containers are commoditizing: E2B, Daytona, Modal, Vercel).

**Unfair advantage:** three live proof cases — **Cuspr** (clinical SaaS, dogfooding proofgate + GitHub-native CD), **Nuvola Academy** (mature, profitable, compliance-heavy public-safety/law-enforcement training, where provable completion *is* the product), and a **construction CRM** built by a **separate services company operating as an AMOS reseller** — a construction company commissioned it and brings a channel of hundreds of similar companies to resell to. The reseller (services co) builds and distributes the vertical app; **AMOS Labs stays the platform.**

**Two go-to-market motions on one platform:**
1. **Direct compliance verticals** — regulated SMBs (training, public safety, legal, clinical) where proof receipts are a budgeted compliance feature. *(Nuvola, Cuspr.)*
2. **Build-and-resell ISV channel** — an operator builds a vertical app on AMOS via Claude Code, then resells it (**white-label, multi-tenant**) to their own network. One signed builder can bring hundreds of downstream tenants. *(Construction CRM.)*

These share the **same core** (multi-tenant provisioning + governed delivery + Claude-Code operability), so two motions is *not* scope creep — it's distribution leverage on top of direct sales. The build-and-resell motion does add requirements: **white-label (reseller's brand) and multi-tenant fan-out** in provisioning/templates (WS-5/WS-7), and **wholesale/reseller billing** (see §6).

**Entity boundary (the services-drag firewall):** the vertical apps (e.g. the construction CRM) are built and resold by **separate services companies acting as AMOS resellers** — the first is a Rick-owned services co — *not* by AMOS Labs. This is the `thesis.md` model (neutral core + permissionless services layer + Services Co bootstrapping demand) and it is what keeps the platform clean. **Discipline for the agent:** AMOS Labs builds *generic reseller primitives* (white-label, per-tenant provisioning/metering, wholesale billing rollup) that *any* reseller can use. The Rick-owned services co is the first **customer and design partner** for those primitives, not a special case — never bake services-co- or construction-specific logic into the platform. Custom vertical work lives in the reseller; reusable primitives live in AMOS.

---

## 2. Keep / Bury / Salvage (repo + crate map)

| Disposition | Component | Action |
|---|---|---|
| **CORE** | `amos-managed-platform` (provisioning, sync, billing, governance) | The product. Harden as the multi-tenant AI-native env control plane. |
| **CORE** | `proofgate` | First-class platform feature: default governed delivery on every customer env; surface receipts as audit artifacts. |
| **CORE (new)** | **MCP control plane** | Build. Expose env resources (provisioning, logs, deploy state, DB, running container, billing/usage) as MCP servers Claude Code calls. The differentiator + the token-efficiency story. |
| **SALVAGE → reframe** | `amos-harness` | Reframe as the per-customer *environment / toolbox* Claude Code plugs into (tools, credentials, schemas, MCP servers). Not a competing agent runtime. |
| **SALVAGE → demote** | `amos-agent` | Optional default agent / fallback. Not the headline; Claude Code is primary. |
| **SALVAGE → repurpose** | `amos-marketplace`, `amos-packages` | Distribution for env templates + vertical packages (clinical→Cuspr, compliance-training→Nuvola). |
| **DECOUPLE (protocol mode)** | `amos-relay`, `amos-oracle`, `amos-solana`, token economics, RSI, physics gradient | Feature-flag/extract so the commercial product runs without them. Keep alive as optional long-game. **Do not delete.** |

---

## 3. Workstreams (each is a proof-carrying contract)

Pick these up as bounties/tickets. Each ships intent + validation plan + receipt per `AGENT_CONTRACT`. Order is roughly dependency order; each is independently shippable.

### WS-1 — MCP control plane (the differentiator)
**Intent:** Expose the managed-platform's resources as MCP servers so Claude Code operates an AMOS environment via tool calls instead of blind discovery.
**Scope:** provisioning (create/start/stop/deprovision), sync/heartbeat, logs, deploy/build state, per-tenant DB access (scoped), billing/usage. Start read-only, add mutating tools behind auth/policy.
**First slice (decided — agent's discretion exercised):** **provision + deploy/release + logs + status/health + env config/secrets** — the minimum surface that makes *deploy & operate an env from Claude Code* (WS-7) work end-to-end. DB access and billing/usage tools follow once that loop is solid. Rationale: the onboarding/deploy flow is the wedge, so the MCP surface should make *that* trivial first, not expose everything at once.
**Acceptance:** Claude Code, given only the MCP endpoint + creds, can inspect and operate a provisioned env end-to-end; a documented task completes with **measurably fewer tokens** than the cold-start/grep baseline (capture the benchmark — it's also the sales artifact).
**Out of scope:** building a new agent; replacing GitHub.

### WS-2 — Claude Code as the operator
**Intent:** Make Claude Code the primary agent driving an AMOS env via the WS-1 MCP layer; demote `amos-agent` to optional default.
**Acceptance:** a customer env can be operated by Claude Code with no dependency on `amos-agent`; `amos-agent` still runs when selected.
**Out of scope:** removing `amos-agent`.

### WS-3 — Governed delivery as a platform feature
**Intent:** Wire proofgate as the default GitHub-native gate on every customer env; productize receipts as an audit/compliance artifact surfaced in the platform (admin/dashboard, exportable).
**Acceptance:** a new env ships with proofgate configured; receipts are queryable per-tenant; a compliance-style "audit export" of receipts exists (the Nuvola/Cuspr selling point).
**Out of scope:** changing the receipt schema (META-007 owns it); on-chain settlement.

### WS-4 — Decouple protocol mode
**Intent:** Feature-flag/extract relay/oracle/solana/token so the commercial product builds and runs with them off.
**Acceptance:** managed-platform compiles + runs + provisions + bills + governs delivery with `protocol_mode = off`; turning it on restores current behavior. No commercial path imports a Solana dependency.
**Out of scope:** deleting protocol code; changing token economics.

### WS-5 — Env templates + vertical packaging
**Intent:** Provisionable env templates per vertical (clinical→Cuspr, compliance-training→Nuvola) with the right tools/MCP servers/proofgate policy preloaded.
**Acceptance:** one command provisions a vertical-templated env with governance on. **Cuspr first** — it's the more complex stack (clinical SaaS: 4 container images, Helm, segmentation worker, Postgres, Key Vault); once Cuspr works, Nuvola (Rails LMS) is straightforward.
**Out of scope:** new verticals beyond the two proof cases (yet).

### WS-6 — Token-efficiency benchmark (proof artifact)
**Intent:** Reproducible benchmark: MCP-native env vs. cold-start/grep baseline, tokens-per-task + reliability. Becomes the demo + the "AI-native, not token-wasteful" evidence.
**Acceptance:** published numbers from a real task on a real env; rerunnable in CI.
**Out of scope:** synthetic-only benchmarks.

### WS-7 — One-flow deploy & migrate from Claude Code (the onboarding product)
**Intent:** Make standing up a governed AMOS env trivial from Claude Code. This is the onboarding surface that *sells* the platform — "deploy and operate, all from Claude Code." Built on the WS-1 MCP first slice.
**Three entry paths:**
- **New build** — in a fresh/empty repo, Claude Code provisions an env, scaffolds packaging (Containerfile/Helm or AMOS packaging), wires proofgate + GitHub-native CD, and ships a hello-world to a live URL.
- **Migrate from Azure (AKS)** — point AMOS at an existing app (containers, Helm charts, Key Vault secrets, managed Postgres); it replicates into an AMOS-hosted env and cuts over. **Cuspr is this proof case**: 4 images + Helm + AKS + Key Vault + Postgres, and its ADO→GitHub CD cutover is already done, so the host migration is the next step. The painful manual version of this is exactly what we just lived — perfect agent task.
- **Migrate from AWS (EKS/ECS)** — same as Azure with ECR/EKS/Secrets-Manager mappings.
**Acceptance:** each path goes from "connect" to "live governed env with proofgate on" in a single Claude-Code-driven flow; Cuspr migrates from AKS to an AMOS env with receipts and no manual K8s surgery.
**Out of scope:** every cloud/orchestrator on day one — Azure-AKS + AWS-EKS first (the proof-case stacks). Bare-metal/other PaaS later.

---

## 4. Guardrails (do NOT)

- **Do not build a competing agent.** Claude Code is the agent. If you're writing agent loop/planning logic, stop and reframe as environment/tooling.
- **Do not build a competing forge.** GitHub stays. Receipts/governance sit above it.
- **Do not couple the commercial product to the token or chain.** Protocol mode is optional.
- **Do not delete protocol-mode code.** Decouple behind flags; it's the long game and the source of future "external commercial signal" the thesis needs.
- **Do not let scope re-expand** into buy-and-automate PE roll-up or full-protocol ambitions. This pivot wins by narrowing. Cuspr and Nuvola are **customers/proof, not parallel bets.**

## 5. Execution discipline

This work governs itself by the same gate it productizes. Every PR carries a proof receipt per `.proofgate/AGENT_CONTRACT.md` (or `docs/AMOS_PROOF_CARRYING_DEV_PIPELINE.md`): intent restating the workstream's behavior, validation plan incl. the workspace's required checks, evidence, and `diff_sha256` against branch HEAD. Touching `amos-solana`, billing, auth, or the gate itself ⇒ `self_modifying: true` (human review). No receipt, no merge.

## 6. Decisions (locked, Rick — June 2026)

1. **Naming:** **AMOS for now.** No rebrand this phase.
2. **Decouple protocol mode:** **full crate extraction** (not just flag-off). Do it right — relay/oracle/solana/token come out as cleanly separable crates the commercial product doesn't compile against. Slower, but no lingering coupling.
3. **MCP surface:** agent's discretion → first slice = **provision + deploy + logs + status + config/secrets** (see WS-1), chosen to make WS-7 deploy-from-Claude-Code work first.
4. **First dogfood:** **Cuspr first** (most complex; Nuvola is easy after). Drives WS-5 and WS-7.
5. **Deploy must be trivial from Claude Code** across **new build + migrate-from-Azure + migrate-from-AWS** → this is now WS-7, a first-class workstream.

### Billing model (decided in principle; structure below for discussion, refine before GA)

Rick's call: resource uplift on AWS + a base bundle of AI credits + purchasable overage, OR bring-your-own model API key. The structure below is the recommended shape of that — **talk it through with the agent and adjust.**

**The trap to avoid:** if the headline price is "AWS cost + a margin," AMOS is a *thin cloud reseller* — eating AWS pricing, competing on price with Render/Vercel/Fly, with margin that compresses as clouds cut prices. The value AMOS owns is **governance + audit + agent-operability**, not raw compute. Price for that.

**Recommended structure (4 components):**

| Component | What it is | Why |
|---|---|---|
| **Platform / governance fee** (the value metric) | Per-seat or per-env monthly: governed delivery (proofgate receipts), audit/compliance export, the MCP control plane, Claude-Code operability | This is the margin core and the thing the compliance ICP actually buys. Price scales with *governance value*, decoupled from compute. |
| **Resources** | Underlying AWS at pass-through + **modest uplift (~15–30%)** | Covers infra + ops, not the profit center. Keeps you price-competitive without betting margin on compute. |
| **AI — managed** | Base bundle of credits in the platform fee; overage purchasable at markup | Simple default for SMBs; some AI margin. |
| **AI — BYO key** | Customer brings their own Anthropic/Bedrock/etc. key; no AI markup | Removes AI-cost risk, lands data-governance-sensitive / enterprise buyers (and the compliance ICP will *want* their own key + data boundary). |

**Compliance tier:** a higher tier for audit-export, receipt retention, SSO, etc. Regulated buyers (Nuvola's agencies, Cuspr's clinical context) pay for exactly this — it's the highest-margin, most-defensible SKU.

**Reseller / wholesale tier:** for build-and-resell partners (e.g. the construction CRM channel), price **wholesale to the partner**, who retails (white-label) to their downstream tenants — per-tenant economics with a partner margin. This is a distribution multiplier, not a one-off sale, and likely the fastest path to volume. Needs: white-label branding, per-tenant provisioning + metering, and partner-level billing rollup.

**Net:** charge for governance and outcomes; treat compute as a modest-uplift pass-through; make AI either an easy bundle or a no-markup BYO. Decide platform-fee-led vs. usage-led pricing before GA.

---

## 7. North Star — Governance Is the Runway to Autonomy

This pivot does not abandon the original AMOS thesis (`thesis.md`: bounded autonomous economic organism, RSI, the proof-carrying loop). It is **the only safe road to it.** META-007 already says it plainly: *"This is the substrate RSI needs. AMOS can safely improve itself only when every self-modification carries an inspectable proof."* Governance is not a side feature — **it is the precondition for autonomy.** You cannot responsibly let agents run unattended without receipts, gates, override accountability, and an audit trail.

So the commercial product *is* the autonomy substrate. The arc:

```
Governed delivery (today)
  → every deployment emits a proof receipt = a verifiable record of what agents did and whether it was good
    → accumulated proof + override accountability + failure capsules calibrate trust
      → progressive trust raises the autonomy ceiling, step by step
        → more of the loop runs agent-driven without a human in it — safely, because the gate still catches failures
          → bounded autonomy, grounded in REAL commercial workflows
```

Three consequences that should shape what we build:

1. **The commercial business is the missing external signal.** `thesis.md` names its own central execution risk: the RSI loop is self-referential until external commercial volume grounds it. Every Cuspr deploy, every construction-CRM tenant, every Nuvola course shipped through the gate **is** that grounding signal — it pays the bills *and* earns the right to dial autonomy up. The pivot funds and de-risks the autonomy vision rather than deferring it.

2. **Receipts are a compounding data asset.** A growing, verifiable corpus of agent work + outcomes + gate decisions is what licenses higher autonomy — internally, and as a product ("we'll run more of your workflow unattended; here is the proof it's safe"). **The autonomy ceiling becomes a product dial and a pricing axis, not a binary.** No one competing on raw container hosting accumulates this.

3. **HARD COMMITMENT — keep the gate real.** The entire arc rests on one thing: the gate must have teeth. META-007 §6: *"the override needs teeth or it becomes the path of least resistance and receipts become theater."* If proofgate degrades into a rubber stamp, **both** the compliance value **and** the autonomy runway collapse at once. Substantive content review (not just shape checks), override that costs reputation, failure capsules that bite, `self_modifying` routed to the strict path. This is non-negotiable; treat any change that weakens the gate as `self_modifying: true` (human review) by definition. Never remove or weaken gates, tests, or checks.

**Discipline — external vs. internal.** Sell **control**, not autonomy. Compliance buyers and reseller partners want *more control over AI*, not a promise it will run itself — "eventual autonomy" reads as *risk* to them.
- **Externally / sales / customer-facing:** governance, audit, proof, control. Never headline autonomy.
- **Internally / investors / this doc:** autonomy is the north star and the upside — **earned through accumulated proof, never promised on a slide.** Bounded, legible, human-governed, exactly as `thesis.md` frames it.

The through-line: **govern agent work for businesses today; let the proven track record progressively unlock autonomy tomorrow.**
