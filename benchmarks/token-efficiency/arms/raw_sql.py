"""Arm B surface: generic raw-access tools over the SAME shared dataset.

This is the honest steel-man of how a generic AI data-analyst agent operates
today: no curated verbs, no pre-digested company brain. It gets:

  * list_tables     — discover what tables exist
  * describe_table  — see a table's columns + a couple of sample rows
  * run_sql         — run a read-only SQL query and get raw rows back

Claude must explore the schema, paginate raw rows into its context, and do all
the business reasoning (aging, cycle-time trend, concentration, cross-source
dedup, stale-deal selection) from scratch. Same underlying SQLite data as Arm A
— the ONLY difference is the surface.
"""

from __future__ import annotations

import re
from typing import Any

from dataset import Dataset

_ROW_CAP = 1000

# Only allow read-only single statements — a raw analyst tool wouldn't let the
# model mutate the books. This is a safety rail, not a token-cost lever.
_FORBIDDEN = re.compile(r"\b(insert|update|delete|drop|alter|create|replace|attach|pragma)\b", re.I)


def list_tables(ds: Dataset) -> dict[str, Any]:
    c = ds.conn.cursor()
    rows = c.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").fetchall()
    return {"tables": [r[0] for r in rows]}


def describe_table(ds: Dataset, table: str) -> dict[str, Any]:
    c = ds.conn.cursor()
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", table):
        return {"error": "invalid table name"}
    cols = c.execute(f"PRAGMA table_info({table})").fetchall()
    if not cols:
        return {"error": f"no such table '{table}'"}
    sample = [dict(r) for r in c.execute(f"SELECT * FROM {table} LIMIT 2").fetchall()]
    return {
        "table": table,
        "columns": [{"name": r[1], "type": r[2]} for r in cols],
        "sample_rows": sample,
    }


def run_sql(ds: Dataset, sql: str, max_rows: int = _ROW_CAP) -> dict[str, Any]:
    if _FORBIDDEN.search(sql):
        return {"error": "only read-only SELECT queries are allowed"}
    if ";" in sql.strip().rstrip(";"):
        return {"error": "only a single statement is allowed"}
    c = ds.conn.cursor()
    try:
        rows = [dict(r) for r in c.execute(sql).fetchmany(min(max_rows, _ROW_CAP))]
    except Exception as e:  # surface SQL errors so the model can adapt
        return {"error": f"SQL error: {e}"}
    return {"row_count": len(rows), "rows": rows}
