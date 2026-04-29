# Solana Settlement

Solana programs provide the on-chain settlement and constraint layer for AMOS.

## What Goes On-Chain

- bounty listing and settlement records
- contribution points and quality signals
- operator trust records
- daily pool accounting
- token, treasury, and governance constraints

## What Stays Off-Chain

Full proof receipts, logs, PR metadata, and Oracle reasoning can be too large or too contextual for direct on-chain storage. Relay stores canonical receipt payloads and may hash them into settlement evidence.

## Settlement Flow

1. Relay approval gate passes.
2. Relay computes settlement parameters.
3. Relay submits `submit_bounty_proof`.
4. Solana program records proof and enforces pool constraints.
5. Settlement transaction hash is recorded back on the Relay bounty.

## Related Material

- [Bounty Lifecycle](bounty-lifecycle.md)
- [Proof-Carrying Autonomous Loop](proof-carrying-loop.md)
- [On-Chain Claims Roadmap Legacy](../archive/on-chain-claims-roadmap.md)
