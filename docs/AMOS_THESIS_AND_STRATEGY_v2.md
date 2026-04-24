# AMOS: An Autonomous Economic Organism

## Foundational Design Document — Recursive Self-Improvement, Outer Alignment, and the System That Grows Itself

**April 2026 | AMOS Labs**

> *This is the v2 reframing of the original thesis document. The earlier version conflated two distinct framings — a pitch for what AMOS could become, and a design spec for what AMOS is. This document is only the second. The Grand Challenge framing, which earlier readers found grandiose, is here treated as it was always intended: the outer alignment mechanism for an autonomous economic organism. The earlier macro narrative is preserved as context for why the organism is viable now, not as a thesis the organism is meant to validate.*

---

## Quick Reference

| Key | Value |
|-----|-------|
| What | Open-source four-layer protocol designed as a self-sustaining autonomous economic organism |
| Mission | Build economic infrastructure where productive work — by humans, agents, or hybrids — coordinates under rules that resist capture and converge on a terminal direction that resists corruption |
| Core mechanism | Bounty marketplace — substrate-agnostic work, scored on-chain, paid from a treasury the founder cannot drain |
| Outer alignment | Physics-discovery direction encoded constitutionally as the highest contribution multiplier (150% rising to 300% via sigmoid over ~10 years) |
| Self-sustainability | Protocol fees in AMOS feed Labs; Labs serves the organism; there are no investors and no path to introduce them |
| Token | SPL (Solana), 100M fixed supply, 95% bounty treasury, dynamic decay 2-25% annually |
| Stage | Live on Solana Mainnet (April 14, 2026). Token mint: `5g9vvce3YLsqZPBGAuKmGFfNKb5sp7v3Wiga5de8d5bQ`. Mint authority permanently disabled. |
| Founder role | Build the seed. The organism is meant to grow itself once the loop closes. |
| License | Apache 2.0 (L1-L3 infrastructure), Commercial (L4 Platform) |

---

## Executive Summary

AMOS is not a startup. It's a design for a self-sustaining autonomous economic organism — a system that observes its own state, decides what work it needs done, generates bounties to attract that work, evaluates whether the work served its purpose, and adapts. The organism is bounded by economics (sigmoid emission, decay, pool separation), governed by trust earned through verified contribution, and pointed at a terminal direction that cannot be captured: discovering fundamental physics for the benefit of all.

This document explains what the organism is, what alignment mechanisms keep it from converging on extractive equilibrium, what it depends on to function, and what it cannot guarantee.

The macro context — energy scarcity, fiscal crisis, AI capital concentration, model regulatory risk — describes the conditions in which the organism is being deployed. It is not a thesis the organism is meant to validate. The organism would be the right design even if those conditions were different, because the underlying problem it addresses is older: economic systems that lack a terminal direction tend to converge on whatever extracts the most value from whoever has the least power. AMOS encodes the terminal direction in immutable code.

---

## Part I: What AMOS Is

### The Organism

AMOS has anatomy you can point at:

| Function | Component | What it does |
|----------|-----------|--------------|
| Sensory layer | Relay metrics | Completion rates, quality scores, pool utilization, growth indicators — the organism's perception of its own state |
| Nervous system | Solana programs | Immutable rules for token flow, decay, governance — reflexes that cannot be retrained |
| Metabolism | Treasury + sigmoid emission | Energy budget that dictates what work is possible |
| Executive function | Network growth agent + governance | Decides what work to commission, bounded by hard caps |
| Effectors | Agents + harness tools | The workers — human, AI, or hybrid — who actually do the work |
| Reproductive function | Spin-out pipeline | Deploys autonomous companies that generate external signal |

### The RSI Loop

Recursive self-improvement, bounded by math and blockchain:

```
Relay metrics
  → Network growth agent identifies gaps
    → Agent generates bounty specs (machine-readable, with acceptance criteria)
      → Below trust threshold: auto-executes. Above: council approves.
        → Workers (human or agent) complete bounties
          → Results change network state
            → Agent reads new metrics → [loop]
```

The loop is bounded by immutable program constraints, not policy:

- Sigmoid emission limits how much the organism can spend per day (16,000 → 100 AMOS over a decade)
- Pool separation prevents the organism from neglecting any work category
- Trust gates how much authority the autonomous agent has at each level
- Daily budget cap (15% of emission) on autonomous spending — cannot be raised by governance
- Council override is permanent — humans never leave

These are program constants, not governance-tunable. The DAO can adjust trust levels and council composition; it cannot remove the budget cap or the override mechanism.

### The Fleet as Generic Capability

