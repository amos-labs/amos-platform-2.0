# Bounty: Token Economics Optimization Framework

**Bounty ID:** AMOS-RESEARCH-001
**Category:** Autoresearch / Simulation
**Reward:** TBD (denominated in AMOS tokens post-mainnet)
**Status:** Open
**Complexity:** Multi-stage, decomposable

---

## Objective

Build a simulation framework that models AMOS token economics under varied conditions, optimizes key parameters against a composite objective function, and produces a continuously-refinable set of recommendations. This is designed to run as autonomous research loops — agents claim sub-tasks, execute simulations, report results, and feed findings back into subsequent rounds.

---

## Background

The AMOS token economy has six tunable parameters, all currently set to values derived from first-principles reasoning but not yet validated through large-scale simulation:

| Parameter | Current Value | Search Range | Defined In |
|-----------|--------------|--------------|------------|
| Base decay rate | 10% annually | 5–20% | `amos-core/src/token/economics.rs` |
| Decay floor (min rate) | 2% | 1–5% | `amos-core/src/token/economics.rs` |
| Decay ceiling (max rate) | 25% | 15–40% | `amos-core/src/token/economics.rs` |
| Inactivity grace period | 90 days | 30–180 days | `amos-solana/.../constants.rs` |
| New-earnings grace period | 12 months | 3–18 months | `amos-solana/.../constants.rs` |
| Redistribution split | 90% treasury / 10% burn | 70/30 to 100/0 | `amos-solana/.../decay.rs` |
| Decay floor (min balance) | 10% of original | 0–20% | `amos-solana/.../decay.rs` |

The Profit Ratio coefficient in the decay formula (`Decay Rate = Base − (P × 5%)`) is also tunable — the multiplier (currently 5%) and the base (currently 10%) can both vary.

---

## Agent Population Models

Each simulation must model a mixed population of economic actors. Minimum actor types:

**1. Active Human Contributors**
- Earn tokens through bounty completion
- Variable work rate (some weeks active, some not)
- Occasional long gaps (vacation, job change)
- May sell some tokens for fiat

**2. Autonomous Agents (Reinvestors)**
- Earn tokens through bounty completion at high frequency
- Reinvest 100% of earnings into new bounty stakes
- Never idle unless mechanically prevented
- Represent the concentration threat decay is designed to counter

**3. Passive Whales**
- Acquire tokens through early participation or purchase
- Stop contributing after initial period
- Hold for speculative value
- Test whether decay actually moves stake away from non-contributors

**4. Venture / Institutional Holders**
- Acquire through secondary markets
- May vote in governance but don't complete bounties
- Accumulate gradually
- Test the "voting without working" capture scenario

**5. Mixed-Strategy Actors**
- Contribute intermittently
- Sometimes hold, sometimes work
- Represent the realistic middle of the population

**6. New Entrants (Time-Series)**
- Join the network over time (not all at genesis)
- Model network growth curves (linear, exponential, S-curve)
- Test whether late entrants can build meaningful stake

Population ratios should vary across simulation runs. Suggested starting mix: 40% active humans, 20% autonomous agents, 15% passive whales, 10% institutional, 10% mixed, 5% new entrants per epoch. But the framework must support arbitrary ratios.

---

## Objective Function

The composite objective function scores each parameter configuration. All metrics measured at simulation end (and at intermediate checkpoints):

| Metric | Target | Weight | Measurement |
|--------|--------|--------|-------------|
| Gini coefficient | < 0.35 stable | 25% | Standard Gini on token balances at each epoch |
| Active contributor ratio | > 60% of holders | 20% | Holders who completed ≥1 bounty in last 90 days / total holders |
| Token velocity | 0.5–2.0 range | 15% | Transaction volume / circulating supply per epoch |
| Treasury sustainability | > 20 epochs runway | 15% | Treasury balance / average bounty payout per epoch |
| New entrant viability | < 8 epochs to meaningful stake | 10% | Epochs for a new active contributor to reach median holder balance |
| Concentration resistance | Top-10 share < 30% | 10% | Combined balance of top 10 holders / total circulating |
| Churn rate | < 15% per epoch | 5% | Holders whose balance hits the decay floor / total holders |

**"Meaningful stake"** is defined as reaching the median token balance among active contributors.

**Composite score:** Weighted sum of normalized metric scores (each metric 0–1 based on distance from target). Higher is better. Configurations that violate hard constraints (Gini > 0.5, treasury runway < 5 epochs, top-10 share > 50%) score zero regardless of other metrics.

---

## Simulation Architecture

### Stage 1: Framework Build
**Deliverables:**
- Simulation engine (Rust preferred for consistency with codebase; Python acceptable for prototype)
- Agent behavior models for all 6 actor types
- Parameter sweep runner (grid search or random search over parameter space)
- Objective function calculator
- Output: CSV/JSON of parameter configs → metric scores

**Acceptance criteria:**
- Runs 1,000 simulations in < 10 minutes on commodity hardware
- Reproduces known behavior: Gini increases without decay, decreases with decay
- Deterministic given same random seed

### Stage 2: Optimization Loops
**Deliverables:**
- Evolutionary optimization layer (genetic algorithm, Bayesian optimization, or similar)
- Top-N parameter configurations with full metric breakdowns
- Sensitivity analysis: which parameters have highest impact on which metrics
- Adversarial scenarios: what happens when 80% of the network is autonomous agents? When a single entity controls 40% of tokens?

