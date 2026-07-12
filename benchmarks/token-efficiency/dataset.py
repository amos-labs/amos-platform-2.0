"""Shared, deterministic Northwind Labs dataset for the token-efficiency benchmark.

BOTH arms read the SAME data from here, so the only variable between arm A
(curated AMOS verbs) and arm B (raw SQL) is the *surface*, never the data.

Provenance — these fixtures are ported verbatim from the AMOS platform's
demo/playground seed so the benchmark exercises the real demo company:

  * Finance lake (customers / invoices / payments / stripe charges):
    amos-platform `src/mcp/ingestion/demo_seed.rs` (Northwind Labs finance lake).
    Dates there are *relative to `as_of`*; we pin `as_of = 2026-07-12` (the same
    value that module's tests pin) and materialise absolute dates so the
    dataset is fully deterministic and reproducible.

  * CRM (customers MRR book + deals pipeline):
    amos-platform `src/mcp/starters.rs` `saas_playground` starter.

  * Documents (board update / pricing / onboarding runbook):
    the `DEMO_DOCS` array in `demo_seed.rs`.

Benchmark-only extension (documented so a skeptic can audit it): the deals
pipeline in `starters.rs` has no per-deal "last activity" timestamp, which the
stale-deals task needs. We add a `last_activity_date` to each deal, derived
deterministically from its stage and close_date. This field is identical for
both arms (it lives in the shared table), so it does not bias the comparison —
it only makes the stale-deals task well-defined.

The data is loaded into an in-memory SQLite database. Arm B queries it directly
via a `run_sql` tool. Arm A never sees SQL — it calls curated Python functions
that mirror the AMOS MCP verbs (`money_flow`, `finance_leaks`, `list_records`,
`get_catalog`, `search_documents`) and return the same pre-digested shapes the
platform returns.
"""

from __future__ import annotations

import datetime as _dt
import sqlite3
from dataclasses import dataclass

# The pinned analysis date. demo_seed.rs shapes its fixtures relative to this and
# its tests pin exactly this value; we reuse it so money_flow / finance_leaks
# fire their intended findings deterministically.
AS_OF = _dt.date(2026, 7, 12)
CURRENCY = "USD"


def _iso(d: _dt.date) -> str:
    return d.isoformat()


def _rel(days: int) -> str:
    """A date `days` before AS_OF, as ISO — mirrors demo_seed.rs `d(days)`."""
    return _iso(AS_OF - _dt.timedelta(days=days))


# ── Finance lake fixtures (ported from demo_seed.rs) ─────────────────────────
# QBO customers — Globex is the concentration whale.
_QBO_CUSTOMERS = [
    ("cus_globex", "Globex Corporation"),
    ("cus_initech", "Initech"),
    ("cus_hooli", "Hooli"),
    ("cus_umbrella", "Umbrella Retail"),
    ("cus_stark", "Stark Industries"),
]

# Invoices: (id, customer_id, status, total_dollars, issued_rel, due_rel)
# Engineered so the leak findings fire (see demo_seed.rs::invoices docstring).
_INVOICES = [
    ("inv_glx_1", "cus_globex", "open", 30000, 40, 10),
    ("inv_glx_2", "cus_globex", "paid", 24000, 70, 40),
    ("inv_ini_old", "cus_initech", "open", 12000, 140, 110),
    ("inv_hoo_old", "cus_hooli", "open", 9000, 130, 100),
    ("inv_umb_mid", "cus_umbrella", "open", 6000, 65, 50),
    ("inv_stk_mid", "cus_stark", "open", 9000, 50, 40),
    ("inv_prior_1", "cus_initech", "paid", 6000, 170, 140),
    ("inv_prior_2", "cus_hooli", "paid", 6000, 165, 135),
    ("inv_prior_3", "cus_stark", "paid", 6000, 160, 130),
    ("inv_cur_1", "cus_hooli", "paid", 6000, 80, 50),
    ("inv_cur_2", "cus_umbrella", "paid", 6000, 75, 45),
    ("inv_recon", "cus_stark", "paid", 18000, 35, 5),
]

# Payments against QBO invoices: (id, invoice_id, amount_dollars, received_rel)
_PAYMENTS = [
    ("pay_prior_1", "inv_prior_1", 6000, 163),
    ("pay_prior_2", "inv_prior_2", 6000, 158),
    ("pay_prior_3", "inv_prior_3", 6000, 153),
    ("pay_glx_2", "inv_glx_2", 24000, 40),
    ("pay_cur_1", "inv_cur_1", 6000, 50),
    ("pay_cur_2", "inv_cur_2", 6000, 45),
    ("pay_recon", "inv_recon", 18000, 10),
]

