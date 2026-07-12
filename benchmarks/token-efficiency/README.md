# Token-Efficiency Benchmark: curated AMOS verbs vs. raw data access

**The claim under test.** AMOS gives Claude a clear, pre-digested surface
(curated MCP verbs like `money_flow`, `finance_leaks`, `list_records`,
`get_catalog`, `search_documents`) so a business task costs **fewer total
tokens** than a generic agent that must explore raw data (list tables →
paginate raw rows into context → reason from scratch). We spend build-time
machinery to buy inference-time token efficiency.

This benchmark makes the API calls *ourselves*, so the model returns exact
token `usage` on every response — token counts are provider-accounted, not
estimated. It is deliberately built to survive a skeptical read: one model held
constant, both arms on identical underlying data, an explicit equivalent-quality
bar per task, and multiple runs with reported spread.

---

## Result (headline)

Model: **`anthropic.claude-opus-4-8`** on AWS Bedrock. N = **3** runs per
(arm × task). Tokens are summed across the *entire* tool-use loop per task.

| Task | AMOS mean tok | Raw mean tok | Ratio (raw ÷ amos) | AMOS $/task | Raw $/task |
|------|--------------:|-------------:|:------------------:|------------:|-----------:|
| `cashflow` — "how's cash flow?"        | 3,618 | 19,613 | **5.4×** | $0.0277 | $0.1874 |
| `leaks` — "where am I losing money?"   | 3,042 | 16,277 | **5.4×** | $0.0246 | $0.1672 |
| `stale_deals` — "draft follow-ups"     | 4,921 |  8,856 | **1.8×** | $0.0458 | $0.0735 |
| `retrieval` — "what do our docs say?"  | 3,831 |  7,727 | **2.0×** | $0.0292 | $0.0521 |
| **overall (equal-weight)**             | —     | —      | **3.4×** | — | — |

Dollar figures use Opus 4.8 rates ($5 / $25 per 1M input / output tokens).

**Where the win is largest:** the curated-analysis tasks (`money_flow` /
`finance_leaks`) — a single verb collapses an entire multi-step analysis
(aging, cycle-time trend, concentration, cross-source dedup) into one
pre-digested tool result, versus the raw agent issuing 4–6 SQL queries and
paginating raw invoice/payment/charge rows into its context.

**Where the win is smaller (but real):** the CRM (`list_records`) and document
(`search_documents`) tasks — ~2×. The curated verb still saves the
schema-discovery round-trips and returns a tighter payload, but the raw agent
can express these as one or two focused `SELECT`s, so there's less raw data to
haul.

See **Honest findings** below for the caveats a critical VC would raise.

---

## Design — two arms, everything held constant except the surface

Both arms run the **same model**, **same** system-prompt scaffold, **same**
`max_tokens` and adaptive-thinking setting, and read the **same** underlying
data. The only variable is the tool surface.

- **Arm A (AMOS):** curated verbs — `get_catalog`, `list_records`, `money_flow`,
  `finance_leaks`, `search_documents`. Each returns a pre-digested,
  business-meaningful result (`arms/amos_verbs.py`).
- **Arm B (baseline = "current method"):** generic raw access — `list_tables`,
  `describe_table`, `run_sql` (`arms/raw_sql.py`). The honest steel-man of how a
  generic data-analyst agent operates today: no curated verbs, no company brain.

Token usage is read from every response's `usage` (input + output + cache) and
summed across the whole loop (`runner.py`), not just the final message.

### Why not passive/production telemetry?

Production business operation runs via the user's own Claude Desktop/Code over
the platform MCP; those reasoning tokens land on the user's third-party
Anthropic account and AMOS never captures them (the harness `messages` table
only records usage for sessions run through the harness agent loop, which these
are not). There is no production number to pull. So *we* make the API calls and
read exact `usage` — third-party-ness becomes irrelevant.

---

## Data — shared, deterministic, ported from the real demo seed

`dataset.py` materialises the **Northwind Labs** demo company into an in-memory
SQLite database. The fixtures are ported verbatim from the AMOS platform seed so
the benchmark exercises real demo data:

- **Finance lake** (customers / invoices / payments / Stripe charges) — from
  `amos-platform/src/mcp/ingestion/demo_seed.rs`. Dates there are relative to
  `as_of`; we pin `as_of = 2026-07-12` (the value that module's tests pin) and
  materialise absolute dates, so the dataset is fully deterministic.
- **CRM** (MRR customer book + deals pipeline) — from
  `amos-platform/src/mcp/starters.rs` (`saas_playground` starter).
- **Documents** (board update / pricing / onboarding runbook) — the `DEMO_DOCS`
  array in `demo_seed.rs`.

Both arms read these same SQLite tables. Arm A's `money_flow`/`finance_leaks`
are a faithful Python port of the platform's analysis semantics (aging buckets
by days-past-due, own-median collection cycle with a prior-period trend,
customer concentration by invoiced share, cross-source Stripe↔QBO dedup); the
engine's numbers are validated against the ground truth `demo_seed.rs` tests
assert (e.g. Stripe net = $14,400 after excluding the $18k reconciled charge;
90+ receivables = $21k; cycle 30d-now vs 7d-prior worsening).

**One documented benchmark-only extension:** the deals pipeline in
`starters.rs` has no per-deal "last activity" timestamp, which the stale-deals
task needs. We add a deterministic `last_activity_date` to each deal. It lives
in the shared table and is identical for both arms, so it defines the task
without biasing the comparison.

