# Oracle Review

The Oracle is the semantic review layer for AMOS. It does not replace Relay validation or CI. It answers the question deterministic checks cannot answer: did this work actually move AMOS in the right direction?

## Role

The Oracle reviews:

- proposed system bounties
- submitted bounty completions
- proof receipts
- validation plan coverage
- mission alignment
- self-modifying changes
- override requests

The live prompt source is [amos-oracle/prompts/amos_constitutional_v1.md](../../amos-oracle/prompts/amos_constitutional_v1.md).

## Receipt-Aware Inputs

For review, the Oracle should receive:

- bounty description and acceptance criteria
- `proof_receipt`
- validation plan and executed commands
- changed files and PR metadata
- failure capsule, if any
- similar precedent decisions and outcomes
- current network health constraints

## Decision Output

Oracle decisions must be structured. At minimum, a review decision should include:

- verdict: `approve`, `reject`, `revise`, or `escalate`
- confidence
- short-term value
- long-term value
- tension resolution
- mission alignment notes
- validation coverage notes
- false-approve versus false-reject weighting
- feedback when revision is required

## Self-Modifying Changes

If a receipt has `self_modifying: true`, Oracle review must escalate to council unless the configured constitutional process explicitly allows automated approval. Override is not available for self-modifying work.

## Two-Tier Split

Relay checks whether the receipt is well-formed and complete. Oracle checks whether the receipt is persuasive.