# Stripe charges: (id, reconcile_ref|None, amount_dollars, received_rel)
_STRIPE_CHARGES = [
    ("ch_1", None, 9000, 12),
    ("ch_2", None, 1800, 20),
    ("ch_3", None, 1800, 28),
    ("ch_4", None, 900, 35),
    ("ch_5", None, 900, 50),
    ("ch_recon", "inv_recon", 18000, 10),
]

# ── CRM fixtures (ported from starters.rs saas_playground) ───────────────────
# customers: (name, plan, mrr, signup_date, status)
_CRM_CUSTOMERS = [
    ("Globex Corporation", "enterprise", 9000, "2025-06-10", "active"),
    ("Initech", "business", 1800, "2025-08-02", "active"),
    ("Hooli", "business", 1800, "2025-09-19", "active"),
    ("Umbrella Retail", "team", 900, "2025-10-05", "active"),
    ("Wonka Foods", "team", 900, "2025-11-11", "active"),
    ("Stark Industries", "business", 1800, "2025-12-01", "active"),
    ("Wayne Enterprises", "team", 900, "2026-01-20", "active"),
    ("Cyberdyne Systems", "starter", 300, "2026-02-14", "active"),
    ("Soylent Co", "starter", 300, "2026-03-03", "active"),
    ("Pied Piper", "starter", 300, "2026-04-08", "trial"),
    ("Vandelay Industries", "team", 900, "2026-04-22", "trial"),
    ("Dunder Mifflin", "starter", 0, "2025-07-30", "churned"),
]

# deals: (title, stage, amount, close_date). last_activity_date is derived below.
_DEALS = [
    ("Globex — seat expansion", "proposal", 36000, "2026-08-15"),
    ("Initech — annual upgrade", "won", 21600, "2026-05-30"),
    ("Hooli — platform tier", "qualified", 24000, "2026-09-01"),
    ("Vandelay — pilot to paid", "proposal", 10800, "2026-07-20"),
    ("Pied Piper — trial conversion", "qualified", 3600, "2026-06-30"),
    ("NewCo Robotics — inbound", "lead", 12000, "2026-10-10"),
    ("Acme Freight — outbound", "lost", 9000, "2026-04-15"),
    ("Stark — multi-year", "won", 64800, "2026-03-28"),
]

# Benchmark-only: deterministic days-since-last-activity per deal title. Chosen so
# a subset of OPEN deals (not won/lost) are clearly stale (>30 days idle) and the
# rest are recent — makes the stale-deals task well-defined. Applied identically
# to both arms.
_DEAL_IDLE_DAYS = {
    "Globex — seat expansion": 42,   # stale
    "Initech — annual upgrade": 43,  # won (excluded from stale follow-ups)
    "Hooli — platform tier": 9,      # recent
    "Vandelay — pilot to paid": 51,  # stale
    "Pied Piper — trial conversion": 38,  # stale
    "NewCo Robotics — inbound": 61,  # stale
    "Acme Freight — outbound": 88,   # lost (excluded)
    "Stark — multi-year": 106,       # won (excluded)
}

# Documents (ported from demo_seed.rs DEMO_DOCS): (filename, title, body)
DEMO_DOCS = [
    (
        "q3-board-update.md",
        "Northwind Labs — Q3 Board Update",
        "# Northwind Labs — Q3 Board Update\n\n"
        "**Stage:** Seed. **Age:** ~14 months. **Headcount:** 9.\n\n"
        "## Revenue\n"
        "Net MRR is up to roughly $18.9K across 11 paying accounts, with two active trials "
        "(Pied Piper, Vandelay). Globex remains our largest account at ~45% of MRR — a "
        "concentration risk the team is actively working to reduce by closing Hooli and the "
        "Vandelay pilot.\n\n"
        "## Collections\n"
        "Days-sales-outstanding worsened this quarter: the typical invoice now takes ~30 days "
        "to collect versus ~7 days in the prior period. Two enterprise invoices (Initech, Hooli) "
        "are more than 90 days past due and at risk. Finance is prioritizing outreach on the "
        "oldest, largest balances first.\n\n"
        "## Asks of the board\n"
        "1. Intros to two mid-market logos to dilute the Globex concentration.\n"
        "2. Approval to hire a part-time collections/AR contractor for one quarter.\n",
    ),
    (
        "pricing-tiers.md",
        "Northwind Labs — Pricing Tiers",
        "# Pricing Tiers\n\n"
        "Northwind Labs sells four subscription tiers, billed monthly.\n\n"
        "| Tier | Price / mo | Best for |\n"
        "|------|-----------|----------|\n"
        "| Starter | $300 | Solo teams getting started |\n"
        "| Team | $900 | Growing teams up to 25 seats |\n"
        "| Business | $1,800 | Multi-team orgs needing SSO + roles |\n"
        "| Enterprise | $9,000 | Custom deployments, SLA, dedicated support |\n\n"
        "Annual prepay earns two months free. Trials run 14 days on the Team tier by default. "
        "Enterprise pricing is negotiated and always includes onboarding.\n",
    ),
    (
        "onboarding-runbook.md",
        "Northwind Labs — Customer Onboarding Runbook",
        "# Customer Onboarding Runbook\n\n"
        "Goal: a new customer reaches first value within 7 days.\n\n"
        "1. **Kickoff (day 0):** confirm the primary use case and success metric; create the "
        "workspace; invite admins.\n"
        "2. **Data in (days 1–2):** import the customer's existing data or connect their source "
        "system. Verify counts match.\n"
        "3. **First workflow (days 3–4):** stand up the one workflow tied to their success "
        "metric; watch them run it once live.\n"
        "4. **Team rollout (days 5–7):** invite the wider team, set roles, schedule a 30-day "
        "check-in.\n\n"
        "Escalate any blocked onboarding older than 3 days to the founder. Trials that stall in "
        "step 2 are the top conversion leak.\n",
    ),
]


