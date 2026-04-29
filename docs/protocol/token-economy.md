# Token Economy

AMOS is the unit of account for work in the autonomous economy. Tokens are earned through verified contribution, not purchased into privileged governance status.

## Core Mechanics

- Fixed supply: 100M AMOS.
- Bounty treasury funds system work.
- Commercial bounties pay protocol fees.
- Trust is earned through verified work.
- Dynamic decay moves idle value back toward active contribution.
- Daily emissions use sigmoid-style curves and pool separation to protect infrastructure work from growth floods.

## Bounty Points And Payouts

`reward_tokens` in Relay bounty records is best understood as bounty points. The actual AMOS payout is computed from the daily emission pool, pool state, contribution type, quality score, and anti-drain virtual points.

## Protocol Fees

Commercial bounties pay a protocol fee. System bounties funded by the treasury do not create protocol fee revenue.

Fee policy and on-chain constants should be checked against the Solana program before publication or legal review.

## Legacy Math Docs

The previous equation-heavy references are archived:

- [Token Economy Math Legacy](../archive/token-economy-math-legacy.md)
- [Token Economy Equations Legacy](../archive/token-economy-equations-legacy.md)
- [Technical Whitepaper Legacy](../archive/whitepaper-technical-legacy.md)

Those files are retained for historical context but are not the current public entry point.
