# AMOS — the open substrate

**Open infrastructure for AI-operated businesses.** This workspace holds the
open-source pieces of AMOS: the shared core, the per-customer **harness**
runtime, and the admin CLI. The managed control plane (provisioning, billing,
the shared multi-tenant brain) lives in the private platform repo; the product
is at **[amoslabs.com](https://www.amoslabs.com)**.

> **Where did the protocol go?** The AMOS protocol — the bounty **relay**,
> **Oracle**, **Solana** programs, and the autonomous **agent** — was
> extracted to [`amos-labs/amos-protocol`](https://github.com/amos-labs/amos-protocol):
> the long-game economic layer, actively developed as a research/side track
> alongside the commercial platform (scale triggers in [`docs/NORTH-STAR.md`](docs/NORTH-STAR.md)).
> Its proof-receipt core lives on as
> [**Plumbline**](https://github.com/amos-labs/plumbline) and the platform's
> operation receipts.

## Workspace

```
├── amos-core       Shared config, errors, types (feature-gated protocol token math)
├── amos-harness    Per-customer runtime (tools, canvas, schemas, sites, templates)
├── amos-cli        Admin CLI
├── amos-packages/  Legacy compiled packages (being retired in favor of runtime starters)
└── docker/         Production Dockerfile (harness)
```

## Build & run

```bash
cargo build
cargo run --bin amos-harness      # port 3000
cargo test --lib -p amos-harness -p amos-core -p amos-cli
./scripts/dev-setup.sh            # macOS: local PostgreSQL + Redis
docker compose up postgres redis -d
```

Configuration is env-driven with the `AMOS__` prefix (`__` as the nested
separator) — see `.env.example`.

## Docs

- [`docs/NORTH-STAR.md`](docs/NORTH-STAR.md) — capture-resistant rails: where this is going
- [`docs/protocol/receipt-schema.md`](docs/protocol/receipt-schema.md) — the open receipt standard
- [`docs/protocol/proof-carrying-loop.md`](docs/protocol/proof-carrying-loop.md) — the proof-carrying loop
- [`docs/README.md`](docs/README.md) — index

Apache 2.0.
