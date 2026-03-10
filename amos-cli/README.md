# amos-cli

Command-line interface for managing AMOS harness and platform instances.

## Binary

```bash
cargo run --bin amos -- --help
```

## Commands

```
amos health              # Check harness health
amos harness status      # Harness instance status
amos platform status     # Platform status
amos platform token stats # Token economics stats
amos config show         # Show current configuration
```

## Architecture

Uses `clap` for argument parsing. Connects to harness and platform HTTP APIs. Provides a human-friendly interface for operations that would otherwise require curl.