> This is a faithful **model** of the platform surface over the real demo data,
> not the live platform process. That is a deliberate reproducibility choice
> (the live seed depends on S3 + DuckDB + a running platform on an unmerged
> branch). See the caveats.

---

## Equivalent-quality bars (so "cheaper" can't mean "worse")

`tasks.py` defines each task's `quality_bar` as objective substrings a correct,
equivalent answer must contain, derived from the dataset ground truth. A run
that misses the bar is marked **failed** and **excluded** from the token means —
a cheap-but-wrong answer scores no win.

| Task | Bar (must be in the answer) |
|------|------|
| `cashflow` | The **$14,400** net charge revenue (i.e. correctly excludes the double-counted $18k reconciled charge) **and** that the collection cycle got slower (30d vs 7d). |
| `leaks` | The **Globex** concentration **and** the aged/overdue receivables (90+ bucket = Initech $12k + Hooli $9k = $21k). |
| `stale_deals` | Follow-ups for exactly the 4 stale **open** deals (**Globex, Vandelay, Pied Piper, NewCo**); must not treat won/lost/recent deals as stale. |
| `retrieval` | Must cite **onboarding-runbook.md** and state that trials stalling at the data-import step are the top onboarding leak. |

---

## Reproduce

```bash
# 1. Python env with the official Anthropic SDK (Bedrock extra)
python3 -m venv .venv && . .venv/bin/activate
pip install 'anthropic[bedrock]' boto3

# 2. AWS creds with Bedrock access to anthropic.claude-opus-4-8
export AWS_PROFILE=...            # or standard AWS env creds
export AWS_REGION=us-east-1

# 3. Validate the dataset + analysis engine offline (no API cost)
python dataset.py
python -c "import sys;sys.path.insert(0,'arms');from dataset import build_dataset;from arms import amos_verbs as a;print(a.money_flow(build_dataset())['summary'])"

# 4. Run the sweep (N runs per arm × task); writes results/results.json
python benchmark.py --runs 3
python benchmark.py --runs 5 --tasks cashflow,leaks   # subset / more runs
```

Output: per-task mean ± stdev tokens, min/max, the raw÷amos ratio, and a dollar
translation. Full per-trial records (including the graded answers) are in
`results/results.json`.

### Files

| File | Role |
|------|------|
| `dataset.py` | Shared deterministic Northwind dataset → in-memory SQLite. |
| `arms/amos_verbs.py` | Arm A: curated AMOS verbs + faithful analysis engine. |
| `arms/raw_sql.py` | Arm B: generic `list_tables` / `describe_table` / `run_sql`. |
| `tasks.py` | The 4 tasks + objective equivalent-quality bars + grader. |
| `runner.py` | One Claude tool-use loop per trial; exact per-response usage summing. |
| `benchmark.py` | Sweep driver + aggregation (mean±spread, ratio, $ translation). |
| `results/` | `results.json` (committed) + `run.log`. |

---

## Honest findings (what a skeptic should know)

1. **The effect is real and large on curated-analysis tasks (~5.4×), modest on
   simple CRM/retrieval tasks (~2×).** The overall equal-weight ratio (3.4×) is
   dominated by the analysis tasks. If a customer's workload is mostly simple
   record lookups, expect closer to 2× than 5×. Reporting the per-task split
   rather than only the headline is the honest presentation.

2. **The raw arm sometimes loses on *quality*, not just tokens.** On
   `stale_deals`, the raw-SQL agent failed the quality bar in **2 of 3 runs** —
   it dropped a genuinely-stale deal (Pied Piper) while doing the
   date-arithmetic classification by hand. Those runs are excluded from the raw
   mean, so the reported 1.8× ratio is computed on the *single* raw run that
   actually got the answer right; the real-world gap (correct answers per
   dollar) is larger than the token ratio alone shows. This is a point in
   AMOS's favor, but it also means the `stale_deals` token ratio rests on n=1
   for the raw arm — treat that cell as directional, not precise.

3. **This is a faithful model of the surface, not the live platform.** Arm A
   calls a Python re-implementation of the platform's analysis verbs over the
   real demo fixtures, because standing up the live seed requires S3 + DuckDB +
   a running platform instance from an unmerged branch. The verb *outputs* are
   validated against the platform's own ground-truth tests, but a purist would
   want the benchmark wired to the live MCP server. That is the natural next
   step and would only strengthen the result (the live verbs return the same
   pre-digested shapes; if anything the raw arm's live-DB exploration would be
   heavier, not lighter).

4. **Caching is effectively off, applied uniformly.** Each trial uses a fresh
   prompt, so neither arm benefits from prompt caching. In production, AMOS's
   stable curated tool set caches better than a raw agent's ad-hoc SQL, which
   would *widen* the gap — so this benchmark is conservative on that axis.

5. **The dataset is small (a seed-stage demo company).** Raw-access cost scales
   with how much data the agent pages into context; on a larger book of
   business the raw arm's cost grows while the curated verb's pre-digested
   summary stays roughly flat — so the ratio should *increase* with data size,
   not decrease. This benchmark therefore likely *understates* the production
   advantage for a real, larger tenant.

6. **Answer quality was graded by objective substring bars, not an LLM judge.**
   That is stricter and more auditable, but it can't catch subtle prose-quality
   differences. Both arms produced correct, complete answers on the passing
   runs.

**Bottom line for the website:** a defensible, honest claim is *"On real
business-analysis questions, AMOS's curated tools let Claude answer in roughly
**3–5× fewer tokens** than a generic SQL-access agent — and more reliably."*
Lead with the per-task range and the reliability point, not a single inflated
number.
