"""Arm A surface: curated AMOS MCP verbs over the shared dataset.

These functions mirror the AMOS platform's curated MCP verb surface. Each returns
the same *pre-digested* shape the real platform returns, so Claude never has to
reconstruct business meaning from raw rows:

  * money_flow      → amos-platform src/mcp/analysis (compute_money_flow + summary)
  * finance_leaks   → amos-platform src/mcp/analysis/leaks (top_leaks + summary)
  * get_catalog     → amos-platform src/mcp/data.rs (collection catalog)
  * list_records    → amos-platform src/mcp/data.rs (records in a collection)
  * search_documents→ amos-platform doc store (grounded snippets over docs)

The analysis math is a faithful port of the Rust model's semantics (aging
buckets by days-past-due, own-median collection cycle with a prior-period trend,
customer concentration by invoiced share, and cross-source Stripe/QBO dedup).
It reads the SAME SQLite tables Arm B queries with raw SQL, so both arms operate
on identical underlying data.
"""

from __future__ import annotations

import datetime as _dt
from typing import Any

from dataset import AS_OF, Dataset

_EVIDENCE_CAP = 5  # mirror model.rs EVIDENCE_CAP


def _d(s: str) -> _dt.date:
    return _dt.date.fromisoformat(s[:10])


def _dollars(cents: int) -> str:
    return f"${cents / 100:,.0f} USD"


