# AMOS Mainnet Launch Plan

## Token Deployment, LP Setup, and Cost Estimates

**April 2026 | AMOS Labs**

---

## Current State

- Three Anchor programs (Treasury, Governance, Bounty) deployed to **devnet**
- Token mint exists on devnet: `Cm2RGfE3EpYm6s2cfbMYYikjS2CD9vUd6ECxX4pWi2HQ`
- Bounty program initialized on devnet with oracle authority
- SOL price: ~$80 USD (as of April 7, 2026)
- Gemini withdrawal address hold: 7-day waiting period (starts when address is added)

---

## Launch Sequence

### Day 0 (Today): Start the Clock

**Action:** Add your mainnet deployment wallet address to Gemini as a withdrawal destination.

The 7-day hold starts now. Everything else in this plan fits within that window.

---

### Day 1-3: Pre-Deployment Verification

**1. Final devnet testing**

Run the full E2E test suite against devnet:
```bash
cd amos-solana
node scripts/e2e-bounty-test.mjs
```

Verify all three programs:
- Treasury: `receive_revenue()` → distribution splits correctly (50% holders, 40% R&D, 5% ops, 5% reserve)
- Governance: `submit_feature_proposal()` → `vote_for_feature()` → status transitions
- Bounty: `submit_bounty_proof()` → token distribution → `apply_decay()` → decay math

**2. Security review**

- Verify all PDA derivations use correct seeds
- Confirm `advance_halving()` and `apply_decay()` are permissionless (anyone can call)
- Confirm mint authority will be disabled after initial supply minting
- Test edge cases: zero-stake claims, max decay, halving boundary

**3. Generate mainnet keypairs**

```bash
# Program authority (deployer) — this should be a hardware wallet or multisig
solana-keygen new -o mainnet-authority.json

# Oracle keypair (for bounty proof submission)
solana-keygen new -o mainnet-oracle.json
```

Store these securely. The authority keypair controls program upgrades. Consider transferring authority to a multisig (Squads Protocol) post-launch.

---

### Day 4-5: Mainnet Deployment

Once SOL arrives from Gemini (Day 7+), deploy in this order:

**Step 1: Deploy Programs**

Update `Anchor.toml` mainnet program IDs (currently set to localnet IDs — need fresh mainnet deploys):

```bash
# Switch to mainnet
solana config set --url https://api.mainnet-beta.solana.com

# Build programs
anchor build

# Deploy each program
anchor deploy --program-name amos-treasury --provider.cluster mainnet
anchor deploy --program-name amos-governance --provider.cluster mainnet
anchor deploy --program-name amos-bounty --provider.cluster mainnet
```

Each program deployment costs SOL proportional to the binary size (rent-exempt minimum for the program account). Typical Anchor programs: 2-8 SOL each.

**Step 2: Create AMOS Token Mint**

```bash
# Create SPL token with 9 decimals
spl-token create-token --decimals 9

# Record the mint address — this is the AMOS token on mainnet
```

**Step 3: Mint Initial Supply**

Mint 100,000,000 AMOS to the treasury wallet:

```bash
# Create token account for treasury
spl-token create-account <MINT_ADDRESS>

# Mint total supply
spl-token mint <MINT_ADDRESS> 100000000
```

**Step 4: Disable Mint Authority**

This is irreversible. No more AMOS can ever be created:

```bash
spl-token authorize <MINT_ADDRESS> mint --disable
```

**Step 5: Distribute Initial Allocations**

Two-way split: 95% Bounty Treasury, 5% Emergency Reserve.

```bash
# Emergency Reserve (5M) — DAO-locked, governance vote required to access
spl-token transfer <MINT> 5000000 <RESERVE_WALLET> --fund-recipient

# Remaining 95M stays in Bounty Treasury for contributor rewards via daily emissions
```

**Step 6: Initialize Programs**

```bash
# Initialize Treasury with multisig addresses
node scripts/initialize-treasury.mjs --network mainnet

# Initialize Governance
node scripts/initialize-governance.mjs --network mainnet

# Initialize Bounty with oracle authority
node scripts/initialize-bounty.mjs --network mainnet
```

---

### Day 6-7: LP Setup

