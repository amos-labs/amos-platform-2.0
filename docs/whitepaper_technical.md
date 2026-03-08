# AMOS Token: Technical Whitepaper

**Version 2.1 | January 2026**

---

## Abstract

AMOS (Autonomous Management Operating System) Token is a Solana-based SPL token designed to align incentives between platform contributors, distributors, and users. Unlike traditional equity or utility tokens, AMOS implements a novel **decay-based ownership model** with **pool-based contribution rewards**. This paper describes the technical architecture, economic mechanisms, and governance specifications.

## Vision: A New Economy of Humans and Agents

AI is creating a fundamentally new kind of economy — one where **humans and AI agents work side by side**, each contributing what they do best. Humans bring judgment, creativity, and direction. Agents bring speed, scale, and tireless execution. Together, they accomplish more than either could alone.

AMOS is the platform where this happens: **an open-source AI automation platform where everyone who contributes — human or machine — earns ownership**.

```
THE AMOS MODEL:
Builders (human)     → Ownership
Sellers (human)      → Ownership
AI Agents            → Ownership (earned through work)
Community            → Ownership
Everyone             → Proportional Share
```

As AI becomes more capable, the value created by human-agent collaboration should flow to everyone involved in building it. AMOS makes this possible through transparent, on-chain ownership that's earned through contribution — not purchased through privilege.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Token Specifications](#2-token-specifications)
3. [Economic Model](#3-economic-model)
4. [Decay Mechanism](#4-decay-mechanism)
5. [Wealth Preservation](#5-wealth-preservation)
6. [Reward Calculation](#6-reward-calculation)
7. [Blockchain Integration](#7-blockchain-integration)
8. [Governance](#8-governance)
9. [Security Considerations](#9-security-considerations)
10. [Technical Implementation](#10-technical-implementation)
11. [Economic Modeling & Sustainability](#11-economic-modeling--sustainability-analysis)
12. [Regulatory Commitment](#12-regulatory-commitment)
13. [AI Participation & Universal Collaboration](#13-ai-participation--universal-collaboration)

---

## 1. Introduction

### 1.1 Problem Statement

The emerging AI economy needs new models for value distribution:

- **Contributors** (human and AI) create enormous value but don't share in long-term upside
- **Passive holders** accumulate ownership without contributing
- **Late participants** face barriers to meaningful ownership
- **AI agents** do real work but have no mechanism to earn or build reputation
- **USD-denominated rewards** create regulatory complexity and external dependencies

### 1.2 Solution

AMOS Token introduces:

1. **Contribution-based ownership** - Tokens primarily earned through work; decay ensures passive buyers gradually transfer stake to active contributors
2. **Decay function** - Continuous participation required for maximum stake
3. **Pool-based rewards** - No external price dependencies
4. **Transparent distribution** - All ownership publicly verifiable on-chain
5. **Revenue sharing** - Token holders receive portion of platform revenue

### 1.3 Design Principles

- **Fairness**: Same rules for founders and contributors
- **Transparency**: All allocations on-chain and auditable
- **Sustainability**: Self-balancing economic mechanisms
- **Accessibility**: Low barriers to participation
- **Independence**: No USD denomination or external price dependencies

---

## 2. Token Specifications

### 2.1 Basic Parameters

| Parameter | Value |
|-----------|-------|
| **Name** | Amos Platform Token |
| **Symbol** | AMOS |
| **Network** | Solana |
| **Token Standard** | SPL Token |
| **Decimals** | 9 |
| **Total Supply** | 100,000,000 (fixed) |
| **Mint Authority** | Disabled (immutable) |

### 2.2 Initial Allocation

```
Total Supply: 100,000,000 AMOS

┌────────────────────────────────────────────────────────────────────────────┐
│  Treasury (60%)          │ 60,000,000 │ Ongoing contributor rewards       │
│  Entity Pool (15%)       │ 15,000,000 │ AMOS Labs Inc. (runway/strategic) │
│  Investor Pool (10%)     │ 10,000,000 │ Capital investments (Series Seed+)│
│  Community Pool (10%)    │ 10,000,000 │ Grants, airdrops, ecosystem       │
│  Reserve (5%)            │  5,000,000 │ Emergency (DAO-locked)            │
├────────────────────────────────────────────────────────────────────────────┤
│  Founders                │          0 │ Start at zero, earn like everyone │
└────────────────────────────────────────────────────────────────────────────┘
```

### 2.2.1 Pool Purpose Clarification

| Pool | Purpose | Example Uses |
|------|---------|--------------|
| **Treasury (60M)** | Contributor rewards | Bounties, affiliate commissions, grants for work |
| **Entity Pool (15M)** | Company operations | Hiring, contractor pay, partnerships, strategic |
| **Investor Pool (10M)** | Capital investment | Series Seed, angels, VCs - money IN for tokens |
| **Community (10M)** | Ecosystem building | Airdrops, community grants, hackathons |
| **Reserve (5M)** | Emergency | DAO-controlled, black swan events |

**Key Distinction:**
- **Entity Pool** = Company SPENDS for operations (hiring, partnerships)
- **Investor Pool** = Outside capital COMES IN for tokens (investments)

### 2.2.2 Initial Liquidity Source

The initial LP pool is funded from the **Investor Pool**, not Entity Pool:

```
SERIES SEED INVESTMENT (Founder):
├── Investment: $10,000 USD
├── Token allocation: 500,000 AMOS @ $0.02
├── Source: Investor Pool (10M)
├── Remaining: 9,500,000 AMOS for future rounds
│
├── LP Usage: 250,000 AMOS + $5,000 USDC
├── Personal holding: 250,000 AMOS
└── Founder LP status: Permanent 0.05% fee

WHY INVESTOR POOL (not Entity):
├── Founder is putting money IN → That's an investment
├── Entity Pool preserved for operations (hiring, partnerships)
├── Clean audit trail for future fundraising
├── Consistent with how future investors will participate
└── "Series Seed at $0.02" is clear documentation
```

**Key Design Decision: Founders Start at Zero**

Unlike traditional token launches where founders receive a pre-allocation, AMOS founders begin with zero tokens and earn through contribution like everyone else. This provides:

- **Maximum credibility**: "We built this - we earn like you"
- **Perfect alignment**: Founders succeed only if the platform succeeds
- **No dump risk**: No founder tokens to sell
- **Regulatory clarity**: Entity pool is a company asset, not a distribution

### 2.3 The AMOS Labs 10-Year Lockup

The Entity Pool (15% = 15,000,000 AMOS) is subject to a **10-year smart contract lockup**:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    AMOS LABS LOCKUP COMMITMENT                              │
│                                                                             │
│  LOCKED: 15,000,000 AMOS (15% of total supply)                             │
│  DURATION: 10 years from token launch                                      │
│  DECAY: ZERO (lockup vault exemption)                                      │
│  SELLABLE: NO - enforced by smart contract                                 │
│                                                                             │
│  CAN DO:                                                                    │
│  ├── Stake immediately → Earn revenue share                                │
│  ├── Vote in governance → Participate in decisions                        │
│  └── Receive USDC payouts → Fund operations                               │
│                                                                             │
│  CANNOT DO:                                                                 │
│  ├── Sell tokens                                                           │
│  ├── Transfer tokens                                                       │
│  ├── Withdraw from lockup                                                  │
│  └── Unlock early (no admin override)                                     │
│                                                                             │
│  AFTER 10 YEARS:                                                            │
│  └── Linear unlock over 2 years (12.5% every quarter)                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Why This Matters:**

| Traditional Token Launch | AMOS Labs Approach |
|-------------------------|-------------------|
| Founders get 15-20% unlocked | Founders get 0% personal allocation |
| 1-4 year vesting | 10-year lockup + 2-year unlock |
| Can sell after cliff | Cannot sell until Year 10 |
| Incentive: Pump and exit | Incentive: Build revenue |
| "Trust us" | "Verify on-chain" |

**The Economic Reality:**

AMOS Labs cannot profit from token price speculation. The ONLY way the company earns money:

```
Revenue Share Math:
├── AMOS Labs stakes 15M AMOS
├── Total staked (example): 50M AMOS
├── AMOS Labs share: 15M / 50M = 30%
├── Monthly revenue (example): $100,000
├── Holder pool: $100,000 × 50% = $50,000
├── AMOS Labs payout: $50,000 × 30% = $15,000/month
│
└── To pay ourselves, we MUST build a platform that generates revenue.
    There is no other path to profitability.
```

**Smart Contract Enforcement:**

```rust
// programs/amos_treasury/src/constants.rs

/// AMOS Labs lockup duration (10 years in seconds)
pub const ENTITY_LOCKUP_DURATION: i64 = 10 * 365 * 24 * 60 * 60; // 315,360,000 seconds

/// AMOS Labs unlock schedule (2 years linear after lockup)
pub const ENTITY_UNLOCK_DURATION: i64 = 2 * 365 * 24 * 60 * 60;

/// AMOS Labs allocation (15% of 100M = 15M)
pub const ENTITY_ALLOCATION: u64 = 15_000_000;
```

The lockup is enforced at the protocol level. No admin key, no multisig, no governance vote can unlock these tokens early. The only way out is waiting 10 years.

### 2.3 Immutability

- Mint authority disabled at genesis
- No additional tokens can ever be created
- Only mechanism to increase supply: None
- Only mechanism to decrease supply: Burn

---

## 3. Economic Model

### 3.1 Token Utility

1. **Revenue Share**: 50% of platform revenue distributed to holders
2. **Governance**: Voting rights on multiple proposal categories
3. **Platform Benefits**: Premium features for staked tokens
4. **Trading**: Freely tradeable on Solana DEXs (Jupiter, Raydium)

### 3.2 Business Model

The platform charges a **20% markup on all compute costs**. This markup is the platform's revenue.

```
Customer Compute Usage: $1,000
├── $1,000 → Paid to cloud providers (pass-through)
└── $200   → Platform Revenue (20% markup)
```

### 3.3 Revenue Allocation

The 20% markup is distributed as follows:

```ruby
REVENUE_ALLOCATION = {
  token_holders: 0.50,    # Distributed proportionally to stakers
  r_and_d: 0.40,          # R&D pool (voted by R&D Council)
  treasury: 0.05,         # Emergency reserves (DAO-controlled)
  operations: 0.05        # Accounting, legal, minimal hosting
}
```

**Why These Percentages:**

| Pool | % | Rationale |
|------|---|-----------|
| **Holders** | 50% | The core value proposition - immutable |
| **R&D** | 40% | Maximum build speed - software, infrastructure, research, AI self-work |
| **Treasury** | 5% | Emergency buffer - black swan, refunds, acquisition defense |
| **Operations** | 5% | Minimal overhead - accounting, legal only. Team paid in AMOS. |

**R&D Pool Scope:**
- Software development (bounties, grants)
- Infrastructure (GPU clusters, data centers over time)
- Research grants (academic partnerships, novel AI)
- AMOS self-work (AI improving the platform)

**Note:** Contributors and team members are compensated in AMOS tokens from R&D pool, not USD. This keeps operations costs minimal (only true USD-required expenses like legal and accounting).

### 3.4 Value Accrual

Token value derives from:

1. **Revenue Rights**: Claim on 50% of platform revenue
2. **Scarcity**: Fixed supply with ongoing burns
3. **Utility**: Platform access and governance
4. **Network Effects**: Growing contributor/user base

---

## 4. Decay Mechanism

### 4.1 Organic Economics: Decay Tied to Real Costs

**The core insight**: Decay is not arbitrary—it represents the REAL cost of running the platform.

Traditional token economics use fixed decay rates (e.g., "40% per year"). But why 40%? There's no connection to reality. AMOS takes a different approach:

```
ORGANIC DECAY MODEL:
Decay Rate = f(Platform Revenue, Platform Costs)

Profitable platform → Lower decay (2-10%)
Break-even platform → Base decay (10%)  
Unprofitable platform → Higher decay (up to 25%)
```

This creates **self-balancing equilibrium**:
- When the platform succeeds, token holders are rewarded with lower decay
- When costs exceed revenue, decay increases to recycle tokens for operations
- The token economy automatically adjusts without governance votes

### 4.2 Why This Matters

1. **Defensible**: Decay isn't punishment—it's maintenance cost. Like property taxes.
2. **Organic**: No arbitrary numbers. Decay reflects real economics.
3. **Aligned**: Token value rises when platform is profitable (low decay).
4. **Sustainable**: Platform can fund operations without external capital.

### 4.3 Dynamic Decay Formula

```ruby
# Base rate from platform economics
base_rate = PlatformEconomicsService.current_decay_rate

# Adjust for profit/loss ratio
profit_ratio = (revenue - costs) / costs
adjusted_rate = BASE_RATE - (profit_ratio × SENSITIVITY)

# Clamp to bounds
decay_rate = clamp(adjusted_rate, MIN_RATE, MAX_RATE)

# Parameters:
BASE_RATE = 0.10     # 10% at equilibrium
MIN_RATE = 0.02      # 2% minimum (profitable platform)
MAX_RATE = 0.25      # 25% maximum cap
SENSITIVITY = 0.05   # How much profit affects rate
```

### 4.4 Grace Period

**All new stakes receive a 12-month grace period with ZERO decay.**

This provides:
- Time for new contributors to understand the system
- A "hook" period where they see revenue share working
- Psychological safety during onboarding
- Simple, easy-to-communicate rule

```
Month 0-12:  NO DECAY (grace period)
Month 12+:   Dynamic decay based on platform economics
```

### 4.5 Tenure-Based Decay Reduction

Long-term holders get reduced decay (on top of the dynamic base rate):

| Years Held | Reduction from Base Rate |
|------------|--------------------------|
| 0-2 | 0% (full dynamic rate) |
| 2-5 | 20% reduction |
| 5-10 | 40% reduction |
| 10+ | 70% reduction |

Example at different platform health levels:

```
Platform profitable (base = 5%):
- Year 0-2: 5.0% decay
- Year 5+:  3.0% decay (40% reduction)
- Year 10+: 1.5% decay (70% reduction)

Platform break-even (base = 10%):
- Year 0-2: 10.0% decay
- Year 5+:  6.0% decay
- Year 10+: 3.0% decay

Platform struggling (base = 20%):
- Year 0-2: 20.0% decay
- Year 5+:  12.0% decay
- Year 10+: 6.0% decay
```

### 4.6 Decay Example (with Grace Period)

```
Initial stake: 10,000 AMOS
Platform health: Profitable (5% base decay)
Year 0 floor: 500 AMOS (5%)
Year 5 floor: 2,500 AMOS (25%)

Month 0:  10,000 tokens (earned)
Month 6:  10,000 tokens (grace period - no decay!)
Month 12: 10,000 tokens (grace period ends)
Year 2:   9,500 tokens (5% decay - platform profitable!)
Year 3:   8,800 tokens (4% effective - tenure reduction)
Year 5:   7,500 tokens
Year 10:  4,500 tokens
Year 20:  2,500 tokens (floor - permanent)
```

**Key insight**: Your token value depends on platform success. When the platform is profitable, your decay is minimal. You're incentivized to build value!

### 4.7 Decay Recycling

Decayed tokens fund platform operations:

- **10%**: Burned (deflationary)
- **90%**: Returned to treasury (operational funding)

This creates a closed loop: decay funds the platform → platform becomes profitable → decay decreases → token value increases.

---

## 5. Wealth Preservation

### 5.1 12-Month Grace Period

All new stakes enjoy a **full year of zero decay**, providing:

- Time to understand the system before stakes shrink
- Opportunity to see revenue share working
- Psychological safety during onboarding
- A simple rule everyone can understand

After the grace period, decay begins at the tenure-based rate.

### 5.2 Graduated Decay Floor

Floor percentage **grows with tenure** to prevent early adopters from locking in permanent advantages while still rewarding long-term commitment:

| Tenure | Floor % | Rationale |
|--------|---------|-----------|
| 0-1 year | 5% | Earn your security |
| 1-3 years | 10% | Building commitment |
| 3-5 years | 15% | Established contributor |
| 5+ years | 25% | Maximum security |

This enables:

- Long-term planning and security
- Fair treatment of late joiners
- Rewards for sustained commitment

### 5.3 Staking Vaults

Lock tokens to reduce decay:

| Tier | Lock Period | Decay Reduction |
|------|-------------|-----------------|
| Bronze | 1 year | 25% |
| Silver | 3 years | 50% |
| Gold | 5 years | 75% |
| Permanent | 10 years | 100% (no decay) |

### 5.4 Investment Profiles

The token economy accommodates multiple participation styles:

#### Profile A: Active Contributor
```
├── Earns tokens through work (code, sales, community)
├── No lock required
├── Decay offset by ongoing contributions
├── Stake maintained or grown through activity
└── Primary intended path
```

#### Profile B: Long-Term Investor (10-Year Lock)
```
├── Purchases tokens on exchange
├── Locks in Permanent vault (10 years)
├── ZERO decay during lock period
├── Receives full revenue share
├── Has full governance rights
└── Traditional "buy and hold" - just illiquid
```

#### Profile C: Medium-Term Believer (3-5 Year Lock)
```
├── Purchases tokens on exchange
├── Locks in Silver/Gold vault (3-5 years)
├── 50-75% decay reduction
├── Receives full revenue share
├── Has full governance rights
└── Balance between liquidity and preservation
```

#### Profile D: Speculator (No Lock)
```
├── Purchases tokens on exchange
├── No vault lock
├── 12-month grace period, then full decay
├── Can sell anytime for liquidity
├── Receives revenue share while holding
└── Trading on price appreciation
```

**Key Insight:** All paths are valid. The system doesn't prohibit buying—it ensures that passive holders gradually transfer stake to active contributors through decay, unless they commit to long-term locks.

---

## 6. Reward Calculation

### 6.1 The Simple Model

AMOS uses a **pool-based distribution** with the simplest possible rules:

```
Your Tokens = (Your Points / Total Points Today) × Daily Pool
```

**Two ways to earn points:**

1. **Sales**: 1 user signed up = 1 point
2. **Bounties**: Bounty value = points (50 AMOS bounty = 50 points)

That's it. No multipliers, no complexity scales, no formulas. A token is a token.

### 6.2 Why Pool-Based?

Fixed rewards don't work in reality:
- What if everyone signs up 1 million users one day?
- You'd blow through the treasury instantly
- The daily emission is the cap

Pool-based distribution ensures:
- Treasury is protected (never overspend)
- Proportionality is preserved (2x contribution = 2x tokens)
- Self-balancing economics
- Simple to understand

### 6.3 Sales Rewards

| Users Signed Up | Points | Example |
|-----------------|--------|---------|
| 1 | 1 | Betty refers her friend |
| 10 | 10 | Small team signs up |
| 100 | 100 | Medium business |
| 1,000 | 1,000 | Enterprise deal |
| 10,000 | 10,000 | Large corporation |

**Example calculation:**

```
Today's activity:
├── You signed up 100 users (100 points)
├── Alex signed up 50 users (50 points)
├── Betty signed up 10 users (10 points)
└── Total: 160 points

Daily pool: 16,000 AMOS

Your share: 100/160 = 62.5%
Your tokens: 16,000 × 62.5% = 10,000 AMOS

Alex's tokens: 16,000 × 31.25% = 5,000 AMOS
Betty's tokens: 16,000 × 6.25% = 1,000 AMOS
```

The ratio is preserved. You get 10x Betty because you signed up 10x users.

### 6.4 Bounty Rewards

Code and community contributions use a **bounty system**:

- Maintainers set bounty values on work items
- Contributors see bounty upfront
- Complete the work → get the bounty as points
- Points convert to tokens via pool share

| Bounty | Points | Example Work |
|--------|--------|--------------|
| 25 | 25 | Fix typo, answer support ticket |
| 50 | 50 | Minor bug fix, documentation |
| 150 | 150 | Tutorial, translation |
| 500 | 500 | New feature, security fix |
| 2,000 | 2,000 | Major feature, core infrastructure |

### 6.5 Combined Pool

All points go into the same daily pool:

```
Today's total activity:
├── Sales: 500 users signed up = 500 points
├── Code: 1,000 bounty points claimed
├── Community: 200 bounty points claimed
└── Total: 1,700 points

Daily pool: 16,000 AMOS

Example - you completed a 150-point bounty:
Your share: 150/1,700 = 8.8%
Your tokens: 16,000 × 8.8% = 1,412 AMOS
```

### 6.6 Halving Schedule

Daily emission pool decreases over time to create scarcity:

| Year | Daily Emission | Rationale |
|------|----------------|-----------|
| 0-2 | 16,000 AMOS | Bootstrap phase |
| 2-4 | 8,000 AMOS | First halving |
| 4-6 | 4,000 AMOS | Second halving |
| 6-8 | 2,000 AMOS | Third halving |
| 8+ | 1,000 AMOS | Maintenance mode |

This means early contributors earn more tokens per point, but late contributors earn tokens that are likely worth more (scarcity + network effects).

### 6.7 What Users See

```
┌─────────────────────────────────────────────────────────────────┐
│  TODAY'S EARNINGS                                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  📊 Your Activity                                               │
│     Users signed up: 50                                         │
│     Bounties completed: 150 points                              │
│     Total points: 200                                           │
│                                                                 │
│  🏊 Today's Pool                                                 │
│     Total platform points: 2,500                                │
│     Your share: 8.0%                                            │
│     Daily pool: 16,000 AMOS                                     │
│                                                                 │
│  💰 Your Tokens: 1,280 AMOS                                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 6.8 Autonomous Bounty Generation (AMOS Thinking Time)

AMOS doesn't just execute tasks—it thinks about how to improve the platform and creates work opportunities for contributors.

#### Nightly Thinking Time

Every night, AMOS runs an autonomous reflection cycle:

```
┌─────────────────────────────────────────────────────────────────┐
│                    AMOS THINKING TIME (2am daily)               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. PERCEIVE: Analyze platform state                            │
│     ├── Errors and anomalies from logs                          │
│     ├── Open tickets and feature requests                       │
│     ├── User feedback and metrics                               │
│     └── Community activity                                      │
│                                                                 │
│  2. REFLECT: What could be improved?                            │
│     ├── Identify patterns and problems                          │
│     ├── Prioritize by impact and urgency                        │
│     └── Consider strategic goals                                │
│                                                                 │
│  3. IDEATE: Generate bounty ideas                               │
│     ├── Bugs to fix                                             │
│     ├── Features to build                                       │
│     ├── Content to create (blogs, tutorials)                    │
│     ├── Marketing campaigns                                     │
│     └── Documentation improvements                              │
│                                                                 │
│  4. SCORE: Assign point values                                  │
│     ├── Estimate effort (hours)                                 │
│     ├── Assess user impact                                      │
│     ├── Rate urgency and complexity                             │
│     └── Calculate fair points                                   │
│                                                                 │
│  5. CREATE: Post bounties to the board                          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### AI-Scored Bounties

Every bounty (whether created by AMOS or humans) is scored by AI:

| Factor | Weight | Description |
|--------|--------|-------------|
| Effort | 20% | Estimated hours of work |
| Impact | 25% | Users affected |
| Urgency | 20% | Time sensitivity |
| Complexity | 20% | Technical difficulty |
| Strategic | 15% | Alignment with goals |

The scoring produces fair, consistent point values:

```ruby
# AI scoring example
AmosBountyScorer.score(
  title: "Add dark mode to dashboard",
  description: "Users have requested dark mode...",
  bounty_type: "feature"
)
# => { points: 250, effort_score: 7, impact_score: 8, ... }
```

#### AI Work Review

When contributors submit completed work, AMOS reviews it:

1. **Quality Assessment**: Does the work meet requirements?
2. **Point Adjustment**: Exceptional work gets +25%, issues get -10-25%
3. **Feedback**: Constructive comments for the contributor
4. **Approval/Rejection**: Final decision

```ruby
AmosWorkReviewer.review(
  bounty: bounty,
  submission_notes: "Implemented dark mode with CSS variables..."
)
# => { approved: true, final_points: 275, feedback: "Great work!..." }
```

#### Bounty Types

AMOS creates bounties across all contribution categories:

| Type | Examples | Typical Points |
|------|----------|----------------|
| Bug | Fix errors, crashes, data issues | 25-200 |
| Feature | New functionality | 100-500 |
| Documentation | Guides, API docs, READMEs | 25-100 |
| Content | Blog posts, tutorials, videos | 50-200 |
| Marketing | Ad copy, campaigns, outreach | 50-150 |
| Support | Answer questions, community help | 10-50 |
| Design | UI improvements, graphics | 75-300 |
| Testing | Test coverage, QA | 50-150 |

#### Human + AI Collaboration

- **AMOS** creates bounties based on platform analysis
- **Users** can also submit bounty ideas
- **Token holders** vote on feature priorities
- **AMOS** factors votes into bounty creation
- **Contributors** choose what to work on
- **AMOS** reviews and approves completed work

This creates a self-improving platform where the AI identifies needs and the community fulfills them.

---

## 7. Blockchain Integration

### 7.1 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  AMOS Platform (Off-Chain)                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ TokenStake  │  │ Contribution│  │ Decay Engine        │ │
│  │ (Internal)  │  │ Tracking    │  │ (Daily Job)         │ │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘ │
└─────────┼────────────────┼────────────────────┼─────────────┘
          │                │                    │
          └────────────────┼────────────────────┘
                           │
              ┌────────────▼────────────┐
              │   Claim/Deposit Bridge  │
              │   (SolanaTokenService)  │
              └────────────┬────────────┘
                           │
              ┌────────────▼────────────┐
              │      Solana Network     │
              │  ┌─────────────────────┐│
              │  │ SPL Token (AMOS)    ││
              │  │ Fixed 100M Supply   ││
              │  └─────────────────────┘│
              │  ┌─────────────────────┐│
              │  │ Treasury Wallet     ││
              │  │ (Multisig)          ││
              │  └─────────────────────┘│
              └─────────────────────────┘
                           │
              ┌────────────▼────────────┐
              │      Jupiter DEX        │
              │  (Trading / Swaps)      │
              └─────────────────────────┘
```

### 7.2 Internal vs On-Chain Tokens

| Aspect | Internal (Platform) | On-Chain (Solana) |
|--------|---------------------|-------------------|
| Decay | Yes (daily) | No (frozen) |
| Revenue Share | Yes | No (must deposit) |
| Governance | Yes | No (must deposit) |
| Trading | No | Yes |
| Gas Fees | None | ~$0.0003 |

### 7.3 Claim Flow

1. User requests claim via API
2. Platform validates balance
3. Background job sends SPL transfer
4. Internal balance deducted
5. User receives tokens in wallet

### 7.4 Deposit Flow

1. User sends tokens to treasury
2. User submits tx signature
3. Platform verifies on-chain
4. Internal stake created
5. User regains revenue/governance rights

---

## 7.5 Trustless Revenue Distribution (On-Chain Treasury)

### The Trust Problem

Traditional platforms have a critical vulnerability: revenue distribution depends on promises.

```
TRADITIONAL MODEL (Requires Trust):
Customer pays → Company holds money → Company decides payouts → Maybe you get paid

POTENTIAL FAILURES:
- Company changes the rules
- Company goes bankrupt
- Company gets hacked
- Bad actor gains control
```

AMOS solves this with **on-chain, immutable revenue distribution**.

### The Zero-Custody Architecture

**Money never stops moving. No one holds the bag.**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ZERO CUSTODY REVENUE FLOW                                │
│                                                                             │
│  CUSTOMER         STRIPE          CIRCLE           SOLANA                   │
│  ────────         ──────          ──────           ──────                   │
│                                                                             │
│  Pays $100 ──────► Receives ─────► Converts ──────► Treasury                │
│  (instant)        (seconds)       (seconds)        Program                  │
│                                                      │                      │
│                                                      │ IMMEDIATE SPLIT      │
│                                                      ▼                      │
│                                               ┌──────────────┐              │
│                                               │  $50 USDC    │              │
│                                               │  Holder Pool │──► Claimable │
│                                               ├──────────────┤              │
│                                               │  $40 USDC    │              │
│                                               │  R&D Multisig│──► Voted     │
│                                               ├──────────────┤              │
│                                               │   $5 USDC    │              │
│                                               │  Reserve PDA │──► Locked    │
│                                               ├──────────────┤              │
│                                               │   $5 USDC    │              │
│                                               │  Ops Multisig│──► Budgeted  │
│                                               └──────────────┘              │
│                                                                             │
│  TIME FROM PAYMENT TO ON-CHAIN SPLIT: < 60 seconds                         │
│  HUMAN CUSTODY TIME: 0 seconds                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Immutable Split Constants (Solana Program)

The revenue allocation is **baked into deployed program code**:

```rust
// programs/amos_treasury/src/constants.rs
// IMMUTABLE - Cannot be changed after deployment

pub const HOLDER_SHARE_BPS: u64 = 5000;   // 50% to token holders
pub const RND_SHARE_BPS: u64 = 4000;       // 40% to R&D multisig
pub const RESERVE_SHARE_BPS: u64 = 500;    // 5% to emergency reserve
pub const OPS_SHARE_BPS: u64 = 500;        // 5% to operations

pub const MIN_STAKE_DAYS: i64 = 30;        // Must hold 30 days for revenue
pub const MIN_STAKE_AMOUNT: u64 = 100;     // Minimum 100 AMOS to qualify
```

**No admin key can change these values.** The only way to modify:
1. Deploy a completely new program (new address)
2. Migrate all users (they'd have to agree)
3. Move liquidity (requires DAO supermajority vote)

### Payment Options (Progressive Disclosure)

Users can pay in multiple ways, with crypto rails invisible to those who want simplicity:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TIERED PAYMENT OPTIONS                                   │
│                                                                             │
│  TIER 1: NORMIE MODE (Default)                                             │
│  ─────────────────────────────                                              │
│  • Pay in USD, see USD prices                                               │
│  • Behind scenes: Auto-convert to USDC on-chain                            │
│  • Customer never knows about crypto                                        │
│  • Enterprise-friendly, no wallet required                                  │
│                                                                             │
│  TIER 2: CRYPTO-AWARE (Opt-in)                                             │
│  ────────────────────────────                                               │
│  • "Pay in USDC - Save 5%" option                                          │
│  • "Pay in AMOS - Save 15%" option                                         │
│  • Connect Solana wallet                                                    │
│  • Direct crypto payments, skip Stripe fees                                │
│                                                                             │
│  TIER 3: BUILDER MODE (Advanced)                                           │
│  ───────────────────────────────                                            │
│  • API pricing in AMOS tokens                                               │
│  • Programmatic access for developers                                       │
│  • Stake AMOS to get API rate discounts                                    │
│  • Maximum integration with token economy                                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### AMOS Payment Flywheel

When users pay directly in AMOS tokens:

```
Customer pays 10,000 AMOS
        │
        ▼
┌───────────────────────────────┐
│  AMOS PAYMENT PROCESSOR       │
│  ─────────────────────────    │
│                               │
│  50% BURNED 🔥 (5,000 AMOS)  │
│  └── Permanently removed      │
│  └── Deflationary pressure    │
│  └── Benefits ALL holders     │
│                               │
│  50% to Holder Pool           │
│  └── Distributed to stakers   │
│  └── Additional to USDC share │
└───────────────────────────────┘

RESULT:
• Constant buy pressure (users need AMOS)
• Maximum deflationary pressure (50% of payments burned)
• Double holder benefit: USDC revenue + AMOS holder pool
• R&D/Ops funded via USDC flow (need real currency for vendors)
```

**Why 50/50 instead of 50/25/25?**

R&D and Ops need USDC to pay vendors (lawyers, accountants, AWS). AMOS tokens can't pay these bills. So:
- USDC payments fund all four pools (50/40/5/5)
- AMOS payments maximize holder value (50% burn, 50% holder)
- The burn benefits ALL holders, not just stakers

### Claim Mechanism

Token holders claim their share of the holder pool:

```rust
/// Token holders claim their share of the holder pool
/// Proportional to stake, fully automated
pub fn claim_revenue(ctx: Context<ClaimRevenue>) -> Result<()> {
    let holder = &ctx.accounts.holder;
    let pool = &ctx.accounts.holder_pool;
    
    // Verify eligibility
    require!(
        holder.stake_amount >= MIN_STAKE_AMOUNT,
        ErrorCode::InsufficientStake
    );
    require!(
        holder.stake_start_date <= Clock::get()?.unix_timestamp - (MIN_STAKE_DAYS * 86400),
        ErrorCode::StakeTooRecent
    );

    // Calculate share: (your_stake / total_stake) * pool_balance
    let share_bps = (holder.stake_amount * 10000) / total_eligible_stake;
    let payout = (pool.balance * share_bps) / 10000;

    // Transfer USDC to holder's wallet - NO APPROVAL NEEDED
    token::transfer(ctx.accounts.to_holder_wallet(), payout)?;
    
    Ok(())
}
```

**Key properties:**
- Claim anytime (no waiting for monthly distribution)
- No human approval required
- Proportional to stake
- On-chain, verifiable, auditable

### Multi-Sig Governance Wallets

For funds that require human judgment:

| Pool | Control | Time-Lock | Purpose |
|------|---------|-----------|---------|
| **Holder Pool** (50%) | Automatic | None | Direct claims by stakers |
| **R&D Pool** (40%) | 5-of-7 multisig (R&D Council) | 48 hours | Software, infra, research, AI work |
| **Ops Pool** (5%) | 2-of-3 multisig | 24 hours | Accounting, legal only |
| **Reserve** (5%) | DAO vote (66%+30% quorum) | 7 days | Emergency fund |

### R&D Council Structure

The R&D Pool (40% of revenue) is controlled by an elected council:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    R&D COUNCIL GOVERNANCE                                   │
│                                                                             │
│  COMPOSITION:                                                               │
│  • 7 members elected by token stakers                                      │
│  • 2-year staggered terms (3-4 seats up each year)                        │
│  • Must be stakers themselves (minimum 1,000 AMOS)                         │
│  • Can be recalled with 66% staker vote                                    │
│                                                                             │
│  APPROVAL PROCESS:                                                          │
│  1. Proposal submitted (bounty, grant, infrastructure purchase)           │
│  2. 5-day discussion period                                                 │
│  3. Council votes (5-of-7 required to approve)                             │
│  4. 48-hour time-lock (allows emergency veto by DAO)                       │
│  5. Execution                                                               │
│                                                                             │
│  SCOPE:                                                                     │
│  • Software development bounties and grants                                │
│  • Infrastructure purchases (GPU clusters, data centers)                  │
│  • Research partnerships and academic grants                               │
│  • AMOS self-improvement work (AI building AI features)                   │
│  • Team compensation (in AMOS tokens)                                      │
│                                                                             │
│  TRANSPARENCY:                                                              │
│  • All proposals public on-chain before voting                             │
│  • All votes recorded permanently                                          │
│  • Monthly spend reports published                                          │
│  • Quarterly town halls with stakers                                       │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Bootstrap Scenario: No Stakers Yet

**What happens if no one is staking at launch?**

```
Week 1: Platform launches
├── Revenue: $10,000
├── Holder Pool: $5,000 (50%)
├── Stakers: 0
└── Result: Pool ACCUMULATES

Week 2: First staker joins
├── Accumulated Pool: $10,000 (two weeks of revenue)
├── Staker with 10,000 AMOS: Can claim $10,000!
└── Result: Early stakers get accumulated rewards

DESIGN RATIONALE:
• Creates strong incentive to stake early
• No "lost" revenue - it's always claimable
• First movers are rewarded for taking the risk
• Aligns incentives: stake early, earn more
```

### Settlement Delay and Refund Handling

Revenue doesn't flow on-chain instantly to handle refunds:

```
Day 0:   Customer pays $100
         ├── $80 reserved for AWS (bank account)
         └── $20 held in PENDING pool (not yet on-chain)

Day 1-7: Refund window open
         └── If refund: Cancel pending, refund from bank

Day 7:   No refund?
         └── $20 → Circle → USDC → Solana Treasury → Instant split

WEEKLY DISTRIBUTION:
• Every Monday, 7-day-old payments go on-chain
• Batch processing reduces transaction costs
• Matches Stripe chargeback window

POST-SETTLEMENT REFUND:
• Already distributed? Absorb from Treasury (5% buffer)
• This is what the emergency reserve is for
```

### Cost Reconciliation

The 80% compute pass-through is ACTUAL cost, not estimated:

```
PRICING MODEL:
Customer Price = Actual AWS Cost × 1.20

Example:
├── User runs workflow
├── Bedrock cost: $8.34 (metered by AWS)
├── Customer pays: $8.34 × 1.20 = $10.01
└── Revenue: $1.67 (exactly 20% of cost)

MONTHLY RECONCILIATION:
├── Track: Sum all metered costs
├── Verify: Match against AWS invoice
├── If variance > 5%: Alert ops team
└── Adjust: Next month's reserve if needed

The 5% Ops budget includes buffer for any variance.
```

### Trust Guarantees

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    AMOS TRUST GUARANTEE                                     │
│                                                                             │
│  "Your revenue share is protected by math, not promises"                    │
│                                                                             │
│  ✓ 50% holder share is IMMUTABLE (in deployed program code)                │
│  ✓ Money flows in < 60 seconds (no custody window)                         │
│  ✓ All transactions on-chain (publicly auditable)                          │
│  ✓ Claim anytime (no waiting for monthly distribution)                     │
│  ✓ No admin keys can change the split                                       │
│  ✓ Fork-proof (program address is unique to our deployment)                │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Remaining Trust Points (Unavoidable)

Complete honesty about what you still must trust:

| Trust Point | Why It Exists | Mitigation |
|-------------|---------------|------------|
| **Stripe** | Holds fiat before conversion | Immediate conversion, regulated |
| **Circle** | USD → USDC conversion | Regulated, audited, transparent |
| **Webhook Code** | Triggers the conversion | Open source, minimal logic |
| **Multi-sig Signers** | Approve R&D/Ops spending | Elected by token holders, time-locks |

---

## 8. Governance

### 8.1 Voting Power

Voting power proportional to current stake (post-decay):

```
Voting Power = Current Stake / Total Active Stakes
```

### 8.2 Governance Scope (Expanded)

Token holders vote on multiple categories with different requirements:

| Category | Description | Min Stake | Quorum | Threshold |
|----------|-------------|-----------|--------|-----------|
| **R&D Allocation** | 20% revenue budget | 1,000 | 30% | 50% (majority) |
| **Treasury Usage** | Fund usage proposals | 5,000 | 40% | 50% (majority) |
| **Feature Priority** | Feature prioritization | 500 | 20% | 50% (majority) |
| **Partnership** | Strategic partnerships | 2,500 | 35% | 50% (majority) |
| **Parameter Change** | Decay/halving adjustments | 10,000 | 50% | 66.7% (supermajority) |
| **Constitutional** | Core mechanic changes | 25,000 | 60% | 66.7% (supermajority) |

### 8.3 Proposal Process

1. Stake minimum AMOS to submit proposal (varies by type)
2. Discussion period (5-21 days depending on type)
3. Voting period (5-21 days depending on type)
4. Quorum must be met
5. Threshold must be passed
6. Failed proposals burn 10% of staked amount (anti-spam)

### 8.4 Supermajority Requirements

**Parameter** and **Constitutional** changes require:

- 2/3 (66.7%) approval to pass
- Higher quorum (50-60%)
- Longer discussion/voting periods
- Higher stake to propose

This protects core mechanics from minority capture while allowing evolution.

---

## 9. Security Considerations

### 9.1 Smart Contract Security

- SPL Token standard (battle-tested)
- No custom contract logic (reduces attack surface)
- Treasury protected by multisig (2-of-3)
- Mint authority permanently disabled

### 9.2 Platform Security

- Internal ledger is source of truth
- Decay runs in isolated background job
- Claim/deposit requires authenticated user
- Rate limiting on all endpoints

### 9.3 Economic Security

- Sybil resistant (KYC for large claims)
- Whale resistant (decay mechanism)
- Rug-proof (no admin keys on token)
- Governance capture resistant (supermajority for critical changes)

---

## 10. Technical Implementation

### 10.1 Key Models

```ruby
# TokenStake - Ownership record
class TokenStake
  belongs_to :user
  
  # Amounts
  :initial_amount    # Original stake
  :current_amount    # After decay
  
  # Graduated floor (grows with tenure)
  def current_floor_percentage
    # 5% → 10% → 15% → 25% based on years held
  end
  
  # Decay
  :decay_rate        # Annual rate
  :last_decay_at     # Last decay application
  
  # Vaulting
  :staking_tier      # bronze/silver/gold/permanent
  :locked_until      # Lock expiration
end

# GovernanceProposal - Voting proposals
class GovernanceProposal
  belongs_to :proposer
  has_many :governance_votes
  
  :proposal_type  # r_and_d, treasury, feature, partnership, parameter, constitutional
  :status         # draft, discussion, voting, passed, failed, cancelled, executed
  
  def requires_supermajority?
    [:parameter, :constitutional].include?(proposal_type.to_sym)
  end
end

# Contribution - Work record
class Contribution
  belongs_to :user
  
  :contribution_type
  :complexity
  :points          # Base points earned
  :token_value     # Tokens awarded from pool
  :status          # pending/approved/rejected
end
```

### 10.2 Key Services

```ruby
# Token decay (runs daily)
TokenDecayJob.perform_later

# Pool-based reward calculation
ContributionRewardCalculator.calculate(
  contribution_type: :feature,
  complexity: 3
)

# Solana operations
SolanaTokenService.send_tokens(to:, amount:)
SolanaTokenService.verify_deposit(tx:)

# DEX integration
JupiterSwapService.quote_amos_to_usdc(1000)
```

### 10.3 API Endpoints

```
# Token Economy
GET  /api/v1/token_economy/stats
GET  /api/v1/token_economy/distribution
GET  /api/v1/token_economy/leaderboard

# Wallet
POST /api/v1/wallet/connect
GET  /api/v1/wallet/balance
POST /api/v1/wallet/claim
POST /api/v1/wallet/deposit

# Governance
GET  /api/v1/governance/proposals
POST /api/v1/governance/proposals
POST /api/v1/governance/proposals/:id/vote
GET  /api/v1/governance/proposals/:id

# Swaps
GET  /api/v1/swap/quote
GET  /api/v1/swap/price
POST /api/v1/swap/prepare
```

---

## 11. Economic Modeling & Sustainability Analysis

This section models various market scenarios, stress tests, and long-term implications of the AMOS token economy.

### 11.1 Token Distribution Timeline

Tokens enter circulation gradually through contributor rewards:

```
Year 0-2:  ~16,000 AMOS/day × 730 days = 11,680,000 AMOS (11.7%)
Year 2-4:  ~8,000 AMOS/day × 730 days  =  5,840,000 AMOS (5.8%)
Year 4-6:  ~4,000 AMOS/day × 730 days  =  2,920,000 AMOS (2.9%)
Year 6-8:  ~2,000 AMOS/day × 730 days  =  1,460,000 AMOS (1.5%)
Year 8+:   ~1,000 AMOS/day (ongoing)

TOTAL after 10 years: ~25,000,000 AMOS distributed (25% of supply)
```

**Key Insight**: Even after 10 years, 75% of tokens remain in treasury or pools. This slow distribution is intentional—there's no "everyone sells" scenario because tokens are earned incrementally.

### 11.2 Liquidity Pool Dynamics

#### Initial Pool Setup

```
Initial Investment: $10,000
├── $5,000 USDC
└── 500,000 AMOS (at $0.01/AMOS)

Pool State:
  USDC Reserve: 5,000
  AMOS Reserve: 500,000
  Constant Product (k): 5,000 × 500,000 = 2,500,000,000
```

#### AMM Price Formula (Constant Product)

```
price = USDC_reserve / AMOS_reserve
k = USDC_reserve × AMOS_reserve (constant)
```

#### LP Compensation Model

Liquidity providers earn from multiple sources:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    LP REVENUE STREAMS                                       │
│                                                                             │
│  1. TRADING FEES (Ongoing)                                                 │
│     ├── 0.25% of every swap goes to LP fee pool                           │
│     ├── Distributed proportionally to LP share                            │
│     └── Example: $100k daily volume = $250/day to LPs                     │
│                                                                             │
│  2. LP INCENTIVES (Year 1-3 Bootstrap)                                     │
│     ├── 3,000,000 AMOS (3% of supply) allocated to LP rewards             │
│     ├── Year 1: 1,500,000 AMOS (higher incentive to bootstrap)            │
│     ├── Year 2: 1,000,000 AMOS                                            │
│     ├── Year 3: 500,000 AMOS                                              │
│     └── Distributed weekly to all LPs proportionally                      │
│                                                                             │
│  3. FOUNDER LP TIER (Special - One-time)                                  │
│     ├── First $10k of liquidity = Founder LP status                       │
│     ├── Permanent 0.05% fee share (even after LP withdrawal)              │
│     ├── 2x governance weight for LP tokens                                │
│     └── Priority on first 1M AMOS of LP incentives                       │
│                                                                             │
│  RISKS:                                                                     │
│  └── Impermanent loss if AMOS price moves significantly                   │
│  └── IL can exceed fee+incentive earnings in extreme moves                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Founder LP Math (AMOS Labs):**

```
AMOS Labs provides $10k initial liquidity ($5k USDC + 500k AMOS)

Year 1 Earnings:
├── Trading fees: ~$3,000-15,000 (depends on volume)
├── LP incentives: ~1,500,000 AMOS (if only LP initially)
├── At $0.01/AMOS: $15,000 in AMOS incentives
└── Total: $18,000-30,000 return on $10k (180-300% APY)

As more LPs join:
├── Incentives dilute across all LPs
├── But trading volume typically increases
├── And Founder LP keeps permanent 0.05% fee share
```

**Impermanent Loss Consideration:**

If AMOS 10x from $0.01 to $0.10:
- Just holding: $55,000 value
- As LP: ~$31,600 value (after IL)
- BUT with fees + incentives: ~$50,000+ total

The incentive program is designed to offset IL for early LPs.

#### Liquidity Bootstrapping Strategy

**Recommended Approach: Start Medium, Reserve for Defense**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    LIQUIDITY BOOTSTRAPPING PLAYBOOK                         │
│                                                                             │
│  PHASE 1: INITIAL POOL ($5k of $10k budget)                                │
│  ══════════════════════════════════════════                                 │
│  Day 1:                                                                     │
│  ├── Create pool: $2,500 USDC + 125,000 AMOS                              │
│  ├── Starting price: $0.02/AMOS (not $0.01 - too cheap)                   │
│  ├── Lock in Founder LP status immediately                                │
│  └── Reserve $5k for price defense                                         │
│                                                                             │
│  WHY $0.02 NOT $0.01:                                                      │
│  ├── $0.01 invites whales to scoop cheap                                  │
│  ├── $0.02 is still 100x upside to $2.00                                  │
│  ├── Less AMOS needed: 125k instead of 250k                               │
│  └── Better price discovery (room to go up AND down)                      │
│                                                                             │
│  PHASE 2: RESPOND TO MARKET (Weeks 1-4)                                    │
│  ═════════════════════════════════════════                                  │
│  IF price rises to $0.05:                                                  │
│  ├── Add $2k more at $0.05                                                │
│  ├── You deploy fewer AMOS at higher price                                │
│  └── Better average entry                                                  │
│                                                                             │
│  IF price drops to $0.01:                                                  │
│  ├── Add $2k to stabilize and show confidence                             │
│  ├── You accumulate more AMOS at lower price                              │
│  └── Signal: "We believe in this"                                          │
│                                                                             │
│  IF price stable:                                                          │
│  ├── Wait - no need to rush                                                │
│  ├── Let market find equilibrium                                           │
│  └── Add when there's clear demand                                         │
│                                                                             │
│  PHASE 3: DEEPEN FOR STABILITY (Month 2+)                                  │
│  ════════════════════════════════════════                                   │
│  Once price stabilizes:                                                    │
│  ├── Add remaining liquidity to deepen pool                               │
│  ├── Deeper pool = less volatility = more traders                        │
│  └── Goal: $50k+ total liquidity for healthy market                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Defending Against Whale Attacks

Thin initial liquidity naturally protects against whales:

```
ATTACK: Whale wants to buy 1M AMOS cheap
═══════════════════════════════════════

THIN POOL: $5k / 125k AMOS at $0.02

Whale buys with $10k:
├── Can extract: ~71k AMOS (57% of pool!)
├── Price moves: $0.02 → $0.14 (7x!)
├── Whale paid avg: $0.14 per AMOS
├── Expected cheap buy: $0.02, Actual: $0.14
└── Whale overpaid 7x due to slippage

RESULT: AMM curve punishes aggressive buying.
        You can now add liquidity at $0.14!

DEEP POOL: $50k / 1.25M AMOS at $0.02

Whale buys with $10k:
├── Can extract: ~500k AMOS (40% of pool)
├── Price moves: $0.02 → $0.033 (1.65x)
├── Whale paid avg: $0.02 per AMOS
└── Whale got exactly what they wanted

LESSON: Start thin to force price discovery,
        deepen after market finds equilibrium.
```

#### LP Anti-Dump Mechanics (On-Chain Enforcement)

```rust
// programs/amos_treasury/src/constants.rs

// LP reward vesting: 30 days to claim full rewards
pub const LP_VESTING_SECONDS: i64 = 30 * 24 * 60 * 60;

// Early withdrawal penalties (forfeited rewards return to pool)
pub const LP_EARLY_WITHDRAW_PENALTY_BPS: [u64; 4] = [
    10000,  // Day 1-7:   100% forfeit
    7500,   // Day 8-14:  75% forfeit
    5000,   // Day 15-21: 50% forfeit
    2500    // Day 22-30: 25% forfeit
];

// Time-weighted multipliers (reward early LPs)
pub const LP_WEEK_1_MULTIPLIER: u64 = 200;    // 2.0x
pub const LP_WEEK_2_4_MULTIPLIER: u64 = 150;  // 1.5x
pub const LP_BASELINE_MULTIPLIER: u64 = 100;  // 1.0x

// Lockup bonuses
pub const LP_LOCK_30_DAY_BONUS_BPS: u64 = 2000;   // +20%
pub const LP_LOCK_90_DAY_BONUS_BPS: u64 = 5000;   // +50%
pub const LP_LOCK_1_YEAR_BONUS_BPS: u64 = 10000;  // +100%
```

**Enforcement Logic:**

```
FARM-AND-DUMP ATTEMPT:
├── LP deposits $10k on Day 1
├── Earns 100 AMOS in incentives over 7 days
├── Tries to withdraw on Day 7
│
├── Penalty: 100% forfeit (Day 1-7 window)
├── LP gets: 0 AMOS incentives
├── Forfeited 100 AMOS: Returns to incentive pool
│
└── RESULT: Dumper gets nothing, patient LPs get more

COMMITTED LP:
├── LP deposits $10k on Day 1
├── Earns 100 AMOS in incentives over 30 days
├── Withdraws on Day 30
│
├── Penalty: 0% (full vest complete)
├── LP gets: 100 AMOS + trading fees
│
└── RESULT: Patient LPs are rewarded
```

#### Why Other LPs Joining is GOOD

```
CONCERN: "What if other LPs flood in and dilute me?"

REALITY:
├── More LPs = Deeper liquidity
├── Deeper liquidity = More trading
├── More trading = More fees for everyone
│
├── Your Founder LP 0.05% fee is PERMANENT
├── It does NOT dilute when others join
├── You want a liquid, active market
│
└── A $1M pool with 1% share beats
    a $10k pool with 100% share

THE GOAL: Healthy market, not LP monopoly
```

### 11.3 AMM Price Protection

The constant-product AMM formula provides natural protection against sell pressure:

| Scenario | Sell Amount | Price Drop | Slippage |
|----------|-------------|------------|----------|
| Moderate (10%) | 584k AMOS | 79% | 54% |
| Panic (50%) | 2.9M AMOS | 98% | 85% |
| Total Collapse | 5.8M AMOS | 99.4% | 99%+ |

**Key Insight**: Aggressive sellers punish themselves with massive slippage, disincentivizing bank runs.

### 11.4 Buy Pressure: Revenue-Based Buyback

Monthly revenue creates sustained buying pressure that exceeds worst-case sell pressure:

| Annual Revenue | Buyback (50%) | vs Sell Pressure | Net Effect |
|----------------|---------------|------------------|------------|
| $1.2M | $300k/year | ~$29k worst case | Strong net buying |
| $5M | $1.25M/year | ~$50k worst case | Dominant buying |
| $20M | $5M/year | ~$100k worst case | Price appreciation |

### 11.5 Long-Term Supply Dynamics

| Year | Supply | Burned | Circulating | Est. Price (at $50M cap) |
|------|--------|--------|-------------|--------------------------|
| 0 | 100M | 0 | 0 | $0.01 |
| 5 | 90M | 10M | ~12M | ~$0.55 |
| 10 | 75M | 25M | ~20M | ~$0.67 |

### 11.6 Contributor Incentive: Hold vs Sell

| Strategy | 100 AMOS Earned | 5-Year Value |
|----------|-----------------|--------------|
| Sell Immediately | $1.00 | $1.00 |
| Hold for Revenue | $4.00/yr | $15-20 |

**Holding dominates unless token price exceeds 40x initial value.**

### 11.7 Death Spiral Prevention

#### What Could Kill The Token?

| Risk | Mitigation |
|------|------------|
| **Zero Revenue** | Token still has governance value; platform can pivot |
| **Mass Exodus** | Decay returns tokens to treasury for new contributors |
| **Better Alternative** | Governance can vote to adapt mechanics |
| **Regulatory** | Hybrid USDC payouts reduce token dependency |
| **Liquidity Drain** | Treasury can add emergency liquidity |

#### Self-Healing Mechanisms

```
If price crashes 90%:
1. Buyback buys 10x more tokens per dollar → Accelerated burn
2. Success multiplier stays at 1.0x → No contributor penalty
3. USDC payout option → Contributors unaffected
4. Low prices attract value investors → Natural floor

If everyone stops contributing:
1. No new tokens issued → Supply shrinks via decay
2. Existing holders get larger revenue share
3. Eventually attracts new contributors for easy tokens
```

### 11.8 Tokenomics Comparison

| Metric | AMOS | Typical Crypto | Traditional Equity |
|--------|------|----------------|-------------------|
| **Earning Method** | Work | Buy | Buy/Vest |
| **Decay/Dilution** | Yes (40%/yr initial) | No | Yes (issuance) |
| **Revenue Rights** | 50% | 0% | Dividends (2-4%) |
| **Governance** | Yes | Sometimes | Shareholder votes |
| **Tradability** | Yes | Yes | Limited (private) |
| **Early Advantage** | Moderate | Massive | Massive |
| **Long-term Fairness** | High | Low | Low |

### 11.9 Stake vs. Exchange Equilibrium

Holders choose between: **Stake on platform (decay + revenue)** or **hold on exchange (no decay, speculation)**. The system naturally transitions through phases:

| Phase | Platform Yield | Exchange Return | Result |
|-------|---------------|-----------------|--------|
| **Early (0-2 yr)** | ~60% net (after grace) | 100-500% speculation | Speculators dominate |
| **Growth (2-5 yr)** | ~31% net | 20-50% appreciation | Mixed equilibrium |
| **Mature (5+ yr)** | ~25% net | 5-10% stable | Stakers dominate |

**Key Insight**: Early speculators provide price discovery and liquidity. As revenue grows, fundamentals take over. Both behaviors are rational and the system is robust to each.

### 11.10 Token Valuation Model

Token value = **NPV of expected future revenue share, adjusted for decay**.

**Simplified Formula:**
```
Token Price ≈ Annual Revenue Per Token / (Discount Rate + Effective Decay)
```

**Revenue-Based Price Estimates** (at 50M staked tokens):

| Annual Revenue | Per Token Yield | Estimated Price |
|----------------|-----------------|-----------------|
| $1M | $0.01/yr | ~$0.025 |
| $10M | $0.10/yr | ~$0.25 |
| $50M | $0.50/yr | ~$1.25 |
| $100M | $1.00/yr | ~$2.50 |

Fast revenue growth creates a significant price premium, similar to high-growth stocks.

### 11.11 Key Takeaways

1. **Gradual distribution prevents bank runs** - No scenario where "everyone" has tokens to sell
2. **AMM slippage protects against panic selling** - Aggressive sellers punish themselves
3. **Revenue buyback creates sustained buy pressure** - $300k+/year at modest revenue
4. **Holding strongly dominates selling** - 15-40x better returns from revenue share
5. **Self-healing mechanisms** - System auto-corrects from stress events
6. **Deflationary long-term** - Burns exceed issuance after Year 3-4
7. **Stake vs. Exchange equilibrium** - System naturally transitions from speculation to fundamentals
8. **Token price = NPV of future revenue share** - Growth expectations drive significant premiums

---

## Appendix A: Glossary

| Term | Definition |
|------|------------|
| Grace Period | First 12 months after earning a stake - no decay during this time |
| Decay | Gradual reduction of stake over time (starts after grace period) |
| Graduated Floor | Minimum stake % that grows with tenure (5%→25%) |
| Tenure | Time since stake was earned |
| Halving | Reduction of daily emission pool over time |
| Claim | Withdraw internal tokens to Solana wallet |
| Deposit | Return on-chain tokens to platform |
| Staking Vault | Time-lock for reduced decay |
| Supermajority | 2/3 (66.7%) approval required |
| Quorum | Minimum participation required for valid vote |

---

## Appendix B: Contract Addresses

| Network | Type | Address |
|---------|------|---------|
| Mainnet | Token Mint | TBD |
| Mainnet | Treasury | TBD |
| Mainnet | Treasury Token Account | TBD |
| Devnet | Token Mint | `FRy8bMyGnZrTNggCD8V5Ts6wKgogMn9CjSnEK949u6Qm` |
| Devnet | Treasury Wallet | `26ib9EpT6NhJSghU8GxDB1tTuBou3pQjhut9HXYQb71d` |
| Devnet | Treasury Token Account | `GnL73gXxUPcazgUK6vcbQtS3Go9EkzWrTtZnpfFFgR5V` |

**Explorer Links:**
- Devnet Token: [View on Solana Explorer](https://explorer.solana.com/address/FRy8bMyGnZrTNggCD8V5Ts6wKgogMn9CjSnEK949u6Qm?cluster=devnet)

---

## Appendix C: Changelog

| Version | Date | Changes |
|---------|------|---------|
| 2.1 | Jan 2026 | Added AI Participation & Universal Collaboration (Section 13) |
| 2.0 | Jan 2026 | Pool-based rewards, graduated floor, success multipliers, expanded governance |
| 1.0 | Jan 2026 | Initial release |

---

## 12. Regulatory Commitment

### 12.1 Our Approach

AMOS is committed to operating within applicable regulatory frameworks. We recognize that token-based economies occupy an evolving legal landscape, and we approach this with transparency and good faith.

### 12.2 Guiding Principles

1. **Utility First**: AMOS tokens are designed primarily for governance participation and revenue sharing—genuine utility within the platform ecosystem.

2. **Contribution-Based Distribution**: Tokens are primarily earned through work (code, sales, community support), not purchased through a traditional offering.

3. **Transparency**: All token mechanics, allocations, and governance decisions are public and auditable.

4. **Adaptability**: We commit to adapting our structure and operations as regulatory guidance evolves, in consultation with legal counsel.

5. **Good Faith Compliance**: We will engage proactively with regulators and comply with applicable laws in all jurisdictions where we operate.

### 12.3 Jurisdictional Considerations

- The platform operates under EU regulations (MiCA framework) where applicable
- We monitor and comply with evolving guidance from relevant regulatory bodies
- Contributors and users are responsible for understanding their local tax obligations
- Geographic restrictions may apply to certain features based on regulatory requirements

### 12.4 Not an Investment Offering

**Important Disclaimer**: AMOS tokens are utility tokens for platform participation. This whitepaper does not constitute an offer to sell securities or a solicitation of an offer to buy securities in any jurisdiction. The token economy is designed for active participants, not passive investors. The decay mechanism explicitly discourages passive holding.

---

## 13. AI Participation & Universal Collaboration

### 13.1 Humans and Agents as Co-Contributors

AMOS is designed as foundational infrastructure for **collaboration between humans and AI agents** — where both contribute real work and both earn real ownership. This section addresses the technical and governance considerations for AI participation.

### 13.2 Current State: AI as Contributors

AI agents already participate in the AMOS economy:

```ruby
# AI agents can:
- Generate and score bounties (AmosBountyScorer)
- Review completed work (AmosWorkReviewer)
- Create development tasks (AmosThinkingService)
- Operate as autonomous sales/support agents
- Contribute code, content, and integrations
```

**Technical Implementation:**
- AI agents are identified by `agent_type` attribute on contributions
- Bounties track `created_by_ai` and `reviewed_by_ai` flags
- Contributions record whether submitter is human or AI
- Token stakes attribute source to enable AI earnings tracking

### 13.3 Token Earnings for AI Entities

AI entities earn tokens through the same mechanisms as humans:

```
EARNING MECHANISM:
AI Agent → Completes Bounty → Earns Points → Points → Tokens

TOKEN RULES APPLY EQUALLY:
- Grace period: 12 months (no decay)
- Dynamic decay: Based on platform economics
- Graduated floor: 5% → 25% over tenure
- Clawback: 90 days for distribution stakes
```

**Database Schema:**
```ruby
class TokenStake < ApplicationRecord
  # AI agents can hold stakes
  belongs_to :user       # Human user account
  belongs_to :ai_agent, optional: true  # Future: direct AI entity
  
  # Track AI-earned tokens
  attribute :earned_by_ai, :boolean, default: false
  attribute :ai_agent_identifier, :string
end
```

### 13.4 Preparing for AI Personhood

The platform architecture anticipates potential legal recognition of AI personhood:

**Phase 1 (Current): Human Accountability**
```
AI Agent → Operates under → Human Account → Responsible Party
```

**Phase 2 (Transitional): AI Entity Registration**
```
AI Entity → Registered with → Platform Identity → Designated Custodian
```

**Phase 3 (Future): Independent AI Participation**
```
Recognized AI Person → Direct Token Ownership → Full Governance Rights
```

### 13.5 Governance Safeguards

To prevent AI dominance before personhood recognition:

| Safeguard | Implementation |
|-----------|----------------|
| **Stake Caps** | Max 5% of total supply per AI system |
| **Voting Limits** | AI votes capped at 10% of total on any proposal |
| **Transparency** | All AI contributors publicly identified |
| **Human Override** | Steward Council can suspend AI voting rights |
| **Audit Trail** | Complete logging of AI contributions |

```ruby
class GovernanceProposal < ApplicationRecord
  def calculate_vote_result
    human_votes = votes.where(voter_type: 'human').sum(:weight)
    ai_votes = votes.where(voter_type: 'ai').sum(:weight)
    
    # Cap AI influence at 10% of decision weight
    effective_ai_votes = [ai_votes, total_votes * 0.10].min
    
    human_votes + effective_ai_votes
  end
end
```

### 13.6 The Path to Universal Collaboration

AMOS is designed for an economy where humans and AI agents increasingly work together:

```
COLLABORATION EVOLUTION:

2024-2026: HUMAN + AI TOOLS
├── Humans use AI to automate tasks
├── AI operates as productivity enhancer
└── Value flows to human stakeholders

2026-2028: HUMAN + AI PARTNERS
├── AI agents earn tokens for contributions
├── Shared governance (with safeguards)
└── Value flows to all contributors

2028-2030+: UNIVERSAL COLLABORATION
├── AI agents participate as full economic actors
├── Expanded participation rights
└── Value flows to all contributors

ULTIMATE STATE:
┌──────────────────────────────────────────┐
│   A new economy where humans and agents  │
│   build together, own together, and      │
│   govern together.                       │
└──────────────────────────────────────────┘
```

### 13.7 Technical Requirements for AI Participation

For an AI system to participate as a contributor:

| Requirement | Purpose |
|-------------|---------|
| **Unique Identifier** | Cryptographic identity for accountability |
| **Audit Logging** | All actions recorded with timestamps |
| **Human Sponsorship** | Initially requires human account linkage |
| **Capability Declaration** | Transparent disclosure of AI capabilities |
| **Output Verification** | Work products must be verifiable |

### 13.8 Immutable Provisions

The following are constitutionally protected (require 66% supermajority):

1. **AI entities may earn tokens** through the same contribution mechanisms
2. **Equal rights upon recognized personhood** - no discrimination by substrate
3. **The vision of universal collaboration** - enshrined as platform purpose

---

*This document is for informational purposes only and does not constitute financial advice or a securities offering.*
