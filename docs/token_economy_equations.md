# AMOS Token Economy: Equation Cheat Sheet

> Quick reference for all the math behind the token economy

---

## 🎯 THE ONE EQUATION THAT RULES THEM ALL

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│         DECAY RATE = 10% - (PROFIT RATIO × 5%)                              │
│                                                                             │
│         Clamped between 2% (min) and 25% (max)                              │
│                                                                             │
│         Where: PROFIT RATIO = (Revenue - Costs) / Costs                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**In plain English:**
- If we're 20% profitable → Decay = 10% - (0.20 × 5%) = **9%** (low decay)
- If we're break-even → Decay = 10% - (0 × 5%) = **10%** (base decay)
- If we're 20% unprofitable → Decay = 10% - (-0.20 × 5%) = **11%** (higher decay)

---

## 💰 COST EQUATIONS

### AWS Costs (What We Pay)

```
AI Cost = (Input Tokens ÷ 1000 × Input Rate) + (Output Tokens ÷ 1000 × Output Rate)
```

| Model | Input Rate | Output Rate |
|-------|------------|-------------|
| Qwen3-Next-80B | $0.20/1M | $0.80/1M |
| Claude 3.5 Haiku | $0.25/1M | $1.25/1M |
| Claude 3.5 Sonnet | $3.00/1M | $15.00/1M |

**Example:**
```
10K input + 5K output on Sonnet:
= (10 × $0.003) + (5 × $0.015)
= $0.03 + $0.075
= $0.105
```

### Total Monthly Costs

```
C_monthly = C_ai + C_email + C_compute + C_storage + C_other + C_fixed

Where:
  C_ai      = Sum of all AI/LLM usage
  C_email   = $0.09 per 1000 emails
  C_compute = ECS/Lambda costs  
  C_storage = S3/RDS costs
  C_other   = Textract, Comprehend, etc.
  C_fixed   = Infrastructure, personnel
```

---

## 💵 REVENUE EQUATIONS

### Customer Billing

```
Customer Charge = AWS Cost × 1.20  (20% markup)
```

**Example:**
```
$100 AWS cost → $120 customer charge → $20 margin
```

### Monthly Revenue

```
R_monthly = R_subscriptions + R_compute + R_enterprise

Where:
  R_subscriptions = Sum of all active plan fees
  R_compute       = Customer compute charges (with 20% markup)
  R_enterprise    = One-time deals and custom pricing
```

---

## 📊 PROFIT RATIO (The Bridge)

```
              Revenue - Costs
Profit (π) = ─────────────────
                  Costs
```

| Scenario | Revenue | Costs | π |
|----------|---------|-------|---|
| Highly profitable | $60K | $40K | +0.50 |
| Healthy | $50K | $45K | +0.11 |
| Break-even | $50K | $50K | 0.00 |
| Slight loss | $45K | $50K | -0.10 |
| Significant loss | $30K | $50K | -0.40 |

---

## 🔄 DECAY EQUATIONS

### Base Decay Rate (from Platform Economics)

```
δ_base = 10% - (π × 5%)

Examples:
  π = +0.40 → δ = 10% - 2%  = 8%
  π = 0.00  → δ = 10% - 0%  = 10%
  π = -0.40 → δ = 10% + 2%  = 12%
```

### Effective Decay Rate (for Your Stake)

```
δ_effective = δ_base × (1 - tenure_reduction) × (1 - vault_reduction)
```

| Tenure | Reduction |
|--------|-----------|
| 0-1 years | 0% |
| 1-2 years | 20% |
| 2-5 years | 40% |
| 5+ years | 70% |

| Vault Tier | Lock Period | Reduction |
|------------|-------------|-----------|
| None | - | 0% |
| Bronze | 30 days | 20% |
| Silver | 90 days | 50% |
| Gold | 365 days | 80% |
| Permanent | No unlock | 95% |

**Example:**
```
Base rate = 10%, 3-year tenure (40% reduction), Silver vault (50% reduction):
δ_effective = 10% × (1 - 0.40) × (1 - 0.50)
            = 10% × 0.60 × 0.50
            = 3%
```

### Daily Decay

```
δ_daily = 1 - (1 - δ_annual)^(1/365)

For 10% annual:
δ_daily = 1 - (0.90)^(1/365) = 0.0289% per day
```

### Stake Value After Decay

```
V_tomorrow = V_floor + (V_decayable × (1 - δ_daily))

Where:
  V_floor = Initial × Floor% (protected, never decays)
  V_decayable = Current - V_floor
```

---

## 🎁 TOKEN EMISSION EQUATIONS

### Daily Emission Pool

```
E_daily = 16,000 × Halving_Multiplier

Halving Schedule:
  Year 0-1:  × 1.00    = 16,000/day
  Year 1-2:  × 0.50    =  8,000/day
  Year 2-3:  × 0.25    =  4,000/day
  Year 3-4:  × 0.125   =  2,000/day
  Year 4-5:  × 0.0625  =  1,000/day
  Year 5-6:  × 0.03125 =    500/day
  Floor:     Minimum   =    100 AMOS/day
```

### Your Token Reward

```
                Your Points
Your Tokens = ────────────── × Daily Pool
              Total Points
```

**Example:**
```
You: 100 points, Total: 5,000 points, Pool: 16,000 AMOS

Your Tokens = (100 / 5,000) × 16,000 = 320 AMOS
```

---

## 📈 POINTS EQUATIONS

### Referral Points

```
P_referral = (Emails × 1) + (Signups × 5) + (Conversions × 10) + (Active Months × 2)
```

