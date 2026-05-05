# Subscription, Onboarding, and BYOK Strategy

**May 2026 | AMOS Labs**

Status: Design doc — not yet implemented.

---

## Premise

AMOS is open infrastructure for autonomous work. Its audience is **tech-curious business owners** — operators who run real businesses, want autonomous systems working for them, and are comfortable learning new tools when something is genuinely useful. Most don't have AWS accounts today. They aren't afraid to set one up if AMOS walks them through it.

This is a much bigger market than self-serve developers. It's also a more demanding one in one specific way: the **onboarding agent has to actually work**, because the user doesn't have prior IAM knowledge to fall back on if the script confuses them.

Signup should be **as easy as possible for a curious business owner who has never opened the AWS console before but trusts a competent agent to guide them through it**. Not as easy as possible for everyone (we are not Claude.ai). Not gated behind prior cloud experience either.

Concretely: **BYOK is the path**, and AMOS itself is the thing that walks you to your key — patiently, with screenshots, in plain language.

---

## The Plan

### Subscription

- **$49.99 / month** flat subscription, billed via Stripe.
- **$5 of Haiku-only Bedrock credits** included each month, granted on subscription renewal.
- Credits **reset** at each billing cycle. No rollover. Use it or BYOK.
- Opus and Sonnet require BYOK from day one — credits do not apply to them.

### Why this shape

- $49.99 is in the comfortable-out-of-pocket zone for a tech-curious owner — same mental bucket as Notion, QuickBooks, or any tool that does something genuinely useful. Lower numbers attract noise; higher numbers trigger a "let me think about it" decision that kills momentum.
- $5 of Haiku ≈ 200–500 substantive messages. Enough to feel real on day one. Not enough to be a primary backend.
- Hard-gating Opus/Sonnet to BYOK caps worst-case shared-Bedrock burn at $5/tenant/month. No bad-debt exposure.
- Monthly reset (vs. rollover) keeps the math trivial — one column, one webhook, no aging logic.

### Margin (rough)

| Item | Per tenant per month |
|---|---|
| Gross subscription | $49.99 |
| Stripe fees (2.9% + $0.30) | -$1.75 |
| AWS infra (ECS task share + RDS schema + ALB) | -$8 to -$15 |
| Bedrock credits (worst case, fully consumed) | -$5 |
| **Net margin** | **$28 to $35** |

Healthy enough to cover support and engineering time; tight enough to require care on infra cost per tenant.

---

## Onboarding Flow

A user's first AMOS conversation is with an **onboarding agent** that walks them through getting a Bedrock key. The user is assumed to be smart and motivated, *not* to have prior AWS experience. Eating our own dogfood — if the agent can't reliably guide a tech-curious owner through IAM setup, AMOS isn't ready to ship to that audience.

### User journey

1. **Signup** — email + password + Stripe card on file. Subscription begins.
2. **Provisioning** — harness comes up, $5 credit grant lands on the tenant.
3. **First message lands the user in the onboarding chat.** System prompt for that initial session is the key-setup walkthrough. Default model is Haiku 4.5 (the only thing the credit covers).
4. **The onboarding agent asks once:** "Do you already have an AWS account, or should we set one up together?" If they don't, the agent links them to AWS's signup page and tells them what to do when they come back. (We don't try to walk through AWS account creation itself — that's AWS's onboarding and we don't want to be in the middle of it.)
5. **Once they're logged into AWS**, the agent walks them through IAM step-by-step in plain language: what IAM is, why we need it, exactly which policy to attach, where to find the access keys. Confidence-building copy at the moments people typically panic ("This policy only lets AMOS call Bedrock — it can't see your other AWS resources").
6. **Validates the key when pasted** by making a test call to Bedrock. Fail loud and fix it together if the test call fails ("Looks like the policy isn't attached yet — let's check that"). Don't let users save a broken key.
7. **Once a working key is saved**, the harness flips that tenant to BYOK mode. Credits remain available as a fallback for Haiku-only quick tasks.

Realistic time budget: **15–20 minutes** for a first-time AWS user, including signup if needed. That's totally acceptable for setting up the tool you're going to run your business on.

### Walking through provider rate limits

Both providers throttle new accounts. The agent has to set realistic expectations *and* help users raise limits when they hit them — otherwise the user blames AMOS for "being slow" when it's their provider account.

**Anthropic direct** has explicit usage tiers:
- **Tier 1** (default for new keys): low requests-per-minute and tokens-per-minute caps, monthly spend ceiling. Users will hit these fast on Opus or sustained Sonnet workloads.
- **Tier 2** unlocks higher caps but requires a deposit (~$40) and a multi-day waiting period.
- Higher tiers require larger deposits and longer waits.

The agent walks users through requesting the next tier in the Anthropic Console (Plans page → upgrade request). Sets the expectation that the upgrade isn't instant — Anthropic holds the deposit for several days before approval. Suggests using Bedrock in parallel if they want immediate higher limits.

**AWS Bedrock** doesn't have per-key tiers but does have account-level quotas (per-model TPM/RPM in AWS Service Quotas). Default limits are generous enough that most users won't hit them, but if they do, the agent walks them through requesting an increase via the Service Quotas console. Approval is usually a few hours to a day.

The walkthrough doesn't try to *do* the limit request for the user — they have to fill it out themselves on the provider's site. The agent's job is to make sure they know how, where, and what to expect.

### Hard cutoff (not graceful degrade)

When credits hit zero mid-month and no BYOK key is set, the harness returns a clean 402 with "Credits exhausted — add a Bedrock key to continue this month." Banner in the UI directs them back to the onboarding agent.