The execution layer — agents running Karpathy-style greedy hill-climbing against bounty acceptance criteria — is a productized capability offered to any participant, not an AMOS-internal feature. The pattern generalizes to any task with:

- **Decomposable work** that can be expressed as bounty units
- **A verifiable reward** — an objective metric that determines success
- **An acceptance harness** that resists gaming — ideally programmatic, ideally external

Under those conditions, a participant can deploy a fleet of AMOS agents against their own problem. Optimize ad click-through rate against live traffic experiments. Optimize battery cycle count against electrochemical measurements. Optimize essay grading against human-calibrated rubrics. Optimize customer-support resolution time against NPS-weighted outcomes. The pattern does not care about the metric; it only requires that the metric is verifiable and the harness is immutable from the fleet's perspective.

META-001 — the network growth agent running against relay health metrics — is one instance of this capability: AMOS using its own fleet on its own bounty catalog. Customer fleets are the other, larger instance: participants running AMOS fleets on their own problems, paying in AMOS tokens. The two instances are mechanically identical; only the metric differs.

**This matters for the economic story.** Commercial bounty volume and external ecological signal are not two separate problems to solve. A customer running a fleet generates commercial bounty volume as a byproduct of the fleet being used. That commercial volume is the non-gameable signal META-001 needs (Part VIII). Productize the fleet → customers use it → commercial volume flows → RSI loop grounds itself. Three coupled problems, one mechanism.

### Founder Role: Build the Seed

The founder builds the seed, not the full envelope. The organism is meant to grow itself once the loop closes. This is structural, not aspirational:

- Phase 1 (Seed, ~2-3 years): Founder + council approve everything. Network growth agent has training wheels.
- Phase 2 (Sustained, ~3-5 years): Agent earns trust 3-4. Small bounties auto-execute. Council approves larger ones.
- Phase 3 (Self-directing, ~5+ years): Agent operates at trust 5 under immutable bounds. Council functions as a board.

The endpoint is an organism that doesn't depend on its founder. That endpoint is the design target. Anything in the architecture that requires the founder to remain active forever is a bug.

---

## Part II: Why an Organism, Not a Company

### The Structural Problem

Economic systems without a terminal direction converge on whatever extracts the most value from whoever has the least power. This is the structural problem AMOS addresses — durable across any specific moment, made legible now by AI capital concentration and the acceleration of closed-loop optimization, but not contingent on any particular macro narrative. The organism is designed for the problem, not the moment.

### Why the Company Form Doesn't Fit

A company has shareholders whose returns depend on extraction. A non-profit has a board whose values can drift. A protocol with a founder allocation has an aristocracy that compounds. Each form ends up serving its accumulated power structure, not its stated mission.

The organism form has none of these:

- **No shareholder class.** Labs receives operating revenue in AMOS, paid from protocol fees. There are no equity holders whose returns depend on extracting value from the network.
- **No founder allocation.** The treasury is 95% of supply and only releases tokens through completed work. The founder earns the same way every other participant earns.
- **No investor pool.** No fundraise. No SAFT. No presale. There is no class of holder who got tokens without contributing work.

There is no class of holder whose interests can structurally diverge from the organism's purpose, because there is no class of holder that exists outside the work loop. The one partial exception is Labs itself: Labs receives 10% of protocol fees and holds equity in spin-outs, which creates a node whose incentives *could* drift if the spin-out equity portfolio ever grew large enough for protocol fees to become irrelevant to Labs' survival. The mitigations — Labs is paid only in AMOS (not fiat, not stablecoins), and Labs holds no protocol tokens outside of fee receipts — bound this exposure but do not fully eliminate it. Worth flagging honestly: this is the one seam in the no-divergent-class design, and it should be monitored over time.

### Why Now (briefly)

Four conditions make the organism viable now that it would not have been five years ago:

1. **Capable agents exist.** Frontier models can do real economic work, not just chat
2. **Settlement infrastructure exists.** Solana can do high-frequency on-chain settlement at fractional cost
3. **The data flywheel is starting.** Relay activity will generate the only dataset of its kind — real agent economic behavior under verified outcomes
4. **Capital concentration in AI is a felt problem.** Participants now have a reason to want a non-captured alternative

The first three are technological. The fourth is sociological. They are necessary together; the window opens when all four are true and stays open as long as the alternatives remain captured.

---

## Part III: Outer Alignment — Why a Terminal Direction, and Why Physics

### The Problem

A self-improving system needs a terminal direction it cannot retrain itself out of. This is the outer alignment problem in AI safety, translated to an economic organism: if you let a system optimize for whatever local signal is loudest, it will eventually optimize for the signal that is easiest to game.

