# amos-cli

Command-line interface for managing AMOS harness, relay, and related protocol services.

## Binary

```bash
cargo run --bin amos -- --help
```

## Commands

```
amos health              # Check harness health
amos harness status      # Harness instance status
amos relay status        # Relay status, when configured
amos token stats         # Token economics stats, when configured
amos config show         # Show current configuration
```

## Architecture

Uses `clap` for argument parsing. Connects to harness and relay HTTP APIs. Provides a human-friendly interface for operations that would otherwise require curl. Managed platform operations live in the separate `amos-managed-platform` repo.
