# AMOS — Investor One-Pager

> **STATUS (July 2026): DO NOT SEND.** This pitch reflects the June framing ("the governed environment Claude Code operates" — a devtools play). The company now sells **the company brain** (see `amos-managed-platform/docs/COMPANY-BRAIN.md`, the vision doc, and amoslabs.com). Reconcile with those before this goes in front of anyone — as written it pitches a different company than the homepage describes.

*Pivot brief — June 2026. Draft. Companion to `AMOS-PIVOT-001` (engineering direction — itself superseded in part).*

> **AMOS is the AI-native, governed environment that Claude Code operates.** Businesses deploy and run real software through AI agents — with a proof receipt on every change.

---

### The problem
AWS and Azure were built for humans, not agents. Every AI agent that touches your stack starts **cold** — re-reading the repo, re-discovering your infrastructure, burning tokens and wall-clock time — and nothing it ships carries proof it did the right thing. So businesses can't let agents do real work without losing the audit trail. **Compute is commoditized; trust and governance are not.**

### The product
An AI-native hosting environment with four properties no hyperscaler ships today:
- **MCP-native** — your environment's resources (deploys, logs, data, status) are exposed to the agent as tools, so Claude Code *operates* your stack instead of blindly exploring it. Dramatically fewer tokens, far more reliable.
- **Claude Code is the agent** — we don't build a competing agent or a competing GitHub. We ride the best agent and the standard forge, and own the governed layer above. Picks-and-shovels for the agent that's winning.
- **Proof-carrying delivery (proofgate)** — every change ships with a machine-checked receipt: intent, validation, evidence, gate decision. *No receipt, no merge.* Governance and audit, built in.
- **Deploy from Claude Code** — new build, or migrate an existing AWS/Azure app, in one agent-driven flow.

### Why now
Agents crossed the threshold of doing real engineering work in 2026, and MCP is becoming the standard protocol for connecting them to systems. The infrastructure purpose-built for agents — governed and MCP-native — doesn't exist yet. The hyperscalers are bolting agent features onto human-era platforms; AMOS starts AI-native.

### Why us — the moat
Not the hosting (containers commoditize). The moat is **governed delivery + an MCP-native environment + a compliance/audit story** — plus two live proof cases most infra startups never get:
- **Cuspr** — clinical treatment-planning SaaS, in active turnaround, dogfooding the full stack today (already on proofgate + GitHub-native CD).
- **Nuvola Academy** — mature, **profitable** compliance-training platform for public-safety / law-enforcement agencies, already running proofgate. Provable completion and audit trails *are its product* — exactly what AMOS sells.
- **Construction CRM** — built by a **separate services company operating as an AMOS reseller**: a construction company commissioned it and brings a **channel of hundreds of similar companies to resell to.** The reseller builds and distributes; **AMOS Labs stays the platform.** One partner → hundreds of downstream tenants.
- Plus early AMOS customers already running isolated managed environments.

### Market — one platform, two go-to-market motions
1. **Direct compliance verticals** — regulated SMBs (training, public safety, legal, clinical) where proof receipts are a **compliance feature with a real budget**, not a developer nicety. *(Nuvola, Cuspr.)*
2. **Build-and-resell ISV channel** — operators build a vertical app on AMOS via Claude Code and resell it **white-label, multi-tenant** to their own networks. One builder can bring hundreds of tenants. *(Construction CRM.)*

Same platform and core (multi-tenant provisioning + governed delivery + Claude-Code operability), two motions — **channel leverage on top of direct sales.**

**Structure:** AMOS Labs is the platform. Vertical apps are built and distributed by **independent resellers** (the first is a Rick-owned services company building the construction CRM) — keeping Labs a clean, investable platform rather than a services shop, and proving the channel is already live.

### Business model
**Platform / governance fee** (the value metric: governed delivery + audit) **+ modest resource uplift** on AWS **+ AI as included credits or bring-your-own model key.** Margin sits on governance and outcomes — not on reselling compute. A premium compliance tier (audit export, retention, SSO) is the highest-margin SKU.

### Optional upside
An open protocol / on-chain settlement layer exists as long-game optionality — fully decoupled. The commercial product needs none of it; a paying infra business also generates the real-world demand signal that layer was always missing.

### Team
**Rick Barkley** — serial operator; running the Cuspr turnaround and the profitable Nuvola Academy; built the AMOS platform (Rust control plane, Docker provisioning, billing/governance) and proofgate.

### The ask
*[Raising $___ to: build the MCP control plane + governed-deploy onboarding, convert early AMOS users to the governed-env product, and land the first ___ compliance-vertical customers off the Cuspr / Nuvola proof.]*

---
*Honest framing: numbers in brackets are placeholders for Rick. Traction is stated qualitatively on purpose — no invented metrics.*