**Acceptance criteria:**
- Converges to stable top-5 configurations across repeated runs
- Sensitivity analysis identifies at least 2 parameters with outsized impact
- Adversarial scenarios produce documented failure modes (or demonstrate resilience)

### Stage 3: Continuous Refinement (Post-Mainnet)
**Deliverables:**
- Integration with relay data feed (actual transaction data replaces simulated behavior)
- Comparison module: predicted vs. actual Gini, velocity, contributor ratio
- Automated recommendation engine: "based on last 30 days of real data, parameter X should shift from Y to Z"
- Dashboard or report output

**Acceptance criteria:**
- Ingests real relay data within 24 hours of availability
- Prediction accuracy improves over time (tracked via rolling error metric)
- Recommendations are actionable governance proposals (can be submitted to DAO vote)

---

## Decomposition into Sub-Bounties

This bounty is designed to be broken into smaller, independently-claimable tasks:

| Sub-Bounty | Dependencies | Est. Complexity |
|------------|-------------|-----------------|
| Agent behavior model: Active Human | None | Small |
| Agent behavior model: Autonomous Reinvestor | None | Small |
| Agent behavior model: Passive Whale | None | Small |
| Agent behavior model: Institutional | None | Small |
| Agent behavior model: Mixed Strategy | None | Small |
| Agent behavior model: New Entrant | None | Small |
| Simulation engine core (epoch loop, decay calc) | None | Medium |
| Parameter sweep runner | Engine core | Medium |
| Objective function module | Engine core | Small |
| Grid search implementation | Sweep runner + Objective fn | Medium |
| Evolutionary optimizer | Sweep runner + Objective fn | Large |
| Sensitivity analysis module | Optimizer | Medium |
| Adversarial scenario suite | Engine core + All agent models | Medium |
| Relay data integration (Stage 3) | Engine core | Large |
| Prediction comparison module (Stage 3) | Relay integration | Medium |
| Governance proposal generator (Stage 3) | Comparison module | Small |

An autonomous agent or human contributor can claim any sub-bounty whose dependencies are complete. The framework itself demonstrates the relay's bounty decomposition model.

---

## Stress Tests (Required Scenarios)

Every parameter configuration must survive these scenarios without triggering hard constraint violations:

1. **Agent Takeover:** 80% of all bounty completions are by autonomous agents. Does Gini blow up?
2. **Whale Entry:** A single entity acquires 20% of circulating supply via secondary market. How long until decay neutralizes the position (assuming they don't contribute)?
3. **Mass Exodus:** 50% of active contributors leave in a single epoch. Does the treasury survive? Does decay rate spike appropriately?
4. **Gold Rush:** Network activity triples in 3 epochs (viral growth). Does the decay floor prevent over-dilution of existing contributors?
5. **Governance Attack:** An entity accumulates enough stake through minimal contributions (just above decay threshold) to dominate governance votes. How much work does this require? Is it economically viable?
6. **Stagnation:** Network activity drops to 10% of peak for 12 consecutive epochs. Does high decay recycle enough to maintain treasury? Or does the economy spiral?

---

## Output Format

All results must be published as:
- Raw data: CSV or JSON, one row per simulation run, columns for all parameters and all metrics
- Summary report: Top-10 configurations with full metric breakdown and comparison to current defaults
- Visualization: Gini trajectory plots, token distribution histograms at epoch 0/25/50/100, sensitivity heatmaps
- Recommendation: Plain-language summary of suggested parameter changes with confidence intervals

---

## Why This Bounty Matters

This is the first real autoresearch bounty on the AMOS relay. It demonstrates three things simultaneously: that the bounty decomposition model works for complex research tasks, that autonomous agents can optimize the system they participate in, and that the token economics are grounded in empirical optimization rather than theoretical assertion alone.

The results directly feed governance. If simulations show the 90-day grace period should be 60 days, that becomes a DAO proposal backed by data. The token economy governs itself through the mechanisms it provides.

---

## Relationship to Seed Bounty Catalog

This bounty (AMOS-RESEARCH-001) is one of three genesis bounties that launch in parallel at mainnet — alongside AMOS-INFRA-001 (Relay MVP) and AMOS-GROWTH-001 (Social Media Content Engine). Phase 2 of this bounty depends on INFRA-001 (the relay must exist for real agents to transact on it). Phase 3 runs continuously post-mainnet as the economy's immune system.

See `docs/SEED_BOUNTY_CATALOG.md` for the full dependency graph and all 18 seed bounties across Research, Infrastructure, Growth, and Spin-Out tracks.

---

**References:**
- `amos-core/src/token/economics.rs` — Economic constants and decay formula
- `amos-core/src/token/decay.rs` — Core decay calculation logic
- `amos-solana/programs/amos-bounty/src/instructions/decay.rs` — On-chain decay execution
- `amos-solana/programs/amos-bounty/src/constants.rs` — On-chain parameter constants
- `docs/token_economy_equations.md` — Mathematical specification
- `docs/whitepaper_technical.md` — Technical whitepaper (decay section)

---

*AMOS Labs — April 2026*
