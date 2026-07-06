# Mission — amos-platform-2.0

This repository is the **open-source AMOS infrastructure for autonomous work**, written in
pure Rust. It is the substrate a business runs on: the per-customer **harness** (the AI
runtime — agent loop, tool surface, canvas, sites, schema, memory), the shared **core**
crate (config, errors, token economics), the admin **CLI**, and the first-party **packages**.
The managed control plane (tenants, billing, provisioning) lives in a separate private repo;
the long-game economic protocol (relay, Oracle, Solana programs, agent) lives in
`amos-labs/amos-protocol`. What merges here is what customers actually download and run.

## What a change here must honor

1. **The agent loop is the product's trust boundary.** `amos-harness/src/agent/` decides what
   the model does with a customer's data and credentials, and `prompt_guard.rs` is the fence
   against injection and unsafe instructions. Weakening either can turn an autonomous worker
   into an exfiltration path. Changes there are `self_modifying` and require human review.
2. **Tools are capabilities, not helpers.** Every tool in `amos-harness/src/tools/` grants the
   agent a real-world power (db writes, S3, HTTP, code execution, provisioning). New or widened
   tools expand the blast radius — they carry a validation plan and escalate.
3. **Secrets and connections are load-bearing.** The credentials, oauth, and connections routes,
   and the auth/rate-limit/security-headers middleware, guard customer keys and access. A scoping
   or refresh miss is a credential leak, not a cosmetic bug.
4. **Migrations are append-only.** Never modify an already-applied sqlx migration — sqlx checksums
   crash the harness on mismatch. New schema goes in a new migration. `migrations/**` is protected.
5. **The gate governs the gate.** Workflows, this `.plumbline/` directory, the core crate, and the
   harness test suite are protected paths — changes to the enforcement and verification machinery
   always escalate to a human.
6. **This is open source others run.** A regression here ships to every self-hosted operator and
   every managed harness image. The receipt's evidence must reflect the real CI runs on the PR
   head (lint/unit, release gate, integration smoke, HTTP integration).

## The loop for contributors (human or agent)

`plumb propose` (issue + contract) → work → `plumb receipt --write` → fill the judgment
fields honestly → push → CI tests + the Plumbline gate must pass → human review where
escalated → merge.

Automate the bookkeeping; never the judgment.