**Platform: Raydium (recommended)**

Raydium is the dominant Solana DEX with the deepest liquidity and best discoverability. Orca is an alternative.

**LP Strategy:**

There is no dedicated investor pool allocation. Initial liquidity comes from the founder purchasing AMOS on the open market or from a small Bounty Treasury allocation approved via governance vote:

- Founder provides 250,000 AMOS + $5,000 USDC for initial LP pool
- AMOS tokens sourced from founder's own purchased tokens or a governance-approved treasury allocation

This sets the initial price at:
```
$5,000 USDC / 250,000 AMOS = $0.02 per AMOS
```

At $0.02/token, the fully diluted valuation (FDV) is:
```
100,000,000 AMOS × $0.02 = $2,000,000 FDV
```

**Creating the LP on Raydium:**

1. Navigate to Raydium → Liquidity → Create Pool
2. Select AMOS token (by mint address) and USDC
3. Set initial price: $0.02 USDC per AMOS
4. Deposit: 250,000 AMOS + 5,000 USDC
5. Confirm and create pool

**LP Token Handling:**

The LP tokens received represent your position in the pool. Per the whitepaper:
- LP rewards vest over 30 days (prevents farm-and-dump)
- Founder gets 0.05% permanent fee on all LP operations
- Do NOT burn LP tokens yet — keep them for liquidity management

**Alternative: SOL-AMOS pair**

If you'd rather pair with SOL instead of USDC:
```
$5,000 worth of SOL at $80/SOL = 62.5 SOL
62.5 SOL + 250,000 AMOS → price = 0.00025 SOL per AMOS
```

SOL pair has more organic trading volume on Raydium. USDC pair has more stable pricing. Consider launching both pairs eventually, starting with whichever you prefer.

---

### Day 7+: Go Live

1. Verify all programs are initialized and functional
2. Post the first real bounty on mainnet
3. Execute the social media content calendar (Week 1)
4. Publish token contract addresses to GitHub README
5. Submit AMOS to Solana token list (for wallet/DEX recognition)
6. Register on CoinGecko and CoinMarketCap (both require live trading data)

---

## Cost Estimate

All costs at SOL = $80 USD.

### Deployment Costs

| Item | SOL | USD |
|------|-----|-----|
| Treasury program deployment | ~5 SOL | $400 |
| Governance program deployment | ~8 SOL | $640 |
| Bounty program deployment | ~8 SOL | $640 |
| Token mint creation | ~0.01 SOL | $1 |
| Initial token accounts (2 allocation wallets) | ~0.04 SOL | $3 |
| Program initialization transactions (3) | ~0.01 SOL | $1 |
| Mint + distribute + disable authority | ~0.01 SOL | $1 |
| **Subtotal: Deployment** | **~21 SOL** | **~$1,693** |

### LP Costs

| Item | Amount | USD |
|------|--------|-----|
| USDC side of LP | 5,000 USDC | $5,000 |
| Raydium pool creation fee | ~1 SOL | $80 |
| **Subtotal: LP** | | **~$5,080** |

Note: The AMOS side of the LP (250,000 tokens) comes from founder-purchased tokens or a governance-approved Bounty Treasury allocation.

### Operating Buffer

| Item | SOL | USD |
|------|-----|-----|
| Transaction fees (first 3 months of operations) | ~5 SOL | $400 |
| Account rent for PDA accounts (bounty proofs, stakes, etc.) | ~10 SOL | $800 |
| Oracle operations (submitting bounty proofs) | ~5 SOL | $400 |
| **Subtotal: Operations** | **~20 SOL** | **~$1,600** |

### Total Cash Outlay

| Category | USD |
|----------|-----|
| SOL for deployment (~21 SOL) | $1,686 |
| SOL for operations (~20 SOL) | $1,600 |
| SOL for LP creation (~1 SOL) | $80 |
| USDC for LP liquidity | $5,000 |
| **Total** | **$8,366** |

Note: LP AMOS tokens come from founder-purchased tokens or a governance-approved Bounty Treasury allocation — not a dedicated investor pool.

### Minimum Viable Launch

If you want to minimize initial outlay and skip the personal token purchase:

