# AMOS Docs Index

Current truth first. The AMOS protocol corpus (thesis, bounty lifecycle, Oracle, token economy,
Solana settlement, EAP, ecosystem playbooks, legacy archive) lives with the
protocol — an actively developed side track — at
[amos-labs/amos-protocol](https://github.com/amos-labs/amos-protocol).

## Read first

1. [NORTH-STAR.md](NORTH-STAR.md) — pointer to the canonical compass (lives in `amos-managed-platform`)
2. [ARCHITECTURE.md](ARCHITECTURE.md) — where things run: the pivot's layer map (MCP is the seam; the actor is pluggable)
3. [Receipt Schema](protocol/receipt-schema.md) — the open receipt standard
4. [Proof-Carrying Loop](protocol/proof-carrying-loop.md) — how work carries proof
5. [Proof-Carrying Dev Pipeline](AMOS_PROOF_CARRYING_DEV_PIPELINE.md) — META-007, the governance contract (now productized as [Plumbline](https://github.com/amos-labs/plumbline))

## Decision record (superseded — kept for history)

- [PIVOT-AI-NATIVE-ENV.md](PIVOT-AI-NATIVE-ENV.md) — the June 2026 pivot direction; **superseded in part by COMPANY-BRAIN** (see its banner)
- [PIVOT-INVESTOR-ONEPAGER.md](PIVOT-INVESTOR-ONEPAGER.md) — June framing; **do not send** (see its banner)

## Reference

- [Example proof receipt](EXAMPLE_PROOF_RECEIPT.json)
- [Subscription & onboarding](SUBSCRIPTION_AND_ONBOARDING.md)
- [core/](core/) — backup & recovery (harness ops)
- [features/](features/) — feature-level docs (test harness, security, …)
- [packages/](packages/) — legacy package docs (packages are being retired for runtime starters)

> **amos-family-finance status:** to be **re-homed as a starter composition on the shared brain** (see `amos-managed-platform/docs/COMPANY-BRAIN.md` §5 and the P8 packages-retirement plan); the compiled-package form is deprecated — do not extend it in this repo. (The package currently exists only as untracked local work.)

The platform-side docs (COMPANY-BRAIN, FIRST-RUN, build plans) live in the
private `amos-managed-platform` repo.