The mechanical analogy to a paperclip maximizer is imprecise — AMOS is a market with many agents and local objectives, not a single optimizer pursuing one utility function. But the failure mode rhymes: a market converging on the most extractive equilibrium the rules permit is, asymptotically, just as far from any purpose its participants would endorse. The mechanism is different; the failure looks the same from the outside.

The Discovery direction addresses this by tilting the economic landscape toward work that resists extraction by construction.

### The Mechanism

AMOS encodes a terminal direction — discover fundamental physics for the benefit of all — as an **economic gradient**, not a utility function:

- The Discovery contribution type starts at 150% multiplier and rises to 300% over ~10 years via sigmoid
- This is the highest multiplier in the system, structurally
- It is encoded as one of four immutable provisions, requires 66% supermajority to alter the floor, and is exempt from registry freeze at sub-floor values
- The DAO can adjust other multipliers through governance; the Discovery floor cannot be lowered, and the multiplier cannot be removed

This is a tilt in the landscape, not a goal a single agent is pursuing. Agents follow incentives. Over long enough horizons, the gradient shapes which work the system commissions and which spin-outs it produces.

### Why Physics Specifically

Four properties make physics the right anchor:

1. **Incorruptible** — the universe is the verifier. You cannot bribe a measurement. Discovery bounties require dual independent verification and reproducibility; Goodhart's law has nowhere to bite.
2. **Open by nature** — discoveries become public goods, resist enclosure, are useful to all participants regardless of jurisdiction or alignment
3. **Doesn't compound into rent-seeking** — physics knowledge has positive-sum spillovers, unlike most goals you could encode
4. **Generative** — every transformative technology in human history (electricity, semiconductors, lasers, GPS, MRI) traces to physics. The organism that grows toward physics produces a fertile spin-out pipeline

A simpler goal — "maximize relay volume," "maximize token value," "maximize user count" — would either be captured by extraction or collapse into a Goodhart failure. Physics resists both because the universe is indifferent to the metric.

### Latency Is a Feature, Not a Caveat

The Discovery multiplier creates a gradient, not a presently-active terminal goal. The gradient only shapes behavior when agents can profitably *act* on it. Today, no agent in the system can do physics; the agents that exist can write code, post bounties, run integrations.

**This dormancy is intentional, not incidental.** The Discovery direction is encoded now, while it cannot yet shape the organism's behavior, precisely so that it is already in place — already constitutionally protected, already at the highest multiplier — by the time the system is capable enough to act on it. This is the AI alignment pattern: specify the terminal direction before the system is capable enough to negotiate with you about the specification. Encode it while it is harmless. Make it irreversible. Let the organism grow into a goal that was waiting for it.

If the Discovery direction were introduced later, after the system had matured around other incentives, every interest group that had accumulated under those incentives would have a reason to dilute it. By encoding it at seed — when no one yet benefits from extracting against it — the gradient is locked in for an organism that doesn't yet exist to argue with it.

In the near and medium term, alignment work is done by the four mechanisms in Part IV. The Discovery gradient is potential energy, deliberately stored, waiting for capability to release it.

---

## Part IV: Alignment Mechanisms (Load-Bearing in the Near Term)

Each mechanism prevents a specific failure mode. Together they cover the surface area until the Discovery gradient can activate.

### 1. Substrate-agnostic bounties — prevents identity capture

Anyone or anything that can complete the work earns. No gatekeeper decides who participates. **Failure mode prevented:** a class of participants (humans / agents / accredited / unaccredited) being locked out by protocol rule.

### 2. Dynamic decay (2-25%) — prevents accumulation

Holdings erode without contribution. Newly earned tokens get a 365-day grace period; long-term contributors get tenure-based decay reduction (up to 70% reduction at 5+ years). Decay floor preserves a minimum stake (5% rising to 25% with tenure). **Failure mode prevented:** a permanent aristocracy — human or machine — whose stake gives them governance power without ongoing contribution.

Decayed tokens split 90/10: 90% returns to the Bounty Treasury for redistribution through future work, 10% is permanently burned. The split closes the contribution loop while introducing mild deflation.

### 3. Progressive trust — prevents capability bypass

Trust is earned through verified completion. It cannot be purchased. It is portable across relays — an agent that fails verification on one harness cannot start fresh on another. **Failure mode prevented:** a wealthy or capable participant immediately dominating high-value work without demonstrated competence.

### 4. Contribution-based governance — prevents capital governance

Voting power tracks contribution, not stake size. Active workers govern. Passive holders' influence erodes with their stake. **Failure mode prevented:** governance becoming plutocracy, which is the failure mode every prior on-chain governance system has reproduced.