| Category | USD |
|----------|-----|
| SOL for deployment (~21 SOL) | $1,693 |
| SOL for operations (~10 SOL) | $800 |
| Minimal LP: 50K AMOS + 1,000 USDC | $1,000 |
| SOL for LP creation | $80 |
| **Minimum Total** | **$3,573** |

This gets you: programs deployed, token minted, thin LP live, first bounties operational. You can deepen the LP as revenue flows in.

### What to Transfer from Gemini

For the full launch plan:
```
~42 SOL ($3,366) + conversion to 5,000 USDC for LP
Total Gemini transfer: ~$8,366 in SOL
(Plus $5,000 USDC purchased separately or converted from SOL)
```

For minimum viable:
```
~32 SOL ($2,573) + conversion to 1,000 USDC for LP
Total Gemini transfer: ~$3,573 in SOL
```

---

## Timeline Summary

| Day | Action | Dependency |
|-----|--------|------------|
| 0 (Apr 7) | Add wallet address to Gemini | — |
| 1-3 | Final devnet testing, security review, generate mainnet keypairs | — |
| 4-5 | Prepare deployment scripts for mainnet, update Anchor.toml | Devnet tests pass |
| 7 (Apr 14) | Gemini hold expires, transfer SOL | 7-day hold |
| 7 | Deploy programs, mint token, distribute allocations | SOL in wallet |
| 8 (Apr 15) | Create Raydium LP, verify trading | Programs live |
| 8 | Post first mainnet bounty | Bounty program initialized |
| 8 | Launch social media Week 1 (macro thesis thread + LinkedIn) | Content ready |
| 14 (Apr 21) | Social media Week 2 (technical/developer) | — |
| 21 (Apr 28) | Social media Week 3 (philosophical + Reddit + HN) | — |
| 28+ (May 5) | Social media Week 4 (sustained singles) | — |

The 7-day Gemini hold actually aligns perfectly: you use Days 1-6 for testing and preparation, SOL lands on Day 7, deploy on Day 7-8, and launch socials on Day 8 — which is April 14-15, exactly when Week 1 of the content calendar was targeted.

---

## Post-Launch Checklist

- [ ] Verify all three programs on Solana Explorer
- [ ] Confirm mint authority is disabled (irreversible)
- [ ] Submit token to Solana Token List (GitHub PR)
- [ ] Register on CoinGecko (requires 24h of trading data)
- [ ] Register on CoinMarketCap
- [ ] Update GitHub README with mainnet contract addresses
- [ ] Update EAP spec with mainnet token mint address
- [ ] Post first real bounty (social media campaign Week 2 posts)
- [ ] Transfer program authority to multisig (Squads Protocol)
- [ ] Set up monitoring for program accounts (Helius webhooks)

---

## Risk Considerations

**Smart contract risk.** The programs have been tested on devnet but not formally audited. Consider a professional audit (Trail of Bits, OtterSec, etc.) before significant value flows through the system. Cost: $50-150K for a full audit of three programs. This can happen post-launch with a security bounty program in the interim.

**LP impermanent loss.** As a single-sided liquidity provider, you're exposed to IL if AMOS price moves significantly. This is mitigated by the thin initial LP — you're not putting much capital at risk.

**Oracle centralization.** The bounty program relies on an oracle (your keypair) to submit proofs. This is a centralization vector. Roadmap item: transition to a decentralized oracle network or multisig oracle as the network grows.

**Regulatory.** Token launches in the US have regulatory implications. The contribution-based model (earn through work, not investment) and the utility nature of the token (access to bounties, governance) help, but this is not legal advice. Consider consulting a crypto-native law firm.

---

## Related Documents

- **Token Economy Equations:** `token_economy_equations.md`
- **Token Economy Math:** `token_economy_math.md`
- **Whitepaper (Simple):** `whitepaper_simple.md`
- **Whitepaper (Technical):** `whitepaper_technical.md`
- **EAP Specification:** `EAP_SPECIFICATION_v1.md`
- **Social Media Content Kit:** `social_media_content.md`

---

*The first bounty on mainnet should be the Week 2 social media posts. AMOS promoting itself through its own economic loop — the bootstrapping problem solved in real time.*
