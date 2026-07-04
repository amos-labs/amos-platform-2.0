# AMOS Architecture — where things run

**The map that pairs with the North Star.** The North Star says *where we're going*; this says *where each thing runs* after the pivot from "AMOS hosts the AI" to "the customer's AI operates AMOS."

## The pivot, in one line
AMOS stopped being the AI. The AI is the customer's — **Claude Code, Codex, or any LLM** — and it operates AMOS over **MCP**. AMOS is the ecosystem it operates.

## The unifying principle
**MCP is the seam; the actor is pluggable.** Everything AMOS does is an **MCP verb**, and *who calls it* is interchangeable. That single idea decides where everything lives.

## Three layers

**1 — The actor** (reasoning, conversation, initiating). Lives **outside** AMOS.
- **Primary:** the customer's own AI (Claude Code / Codex / any MCP client).
- **Optional (managed):** the harness's embedded agent — now **just another MCP client**, for customers who don't bring their own. Demoted from "the cockpit," not deleted.

**2 — The runtime substrate** (state + deterministic execution). The **harness**, per tenant.
- Collections/records, canvases (store + render), automations (run without any AI), connections, sites, memory.
- The business's source of truth and where deterministic things happen. **Modules (P1) live here.**

**3 — The control plane.** The **platform**.
- The MCP endpoint the actor connects to, provisioning, billing, RBAC, proofgate/governance, the module **registry**.

## The division of labor
- **Reason · decide · generate · converse → the actor.**
- **Store · execute · persist · render · enforce → AMOS.**
- **MCP is the line between them** — now *the* product interface, not a secondary one.

## Consequences (what moves, given the pivot)
- **Canvas *generation*** → the actor defines a view via an MCP verb; the harness only **stores + renders** it. The harness no longer needs its own LLM to generate UIs.
- **Chat** → the actor's client *is* the chat surface.
- **The embedded agent + its own Bedrock calls** → become the **managed-AI option**, one MCP client among others. Trim or keep, but no longer central.

## Decisions recorded
- **BYO-AI is primary; managed AI is an option** — both are MCP clients hitting the same verbs.
- **P1 (the module/template system)** was first built harness-native here, then **re-homed to the shared multi-tenant brain in the managed platform** (starters + `apply_starter` on the platform's tenant-scoped data layer). *Superseding decision:* `amos-managed-platform/docs/COMPANY-BRAIN.md` — the harness narrows to the **dedicated-runtime tier** (complex apps with real background jobs/custom code); standard apps run on the shared brain.

## What this unblocks
The layer map stands; the substrate question (shared brain vs. per-tenant harness) was resolved by complexity-based routing — see COMPANY-BRAIN §3.

---

*Pairs with: `NORTH-STAR.md` (the goal — canonical copy in `amos-managed-platform`) · `amos-managed-platform/docs/COMPANY-BRAIN.md` (the resolved model) · `docs/protocol/receipt-schema.md` (the open standard).*