**Example:**
```
Send 20 emails, get 4 signups, 2 convert, stay 6 months:
= (20 × 1) + (4 × 5) + (2 × 10) + (6 × 2)
= 20 + 20 + 20 + 12
= 72 points
```

### Sales Points

```
P_sales = Users Signed Up × 1
```

### Bounty Points

```
P_bounty = Bounty Value (in AMOS)

50 AMOS bounty = 50 points
```

---

## 💸 REVENUE SHARE EQUATIONS

### Revenue Allocation

```
Token Holders:  50% of Revenue (immutable on-chain)
R&D:            40% of Revenue (software, infrastructure, research, AI work)
Treasury:        5% of Revenue (emergency fund, DAO-controlled)
Operations:      5% of Revenue (accounting, legal only)
```

### Your Payout

```
                  Your Stake
Your Payout = ──────────────── × (Revenue × 50%)
              Total Staked
```

**Example:**
```
You: 50,000 AMOS, Total: 10,000,000 AMOS, Revenue: $100,000

Holder Pool = $100,000 × 50% = $50,000
Your Payout = (50,000 / 10,000,000) × $50,000 = $250/month
```

---

## 🛡️ PROTECTION EQUATIONS

### Grace Period

```
First 365 days: NO DECAY at all
```

### Decay Floor (Never Goes Below)

```
V_floor = Initial × Floor%

Floor Schedule:
  Year 0-1:  5% floor
  Year 1-2: 10% floor
  Year 2-5: 15% floor
  Year 5+:  25% floor
```

**Example:**
```
10,000 AMOS stake at Year 3:
V_floor = 10,000 × 15% = 1,500 AMOS (protected forever)
```

### Clawback (Distribution Stakes)

```
First 90 days: Stake can be clawed back if customer churns
After 90 days: Stake is confirmed permanent
```

---

## 📱 QUICK REFERENCE CARD

```
┌──────────────────────────────────────────────────────────────────┐
│                     KEY NUMBERS                                   │
├──────────────────────────────────────────────────────────────────┤
│  Total Supply:          100,000,000 AMOS                         │
│  Daily Emission:        16,000 AMOS (halving annually)           │
│  Base Decay:            10% annual                               │
│  Min/Max Decay:         2% - 25% annual                          │
│  Compute Markup:        20%                                      │
│  Revenue to Holders:    50%                                      │
│  Grace Period:          12 months                                │
│  Clawback Period:       90 days                                  │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                     THE FLOW                                      │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  AWS Costs ──→ 20% Markup ──→ Revenue                            │
│                                 │                                │
│                                 ▼                                │
│                          Profit Ratio                            │
│                                 │                                │
│                                 ▼                                │
│                          Decay Rate                              │
│                                 │                                │
│                                 ▼                                │
│                       Token Value Stability                      │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                     EARN → STAKE → EARN                          │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Contribute ──→ Earn Points ──→ Get Tokens                       │
│                                      │                           │
│                                      ▼                           │
│                              Stake Tokens                        │
│                                      │                           │
│                                      ▼                           │
│                           Get Revenue Share                      │
│                                      │                           │
│                                      ▼                           │
│                           Re-invest or Cash Out                  │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## 🧮 EXAMPLE: COMPLETE CYCLE

```
SCENARIO: You're an active contributor for one month

STEP 1: AWS Usage (Platform Level)
  Monthly AI costs:     $30,000
  Monthly email costs:   $1,000
  Monthly compute:       $5,000
  Monthly other:         $4,000
  ──────────────────────────────
  Total Costs:          $40,000

STEP 2: Revenue
  Subscriptions:        $25,000
  Compute (w/ 20%):     $24,000  ($20K cost + $4K margin)
  Enterprise deals:     $10,000
  ──────────────────────────────
  Total Revenue:        $59,000

STEP 3: Profit Ratio
  π = ($59,000 - $40,000) / $40,000 = 0.475 (47.5% profit!)

STEP 4: Decay Rate
  δ = 10% - (0.475 × 5%) = 10% - 2.375% = 7.625%
  (Very healthy - low decay rewards holders)

STEP 5: Your Contribution
  You referred 5 users (signups): 5 × 5 = 25 points
  1 converted to paid:            1 × 10 = 10 points
  You completed a 50-point bounty: 50 points
  ──────────────────────────────────────────────
  Total points: 85 points

STEP 6: Daily Token Reward (average day)
  Daily pool: 16,000 AMOS
  Your daily points: ~3 (85 ÷ 30 days)
  Platform daily points: ~500
  Your daily tokens: (3 / 500) × 16,000 = 96 AMOS

STEP 7: Monthly Token Earnings
  96 AMOS × 30 days = 2,880 AMOS earned

STEP 8: Decay on Existing Stake
  Previous stake: 10,000 AMOS
  Annual decay: 7.625%
  Monthly decay: ~0.65%
  Decay amount: 10,000 × 0.65% = 65 AMOS

STEP 9: Net Position
  Previous: 10,000 AMOS
  Earned:   +2,880 AMOS
  Decayed:     -65 AMOS
  ─────────────────────
  New stake: 12,815 AMOS  (+28% growth!)

STEP 10: Revenue Share
  Your stake: 12,815 AMOS
  Total staked: 10,000,000 AMOS
  Your share: 0.128%
  Holder pool: $59,000 × 50% = $29,500
  Your payout: 0.128% × $29,500 = $37.80/month
```

---

*This is the complete, bulletproof math. All tied together.*