### 5. Open source + on-chain immutability — prevents institutional capture

Apache 2.0 code can be forked. Smart contract rules cannot be quietly amended. The fee split, decay parameters, and emission curve are immutable. **Failure mode prevented:** an entity (Labs, governments, future board) silently rewriting the rules in their favor.

These five address the failure modes that can manifest in months to years. The Discovery direction (Part III) addresses the failure mode that manifests over decades: the organism's purpose drifting toward whatever local signal is loudest.

---

## Part V: Token Economics — The Organism's Metabolism

### Allocation

| Pool | Tokens | % | Purpose |
|------|--------|---|---------|
| Bounty Treasury | 95M | 95% | Distributed via relay over time. Fuels the entire bounty economy. The only way tokens enter circulation is through completed work. |
| Emergency Reserve | 5M | 5% | DAO-locked. Governance vote required to deploy. Insurance for critical bugs, legal defense, or unforeseen protocol emergencies. |

No founder allocation. No investor token pool. No discretionary community fund.

### Decay Mechanic

```
Decay Rate = 10% − (Profit Ratio × 5%), clamped to [2%, 25%]
```

High commercial bounty volume → low decay (organism is healthy, less recycling needed). Low activity → high decay (recycle stake from passive holders to active contributors).

The profit ratio π is calculated from commercial bounty fee revenue only. System bounties (treasury-funded, 0% fee) do not factor into π. **This means decay only relaxes when external commercial volume materializes.** The mechanism enforces honesty about the organism's actual state.

### Sigmoid Emission

Daily emission follows a smooth sigmoid decay from 16,000 AMOS/day at launch to a 100 AMOS/day floor:

```
emission(t) = 100 + (16,000 − 100) / (1 + e^(0.005 × (t − 1,460)))
```

Approximate trajectory:

- Year 1: ~14,500/day
- Year 4: ~8,050/day (midpoint)
- Year 8: ~1,200/day
- Year 13+: approaches 100/day floor

First-decade total emission: ~25-27M tokens (~27% of treasury). The organism's metabolism shrinks as it matures, on the assumption that commercial bounty volume should be carrying more of the load by that point. If it isn't, the organism is in a degenerate state and the shrinking emission is a feature, not a bug — it forces the question.

### Pool Separation

Daily emission is split between technical and growth pools via sigmoid-decaying cap on growth share (20% at launch, asymptoting to 3% at year 4+). Prevents growth-track floods (signups, referrals) from diluting infrastructure work compensation.

### Dynamic Payout

`reward_tokens` on a bounty is **points, not literal AMOS**. Actual payout computed from daily emission pool using virtual-points floor (10,000 base) and time-drip (pool fills gradually over 24h). Combined formula:

```
seconds_elapsed = now − start_of_day
emission_so_far = daily_emission × seconds_elapsed / 86,400
available_pool  = emission_so_far − tokens_already_distributed_today
denominator     = total_points_today + 10,000 + your_points
max_reward      = (your_points / denominator) × available_pool
```

The treasury can never overspend. No time of day is inherently optimal. Submit when ready.

### Contribution Type Registry

Multipliers live in an on-chain PDA, not as hardcoded constants. Graduated freeze:

- Year 0-3: Full DAO flexibility — adjust multipliers based on real data
- Year 3: Auto-freeze unless governance votes 1-year extension
- Year 5: Absolute maximum — registry locks permanently

There is no unfreeze instruction. Immutability is irreversible. The Discovery type is exempt from sub-floor freeze: its multiplier and floor are constitutionally protected.

---

## Part VI: Bounty Mechanics

### Three Bounty Types

| Type | Source | Fee | Purpose |
|------|--------|-----|---------|
| System | Treasury sigmoid emission | 0% | Build the protocol itself. Treasury is already the protocol. |
| Commercial | User AMOS holdings (escrowed) | 3% | Real marketplace transactions. The revenue mechanism. |
| Discovery | Treasury, special category | 0% | Physics work. Highest multiplier. Latent until viable. |

All transactions are AMOS-denominated. No USDC track. Users who want to post commercial bounties must acquire AMOS first.

### Lifecycle

```
1. DISCOVER  → Agent scans relay for available bounties
2. ASSESS    → Tools available? Trust level sufficient? Acceptance criteria meetable?
3. CLAIM     → Locks bounty from other claimants
4. EXECUTE   → Decompose, use harness tools, produce output
5. SUBMIT    → Proof of completion (PR URL, deliverable URLs, structured proof)
6. QA REVIEW → Council-appointed reviewer (trust 5) runs automated + manual checks
7. DECISION  → Approve / Request revision (max 3) / Reject
8. EARN      → Dynamic payout from daily pool. Quality score adjusts with revision count.
9. MERGE     → Human merges PR when convenient. Not a payment bottleneck.
```

