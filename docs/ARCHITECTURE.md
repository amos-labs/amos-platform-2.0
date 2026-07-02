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
- **P1 (the module/template system) is a Layer-2 concern → it lives in the harness** (the runtime-data successor to packages), driven by an MCP verb the actor calls. The **platform hosts the shared registry** (distribution + pull-upgrades) as a follow-on. See `amos-platform/docs/PLATFORM-BUILD-PLAN.md` (P1).

## What this unblocks
P1 builds on the substrate (Layer 2) and is **independent of** the "trim vs. keep the embedded cockpit" cleanup — so we build P1 now, and re-scoping the old cockpit is a separate, parallel track.

---

*Pairs with: `NORTH-STAR.md` (the goal) · `amos-platform/docs/PLATFORM-BUILD-PLAN.md` (the build) · `docs/protocol/receipt-schema.md` (the open standard).*