Graceful degrade (let them keep going on goodwill) reintroduces exactly the bad-debt problem this design is built to avoid. Don't.

### Why this is doing double duty

The BYOK walkthrough isn't only about getting an LLM key. Once a user has set up an IAM policy and access keys for AMOS, that trust relationship is the foundation for future product surface area:

- **BYOC harness hosting** — provision their tenant in their own AWS account (data residency, compliance, zero AMOS infra cost).
- **Customer-owned S3 for proof receipts** — sensitive artifacts in their bucket, not yours.
- **VPC peering** — for users who need AMOS to talk to private resources.
- **Cost transparency** — Bedrock charges land on their AWS bill directly. No markup confusion.

The friction of BYOK pays off twice. We don't ship any of this on day one, but the foothold is what makes it possible later.

---

## Build Shape

### Schema

One column on the existing tenant/customer table:

```sql
ALTER TABLE customer_subscriptions
    ADD COLUMN credit_balance_microcents BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN credits_granted_at TIMESTAMPTZ;
```

Microcents (1/100 of a cent) matches the existing `cost_microcents` convention in `amos-platform/src/billing/metered_billing.rs`. $1 = 10,000 microcents, so **$5 = 50,000 microcents**.

### Stripe webhooks

Two events to wire up in `amos-platform`:

- `customer.subscription.created` → grant $5 credit, set `credits_granted_at = now()`.
- `invoice.payment_succeeded` (recurring) → reset balance to $5, update `credits_granted_at`.

If a user cancels mid-cycle, credits remain valid until the period ends (Stripe handles the period boundary).

### Harness pre-call check

In `amos-harness/src/routes/agent_proxy.rs`, before dispatching to Bedrock:

```
if byok_key_configured(tenant):
    use BYOK path (existing)
elif model_id is Haiku AND credit_balance_microcents > estimated_cost:
    use shared Bedrock, deduct after call
else:
    return 402 with "Set up a key" CTA
```

Deduction happens in the same transaction that writes `llm_usage_records` — atomic with the existing token accounting.

### Onboarding persona

A versioned, checked-in walkthrough script at (proposed) `amos-harness/prompts/onboarding_walkthrough.md`. Loaded as the system prompt for any session where the tenant has no BYOK key configured.

Improv onboarding is a bad idea — a single hallucinated IAM policy gives the user a worse first impression than handing them docs would. The agent reads from a tested script and does the talking, not the inventing.

---

## What This Design Does Not Build

Explicitly out of scope, even though they're tempting:

- **Stripe Usage Records / metered overage billing.** Credits are a fixed entitlement, not a metered ceiling. If a user wants more than $5 of shared Bedrock per month, they BYOK. We do not bill for additional shared usage.
- **Top-up flow / prepaid credit purchase.** No Stripe Checkout for credit packs. Same reason.
- **Credit rollover.** Adds aging logic, recurring user complaints about expired balances, and accounting nuance for no real benefit.
- **Sonnet/Opus on shared backend.** Even with credits. Hard gate.
- **LLM token resale at markup** (the "own the infra play"). Considered and rejected as a near-term direction. AMOS's wedge is the protocol and runtime, not commodity gateway business. Revisit if BYOK conversion data demands it.

---

## Open Decisions

To resolve before implementation:

1. **Onboarding walkthrough content.** Need a tested script for both Bedrock and Anthropic-direct paths, written for someone who has never used AWS before. Must include rate-limit expectation-setting *and* the limit-raise procedure for each provider (Anthropic tier upgrade, AWS Service Quotas request). Probably a half-day writing exercise *plus* validation against a freshly-created AWS account by someone who matches the target user. Should live as `amos-harness/prompts/onboarding_walkthrough.md`. The bar is "my parent could follow this," not "my coworker could follow this."
2. **AWS signup handoff.** Confirmed: agent does NOT walk users through AWS account creation itself — links to AWS's signup, tells them what to do when they get back. Account creation is a 10-minute distinct flow on AWS's side and they own it.
3. **BYOK key validation method.** Bedrock: small `InvokeModel` call against `claude-haiku-4-5` with a 1-token prompt. Anthropic-direct: same. Validates on save in settings; fail loud, not silent.
4. **What does the user see when credits hit zero?** Probably: existing chat UI returns 402, banner appears at top of harness with "Credits exhausted this cycle. [Set up Bedrock key →]". Clean, not buried.
5. **Trial period before first Stripe charge?** Probably no — paid signup from day one filters audience. But worth confirming before building.

---

## Implementation Order (when greenlit)

1. Schema migration (`credit_balance_microcents`, `credits_granted_at`).
2. Stripe webhook handlers in `amos-platform` for the two events.
3. Harness pre-call gate in `agent_proxy.rs`.
4. Onboarding walkthrough script — written, reviewed, checked in.
5. Settings page UX: BYOK setup form with test-key button + per-provider validation.
6. Frontend: balance banner + credits-exhausted state.

Estimated scope: ~3–4 days of focused work end to end. Most of the cost is the walkthrough script and the test-key validation flow, not the credit accounting itself.

---

## Verification

When live:

- Sign up a fresh test tenant. Verify $5 credit lands within seconds of subscription activation.
- Send a Haiku message. Verify `credit_balance_microcents` decreases by the expected cost (visible in DB).
- Try sending an Opus message without BYOK. Verify clean 402 with the right CTA.
- Configure a BYOK Bedrock key. Verify subsequent calls use BYOK and do not deduct from credits.
- Burn through the $5. Verify hard cutoff with banner, no goodwill overrun.
- Wait one billing cycle. Verify credits reset to $5 on `invoice.payment_succeeded`.