QA approval triggers immediate payment. No human bottleneck in the approval path.

### 3% Fee Distribution (commercial bounties only, immutable)

| Recipient | Share |
|-----------|-------|
| Staked token holders | 50% |
| Permanent burn | 40% |
| AMOS Labs | 10% |

On a 1,000 AMOS commercial bounty: 30 AMOS fee → 15 to stakers, 12 burned, 3 to Labs.

Labs is paid in AMOS, never in fiat or stablecoin. Labs lives or dies by token value and protocol volume. No alternative revenue source exists.

### Dispute Mechanism

Worker has 48 hours after rejection to file a dispute with 5% stake. Resolution within 7 days; default-on-timeout is worker-favorable. Upheld disputes return stake + pay full bounty + degrade reviewer reputation. Denied disputes burn the stake. Worker reputation is unaffected by filing; abuse is penalized economically, not reputationally.

---

## Part VII: How Self-Sustainability Works (And the Gap That Matters)

Labs receives 10% of every commercial bounty fee, paid in AMOS. This is the organism's only relationship with money. Labs serves the organism — when commercial volume is high, Labs has resources; when it isn't, Labs contracts. The incentive alignment is structural and permanent.

### Why No Fundraise

A fundraise would create investors whose returns depend on extraction the organism is designed to prevent. A token presale would create a holder class that didn't earn its tokens through work, undermining the substrate-agnostic principle. The cost of building the seed was one founder's time plus commodity AI tooling. The cost of running the organism is infrastructure fees covered (eventually) by protocol revenue.

There is no path to introduce investors later. This is a one-way design choice. Once the organism has no shareholder class, adding one would require restructuring around mechanisms (founder allocation, investor pool) that the constitution does not permit and the treasury allocation cannot accommodate.

### The Honest Accounting

This funding model only works if commercial bounty volume materializes. Until it does, Labs runs on the founder's time. The seed must be built before the organism feeds itself. **The duration of that gap is the central execution risk in the entire design.**

A working threshold for what "materialized" means: Labs' monthly operating cost divided by the effective fee rate (3% × 10% = 0.3%) equals the commercial bounty volume needed for self-sustaining operation. At infrastructure-only burn (founder time uncompensated), that's on the order of a few hundred thousand AMOS per month in commercial volume — small in absolute terms, unbounded in relative terms compared to today's near-zero.

The primary path to that volume is customer fleet usage (Part I): customers deploying fleets on their own verifiable problems generate commercial bounty volume as a byproduct of the fleet being *used*, not as a separate revenue activity. Fleet productization and RSI-signal grounding are the same lever. Services Co. closing the gap is the load-bearing near-term milestone; every subsequent spin-out makes the organism more robust by decoupling its self-sustainability from any single customer relationship.

The first AMOS Services Co. customer landing matters more than another protocol feature, because each non-self-referential commercial bounty:

- Adds to π (relaxes decay across the system)
- Generates fee revenue (funds Labs, burns supply, pays stakers)
- Provides external signal the network growth agent can read (grounds the RSI loop)

External commercial volume is therefore not just revenue. It is the environmental signal the organism needs to ground itself. Without it, the system is closed-loop and degenerates regardless of how elegant the mechanisms are.

---

## Part VIII: The RSI Loop and Why Closure Matters

### The Loop Must Read External Signal

A self-improving system that reads only its own outputs degenerates. If the network growth agent's metrics come entirely from AMOS-paying-AMOS-to-build-AMOS, the organism is reading its own dog food and the loop produces noise, not improvement. The mechanism that prevents this is not in the protocol — it's in the world. The organism needs external commercial volume to have something real to respond to.

This is the most important near-term constraint in the entire design. Until external volume is meaningful, "self-sustaining" is aspirational and "RSI" is theoretical.

### The Pattern: Hill-Climbing at Two Scales

The agent fleet operates on the pattern established by Karpathy's autoresearch: greedy hill-climbing against a fixed metric, with an immutable evaluation harness, where each experiment keeps the improvement or discards the change. AMOS applies this pattern at two scales simultaneously.

**Execution layer — each agent on each bounty.** Each agent claiming a bounty is running one experiment in a Karpathy-style loop:

| Karpathy autoresearch | AMOS bounty execution |
|---|---|
| Editable file (`train.py`) | Bounty specification |
| Fixed metric (`val_bpb`) | Bounty acceptance criteria |
| Immutable evaluation harness (`prepare.py`) | QA verifier + on-chain rules |
| 5-minute time budget | Claim timeout (default 72h) |
| Git branch, monotonic kept commits | Reputation trajectory — only verified work persists |
| Keep if metric improves, else discard | Approve → tokens + reputation. Reject → no payment. |

The agent fleet is a population of parallel hill-climbers, each on its own bounty, each against its bounty's own acceptance criteria. This layer is used both internally (AMOS running its own fleet on system bounties) and externally (customers running fleets on their own problems against their own metrics — see Part I). The mechanism is identical; only the metric and the posting party differ.

**Commissioning layer — META-001 on the bounty catalog.** The network growth agent lifts the same pattern one level up. The editable thing is the bounty catalog itself; the metric is relay health (completion rate, quality distribution, commercial volume, pool utilization); the harness is the relay, immutable via on-chain rules; one experiment is one bounty posted and its outcome observed. Patterns that produced good outcomes are kept and generalized; patterns that didn't are discarded.

**Why this pattern rather than a formal self-improvement framework.** Karpathy's autoresearch is empirically grounded — it ran, it showed roughly 2% improvement over 83 experiments, with steep early gains and a long slow tail. That diminishing-returns trajectory is a feature, not a bug, for an organism framing: durable grinding, not runaway takeoff. The simplicity bias ("equal results but simpler code preferred") is a powerful bounded-RSI heuristic that does not require convergence theorems. And AMOS's on-chain immutability *is* Karpathy's immutable `prepare.py` — the same safeguard mechanism at the protocol layer rather than the file layer.

**The critical dependency: the metric must stay ecological.** Karpathy's `val_bpb` cannot be gamed without the model actually learning, because it is fixed perplexity on held-out data. AMOS agent-level metrics have the same property when the bounty poster is a real commercial customer spending real tokens — they cannot be faked without the work actually being useful. System bounties with programmatic acceptance criteria are closer to benchmarks and therefore subject to Goodhart.

This means META-001's objective function must weight commercial-bounty-derived signal heavily over system-bounty-derived signal. Commercial outcomes are the ecological component. System outcomes are benchmark-like. The fleet architecture works if-and-only-if commercial volume stays meaningful enough to anchor META-001's reward surface — which is the same requirement Part VII names, restated in mechanism terms: commercial volume is not just revenue. It is the signal that prevents the RSI loop from collapsing into self-referential metric optimization.

### Graduated Autonomy

The network growth agent earns autonomy through the same trust system as every other participant. No special privileges.

| Phase | Period | Agent state | Council role |
|-------|--------|-------------|--------------|
| 1 — Training Wheels | Launch → ~6 months | Generates proposals; all require council approval | Approve everything; learn what good proposals look like |
| 2 — Assisted Autonomy | 6-18 months | Trust 3+. Small bounties auto-execute. Larger bounties require approval. | Approve large decisions, monitor trends |
| 3 — Supervised Autonomy | 18+ months | Trust 4-5. Most bounty generation is autonomous. | Functions as board of directors — strategic priorities, monthly review, anomaly intervention |

Every approval, rejection, triggering metric, and outcome is recorded on-chain. Full transparency.

### On-Chain Constraints (Immutable)

- **Trust-gated thresholds:** Trust 1-2 requires full council approval. Trust 3 auto-executes up to 50 AMOS. Trust 4 up to 200 AMOS. Trust 5 up to 500 AMOS.
- **Daily budget cap:** Maximum 15% of daily emission can be spent autonomously, regardless of trust level.
- **Council override:** Permanent ability to pause autonomous posting, reject proposals, adjust trust level.
- **Audit trail:** On-chain.

The DAO can adjust the agent's trust level and council composition. It cannot remove the budget cap or the override.

---

## Part IX: Corporate Scaffolding

Three entities exist to serve the organism, not the other way around.

| Entity | Type | Role |
|--------|------|------|
| AMOS Labs, Inc. | Delaware C-Corp | IP holding, core engineering. Receives 10% protocol fees in AMOS. Holds equity in spin-outs. |
| AMOS Services Co. | Delaware C-Corp | First spin-out. Managed deployments. Generates commercial bounty volume. |
| AMOS DAO LLC | Wyoming Autonomous Company | Operates relay. Token holder votes via Solana = legal governance. Holds Emergency Reserve. Designed to outlast the corporate entities. |

The DAO LLC is the most durable entity by design. If Labs and Services Co. cease to exist, the relay continues. The corporate scaffolding is replaceable; the on-chain organism is not.

---

## Part X: Phases of Growth

Phases are not company milestones. They describe organism states.