# ── the money-flow model (faithful port of compute_money_flow semantics) ─────
def _compute_model(ds: Dataset, window_days: int = 90) -> dict[str, Any]:
    c = ds.conn.cursor()
    as_of = AS_OF
    window_start = as_of - _dt.timedelta(days=window_days)

    invoices = [dict(r) for r in c.execute("SELECT * FROM invoices").fetchall()]
    payments = [dict(r) for r in c.execute("SELECT * FROM payments").fetchall()]
    charges = [dict(r) for r in c.execute("SELECT * FROM stripe_charges").fetchall()]
    fin_customers = {r["id"]: r["name"] for r in c.execute("SELECT id, name FROM finance_customers").fetchall()}

    in_window = lambda iso: _d(iso) >= window_start if iso else False

    # ── invoiced / paid stages (in-window by issue date) ──
    win_inv = [i for i in invoices if in_window(i["issued_at"])]
    invoiced_cents = sum(i["total_cents"] for i in win_inv)
    paid_cents = sum(i["total_cents"] for i in win_inv if i["status"] == "paid")

    # ── receivables aging (open invoices, by days past due vs as_of) ──
    bucket_defs = [("0-30", 0, 30), ("31-60", 31, 60), ("61-90", 61, 90), ("90+", 91, None)]
    aging = []
    for label, lo, hi in bucket_defs:
        rows = []
        for i in invoices:
            if i["status"] != "open" or not i["due_at"]:
                continue
            past = (as_of - _d(i["due_at"])).days
            if past < lo:
                continue
            if hi is not None and past > hi:
                continue
            rows.append((i["id"], fin_customers.get(i["customer_id"], i["customer_id"]), i["total_cents"], past))
        rows.sort(key=lambda r: r[2], reverse=True)
        total = sum(r[2] for r in rows)
        aging.append({
            "bucket": label, "min_days": lo, "max_days": hi,
            "count": len(rows), "total_cents": total,
            "invoices": [
                {"id": iid, "customer": cust, "amount": _dollars(cents), "days_past_due": past}
                for iid, cust, cents, past in rows[:_EVIDENCE_CAP]
            ],
        })

    # ── collection cycle: issue→paid days, current vs prior period, trend ──
    pay_by_invoice = {p["invoice_id"]: p for p in payments}
    cur_cycles, prior_cycles = [], []
    for i in invoices:
        if i["status"] != "paid":
            continue
        p = pay_by_invoice.get(i["id"])
        if not p or not p["received_at"] or not i["issued_at"]:
            continue
        cycle = (_d(p["received_at"]) - _d(i["issued_at"])).days
        (cur_cycles if in_window(i["issued_at"]) else prior_cycles).append(cycle)

    def _median(xs: list[int]):
        if not xs:
            return None
        s = sorted(xs)
        n = len(s)
        return float(s[n // 2]) if n % 2 else (s[n // 2 - 1] + s[n // 2]) / 2.0

    cur_med = _median(cur_cycles)
    prior_med = _median(prior_cycles)
    trend = "insufficient_data"
    if cur_med is not None and prior_med is not None:
        if cur_med > prior_med * 1.15:
            trend = "worsening"
        elif cur_med < prior_med * 0.85:
            trend = "improving"
        else:
            trend = "steady"

    # ── customer concentration (invoiced share in window) ──
    per_cust: dict[str, int] = {}
    for i in win_inv:
        name = fin_customers.get(i["customer_id"], i["customer_id"])
        per_cust[name] = per_cust.get(name, 0) + i["total_cents"]
    total_inv = sum(per_cust.values()) or 1
    ranked = sorted(per_cust.items(), key=lambda kv: kv[1], reverse=True)
    concentration = {
        "customers_seen": len(per_cust),
        "top": [
            {"customer": name, "invoiced": _dollars(cents), "share_pct": round(100 * cents / total_inv, 1)}
            for name, cents in ranked[:3]
        ],
    }

    # ── charge revenue (cross-source, dedup Stripe charge vs its QBO invoice) ──
    paid_invoice_ids = {i["id"] for i in invoices if i["status"] == "paid"}
    win_charges = [ch for ch in charges if in_window(ch["received_at"])]
    deduped = [ch for ch in win_charges if ch["reconcile_ref"] in paid_invoice_ids]
    counted = [ch for ch in win_charges if ch["reconcile_ref"] not in paid_invoice_ids]
    charge_net = sum(ch["amount_cents"] for ch in counted)
    charge_revenue = {
        "charge_count": len(counted),
        "net_cents": charge_net,
        "by_provider": [{"provider": "stripe_demo", "net": _dollars(charge_net), "charge_count": len(counted)}],
        "deduped_count": len(deduped),
        "deduped": _dollars(sum(ch["amount_cents"] for ch in deduped)),
        "dedup_confidence": "high" if deduped else "none",
    }

    return {
        "as_of": as_of.isoformat(),
        "window_days": window_days,
        "currency": "USD",
        "stages": {"invoiced": _dollars(invoiced_cents), "paid": _dollars(paid_cents),
                   "conversion_pct": round(100 * paid_cents / (invoiced_cents or 1), 1)},
        "cycle": {"median_days": cur_med, "prior_median_days": prior_med, "trend": trend},
        "aging": aging,
        "concentration": concentration,
        "charge_revenue": charge_revenue,
    }


def _money_flow_summary(m: dict[str, Any]) -> str:
    cr = m["charge_revenue"]
    parts = [
        f"Over the last {m['window_days']} days you took in {cr['by_provider'][0]['net']} in charge "
        f"revenue across {cr['charge_count']} charge(s) via stripe_demo."
    ]
    if cr["deduped_count"]:
        parts.append(
            f"{cr['deduped_count']} Stripe charge(s) totalling {cr['deduped']} were reconciled against "
            f"paid invoices (high-confidence) and excluded so revenue isn't double-counted."
        )
    parts.append(
        f"On the invoice side you invoiced {m['stages']['invoiced']} in this window and collected "
        f"{m['stages']['paid']} ({m['stages']['conversion_pct']}% conversion)."
    )
    cyc = m["cycle"]
    if cyc["median_days"] is not None and cyc["prior_median_days"] is not None:
        parts.append(
            f"Collection cycle is {cyc['median_days']:.0f} days (median) vs {cyc['prior_median_days']:.0f} "
            f"days in the prior period — trend is {cyc['trend']}."
        )
    top = m["concentration"]["top"]
    if top:
        parts.append(
            f"{top[0]['customer']} is your largest account at {top[0]['share_pct']}% of invoiced revenue "
            f"({m['concentration']['customers_seen']} customers seen)."
        )
    b90 = next((b for b in m["aging"] if b["bucket"] == "90+"), None)
    if b90 and b90["total_cents"]:
        parts.append(f"{b90['count']} invoice(s) totalling {_dollars(b90['total_cents'])} are 90+ days past due.")
    return " ".join(parts)


def _top_leaks(m: dict[str, Any]) -> list[dict[str, Any]]:
    candidates = []
    b90 = next((b for b in m["aging"] if b["bucket"] == "90+"), None)
    if b90 and b90["total_cents"] > 0:
        candidates.append({
            "id": "aged_receivables_90",
            "statement": f"{b90['count']} invoice(s) totalling {_dollars(b90['total_cents'])} are 90+ days past due.",
            "dollar_impact_cents": b90["total_cents"],
            "dollar_impact": _dollars(b90["total_cents"]),
            "evidence": b90["invoices"],
            "suggested_action": "send_reminder on the oldest, largest balances first",
        })
    mid_total = sum(b["total_cents"] for b in m["aging"] if b["bucket"] in ("31-60", "61-90"))
    mid_count = sum(b["count"] for b in m["aging"] if b["bucket"] in ("31-60", "61-90"))
    if mid_total > 0:
        candidates.append({
            "id": "aged_receivables_31_90",
            "statement": f"{mid_count} invoice(s) totalling {_dollars(mid_total)} are 31–90 days past due.",
            "dollar_impact_cents": mid_total,
            "dollar_impact": _dollars(mid_total),
            "suggested_action": "send_reminder before these roll into the 90+ bucket",
        })
    top = m["concentration"]["top"]
    if top and top[0]["share_pct"] >= 40.0:
        # impact ~ the concentrated customer's invoiced dollars
        conc_cents = int(round(float(top[0]["invoiced"].replace("$", "").replace(",", "").replace(" USD", "")) * 100))
        candidates.append({
            "id": "customer_concentration",
            "statement": f"{top[0]['customer']} is {top[0]['share_pct']}% of invoiced revenue — a concentration risk.",
            "dollar_impact_cents": conc_cents,
            "dollar_impact": top[0]["invoiced"],
            "suggested_action": "diversify the customer base to reduce single-account dependence",
        })
    if m["cycle"]["trend"] == "worsening":
        candidates.append({
            "id": "collection_slowdown",
            "statement": (
                f"Collection cycle worsened: {m['cycle']['median_days']:.0f}d now vs "
                f"{m['cycle']['prior_median_days']:.0f}d prior."
            ),
            "dollar_impact_cents": 0,
            "dollar_impact": "trend",
            "suggested_action": "tighten collections cadence on newly-issued invoices",
        })
    candidates.sort(key=lambda x: x["dollar_impact_cents"], reverse=True)
    return candidates[:3]


# ── the curated verb surface exposed to Arm A's Claude ───────────────────────
def money_flow(ds: Dataset, window_days: int = 90) -> dict[str, Any]:
    m = _compute_model(ds, window_days)
    return {"providers": ["qbo_demo", "stripe_demo"], "summary": _money_flow_summary(m), "model": m}


def finance_leaks(ds: Dataset, window_days: int = 90) -> dict[str, Any]:
    m = _compute_model(ds, window_days)
    leaks = _top_leaks(m)
    summary = "Top money leaks right now: " + " ".join(f"{i + 1}. {l['statement']}" for i, l in enumerate(leaks))
    return {"summary": summary, "leaks": leaks,
            "based_on": {"as_of": m["as_of"], "window_days": m["window_days"], "currency": "USD"}}


_COLLECTIONS = {"customers": "crm_customers", "deals": "deals"}


def get_catalog(ds: Dataset) -> dict[str, Any]:
    c = ds.conn.cursor()
    out = []
    for coll, tbl in _COLLECTIONS.items():
        cols = [r[1] for r in c.execute(f"PRAGMA table_info({tbl})").fetchall()]
        count = c.execute(f"SELECT COUNT(*) FROM {tbl}").fetchone()[0]
        out.append({"collection": coll, "fields": cols, "record_count": count})
    return {"collections": out}


def list_records(ds: Dataset, collection: str, filter: dict[str, Any] | None = None) -> dict[str, Any]:
    tbl = _COLLECTIONS.get(collection)
    if not tbl:
        return {"error": f"unknown collection '{collection}'. Known: {list(_COLLECTIONS)}"}
    c = ds.conn.cursor()
    sql = f"SELECT * FROM {tbl}"
    params: list[Any] = []
    if filter:
        clauses = []
        for k, v in filter.items():
            clauses.append(f"{k} = ?")
            params.append(v)
        sql += " WHERE " + " AND ".join(clauses)
    rows = [dict(r) for r in c.execute(sql, params).fetchall()]
    return {"collection": collection, "count": len(rows), "records": rows}


def search_documents(ds: Dataset, query: str, limit: int = 3) -> dict[str, Any]:
    """Hybrid keyword search returning grounded snippets (mirrors the doc store)."""
    c = ds.conn.cursor()
    docs = [dict(r) for r in c.execute("SELECT * FROM documents").fetchall()]
    terms = [t for t in query.lower().split() if len(t) > 2]
    scored = []
    for doc in docs:
        hay = (doc["title"] + "\n" + doc["body"]).lower()
        score = sum(hay.count(t) for t in terms)
        if score:
            scored.append((score, doc))
    scored.sort(key=lambda s: s[0], reverse=True)
    results = []
    for score, doc in scored[:limit]:
        # return the whole short doc body — these are small demo docs, grounded
        results.append({"filename": doc["filename"], "title": doc["title"], "content": doc["body"]})
    return {"query": query, "results": results}