@dataclass(frozen=True)
class Dataset:
    conn: sqlite3.Connection


def build_dataset() -> Dataset:
    """Materialise the shared dataset into a fresh in-memory SQLite DB."""
    conn = sqlite3.connect(":memory:")
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    cur.executescript(
        """
        CREATE TABLE finance_customers (id TEXT PRIMARY KEY, name TEXT, email TEXT);
        CREATE TABLE invoices (
            id TEXT PRIMARY KEY, customer_id TEXT, status TEXT,
            total_cents INTEGER, currency TEXT, issued_at TEXT, due_at TEXT
        );
        CREATE TABLE payments (
            id TEXT PRIMARY KEY, invoice_id TEXT, amount_cents INTEGER,
            currency TEXT, received_at TEXT
        );
        CREATE TABLE stripe_charges (
            id TEXT PRIMARY KEY, reconcile_ref TEXT, amount_cents INTEGER,
            currency TEXT, received_at TEXT
        );
        CREATE TABLE crm_customers (
            name TEXT PRIMARY KEY, plan TEXT, mrr INTEGER, signup_date TEXT, status TEXT
        );
        CREATE TABLE deals (
            title TEXT PRIMARY KEY, stage TEXT, amount INTEGER,
            close_date TEXT, last_activity_date TEXT
        );
        CREATE TABLE documents (
            filename TEXT PRIMARY KEY, title TEXT, body TEXT
        );
        """
    )

    cur.executemany(
        "INSERT INTO finance_customers VALUES (?,?,?)",
        [(cid, name, f"billing@{name.lower().replace(' ', '')}.example") for cid, name in _QBO_CUSTOMERS],
    )
    cur.executemany(
        "INSERT INTO invoices VALUES (?,?,?,?,?,?,?)",
        [
            (iid, cust, status, total * 100, CURRENCY, _rel(issued), _rel(due))
            for iid, cust, status, total, issued, due in _INVOICES
        ],
    )
    cur.executemany(
        "INSERT INTO payments VALUES (?,?,?,?,?)",
        [(pid, inv, amt * 100, CURRENCY, _rel(rec)) for pid, inv, amt, rec in _PAYMENTS],
    )
    cur.executemany(
        "INSERT INTO stripe_charges VALUES (?,?,?,?,?)",
        [(cid, ref, amt * 100, CURRENCY, _rel(rec)) for cid, ref, amt, rec in _STRIPE_CHARGES],
    )
    cur.executemany(
        "INSERT INTO crm_customers VALUES (?,?,?,?,?)",
        _CRM_CUSTOMERS,
    )
    cur.executemany(
        "INSERT INTO deals VALUES (?,?,?,?,?)",
        [
            (title, stage, amount, close, _iso(AS_OF - _dt.timedelta(days=_DEAL_IDLE_DAYS[title])))
            for title, stage, amount, close in _DEALS
        ],
    )
    cur.executemany(
        "INSERT INTO documents VALUES (?,?,?)",
        DEMO_DOCS,
    )
    conn.commit()
    return Dataset(conn=conn)


if __name__ == "__main__":
    ds = build_dataset()
    c = ds.conn.cursor()
    for tbl in ("finance_customers", "invoices", "payments", "stripe_charges",
                "crm_customers", "deals", "documents"):
        n = c.execute(f"SELECT COUNT(*) FROM {tbl}").fetchone()[0]
        print(f"{tbl}: {n} rows")