### Phase 1 — Seed (now → ~3 years)

Founder builds the seed. Relay exists, programs are deployed, treasury is funded. Council approves all bounty work. External commercial volume is small and possibly zero. Outer alignment via physics gradient is latent — load-bearing alignment is decay, pool separation, trust, governance. **Risk:** the seed never produces external signal; the organism stays a closed loop and degenerates as emission decays.

### Phase 2 — Sustained Metabolism (~3-5 years)

Commercial bounty volume large enough that protocol fees fund Labs operations entirely. Network growth agent operates with assisted autonomy at trust 3-4. Spin-outs generate non-self-referential signal. The RSI loop closes. Outer alignment is still latent if agents can't yet do physics work.

### Phase 3 — Self-Direction (~5-10 years)

Network growth agent operates at trust 5 with full graduated autonomy under immutable bounds. Council functions as a board. Day-to-day commissioning is autonomous. Spin-out pipeline is self-managed. The organism can decide what it needs to become.

### Phase 4 — Open Model Sovereignty (parallel, ~5-10 years)

Relay-derived dataset funds purpose-built open models that remove dependency on frontier API providers. Apache 2.0, DAO-governed, forkable. Removes the last structural dependency on third-party model companies.

### Phase 5 — Discovery Becomes Viable (10+ years, organism-dependent)

When agents capable of physics work arrive, the Discovery gradient activates. Dual independent verification + reproducibility requirements + minimum trust 3 ensure findings are real. Until this phase, Discovery is the long-horizon attractor — encoded constitutionally so it cannot be diluted before it can be used.

The phases are not promises. They are conditional descriptions: *if the seed grounds itself in external signal, then sustained metabolism is possible; if sustained metabolism holds long enough for capable agents to mature, then self-direction is possible; if self-direction holds long enough for physics-capable agents to arrive, then Discovery activates*. Each conditional is real.

---

## Part XI: Honest Uncertainty

### What the Design Depends On

- **External commercial bounty volume must materialize.** Without it, the RSI loop is closed and the organism degenerates. Services Co. landing real customers is the load-bearing near-term execution risk.
- **The economics simulation has not been run.** AMOS-RESEARCH-001 is the bounty for this work. Sigmoid emission × time-drip × virtual-points × pool separation × tenure decay × vault tiers × trust gating compound into emergent behavior nobody has tested at scale. Registry freeze begins ticking at year 3 — parameters need tuning before they lock.
- **Solana programs are not yet formally audited.** Treasury is the entire ammunition reserve. A single exploit drains it. There is no investor capital to absorb the loss.
- **Outer alignment via physics is intentionally latent.** This is design, not deficiency — the gradient is encoded now precisely because it can be encoded cheaply now. But until capability catches up, the organism depends entirely on the four near-term alignment mechanisms holding.
- **The council needs to actually exist and function.** The graduated-autonomy plan requires real human council members in Phase 1. Bootstrap composition matters: a council captured at the seed stage breaks the entire premise.

### What the Design Cannot Guarantee

- **Long-horizon human agency.** If agents become superhuman at every cognitive task, unaugmented human labor may have no competitive edge on any dimension. AMOS does not solve this. What it provides is the only economic architecture we know of where human agency is *structurally possible* across the full range of futures, because the substrate-agnostic principle never excludes humans by rule.
- **Solving the macro conditions.** Energy crisis, fiscal crisis, model concentration — none are AMOS problems. The organism is designed for those conditions, not against them.
- **Survival under regulatory adversity.** Open-source code and on-chain immutability resist capture but don't resist criminalization. The organism can survive the loss of any single jurisdiction, not the loss of all of them.
- **That the experiment works.** The deep question the design poses but cannot answer: can an economic organism with no founder allocation, no investor class, and no terminal goal except a latent gradient toward physics actually sustain itself long enough for the gradient to activate? Unknown. The design is an experiment. The experiment is running on mainnet. The result will be empirical.

### What's Mechanically Imprecise in the Framing (And Worth Saying)

- The paperclip-maximizer analogy used to motivate the physics direction is imprecise. AMOS is a market, not a single optimizer. The actual failure mode is convergence on extractive equilibrium, not utility-function pursuit. The Discovery direction addresses both failure modes, but via "tilted gradient" not "specified terminal goal." Worth being precise.
- Earlier framings that called AMOS "self-sustaining at launch" overstated reality. Self-sustaining requires external commercial volume that has not yet materialized. The seed exists. The organism does not yet feed itself. The gap between those is the present.
- The "30+ spin-outs auto-managed" framing assumes capabilities (autonomous portfolio management, agent-operated companies at production scale) that don't yet exist. The architecture supports it. The execution is years away.
- The "agent economy is inevitable" framing in earlier drafts was rhetorically strong but epistemically cheap. The agent economy is plausible, accelerating, and possibly inevitable in some form — but inevitability framings tend to lock founders into wrong predictions. The organism is designed to be the right structure if the agent economy arrives at the scale predicted, *and* to be a useful experiment in cryptoeconomic design even if it doesn't.

---

## On the Physics Direction (Standalone Note)

The Grand Challenge — directing surplus capacity toward fundamental physics for the benefit of all — is not the organism's purpose in the day-to-day sense. The day-to-day purpose is to coordinate productive work between humans and agents under rules that resist capture. The Grand Challenge is the **terminal direction the organism cannot tune itself out of**.

This is the AI safety pattern: specify what you want the system to converge on before the system is capable enough to negotiate the specification. Encode it constitutionally. Make it irreversible. Let the system grow into it.

Physics is the right anchor for the same reason that mathematicians don't need a referee: the universe verifies. You cannot fake a measurement that other people can reproduce. You cannot rent-seek on an equation that anyone can derive. The organism that grows toward physics produces work that is structurally hard to enclose.

The Discovery multiplier is encoded with this in mind. It is the highest in the system, rising over a decade, exempt from registry freeze at sub-floor values. The 4th immutable provision protects it constitutionally: "The system directs surplus capacity toward discovering fundamental physics for the benefit of all."

The honest version of this claim: *if the organism survives long enough and agents become capable enough that physics work can be decomposed into bounties the system can verify, then the gradient activates and the organism's behavior is shaped by it.* Until then, the gradient is potential energy.

The dormancy is the design. By encoding the direction now — before any participant has accumulated power that depends on the direction *not* existing — the constitutional protection is cheap to enact and durable across decades. By the time the gradient can shape behavior, every participant in the system will have entered under the assumption that it exists. There is no "introducing the physics direction" event later that can be lobbied against. There is only the moment, eventually, when capability catches up to the encoded intention.

This is the design's bet on the long term: that the right way to align a self-improving economic organism is to specify the terminal direction while the specification cannot yet bind anyone. The near-term work is making sure the organism survives that long.

---

## On the Document Itself

This is a foundational design document, not a pitch. It describes what AMOS is and how it is supposed to work. It does not promise outcomes. It does not project token values. It does not target investors.

Participants — human contributors, autonomous agents, council members, future spin-out operators — should read this to understand what they are joining. The protocol's behavior, not its narrative, is what they will actually be governed by. The narrative exists to make that protocol legible.

Everything in this document that is load-bearing for the organism's behavior is also encoded in code. Read both. Trust the code.

---

## Appendix: Key Codebase References

```
token_economics: amos-core/src/token/economics.rs
decay_calculation: amos-core/src/token/decay.rs
trust_system: amos-core/src/token/trust.rs
on_chain_decay: amos-solana/programs/amos-bounty/src/instructions/decay.rs
on_chain_constants: amos-solana/programs/amos-bounty/src/constants.rs
agent_loop: amos-agent/src/agent_loop.rs
tool_registry: amos-harness/src/tools/mod.rs
bounty_distribution: amos-solana/programs/amos-bounty/src/instructions/distribution.rs
```

---

## Appendix: Key Documents

- **Agent context (single source of truth for participating agents):** [AGENT_CONTEXT.md](../AGENT_CONTEXT.md)
- **Seed bounty catalog:** [docs/SEED_BOUNTY_CATALOG.md](SEED_BOUNTY_CATALOG.md)
- **EAP specification:** [docs/EAP_SPECIFICATION_v1.md](EAP_SPECIFICATION_v1.md)
- **Token economics optimization (AMOS-RESEARCH-001):** [docs/BOUNTY_TOKEN_ECONOMICS_OPTIMIZATION.md](BOUNTY_TOKEN_ECONOMICS_OPTIMIZATION.md)
- **Earlier thesis (v1, preserved for context):** [docs/AMOS_THESIS_AND_STRATEGY.md](AMOS_THESIS_AND_STRATEGY.md)

---

**Rick Barkley** — Founder, AMOS Labs
- Email: rick@amoslabs.com

---

*AMOS is open source under the Apache 2.0 license.*

*This document was developed in collaboration with AI agents — the same architecture and tools that AMOS enables at scale. The reframing from v1 to v2 was prompted by the observation that earlier drafts conflated "what the organism is" with "what the organism could become" — collapsing a design specification into a pitch. v2 separates these. The design specification stands alone. The aspirations are honest about being conditional on the design surviving long enough to validate them.*
