"""Core benchmark runner: one Claude tool-use loop per (arm × task) trial.

Design invariants (so a critic can't call it rigged):
  * ONE model held constant across both arms: anthropic.claude-opus-4-8 on AWS
    Bedrock (the same LLM backend AMOS uses), via the official Anthropic SDK's
    AnthropicBedrockMantle client.
  * Token usage is read straight from each response's `usage` (input, output,
    and cache tokens) and SUMMED across the ENTIRE tool-use loop for a task —
    not just the final message. This is exact provider accounting, not a
    character-count estimate.
  * Both arms use the same system-prompt scaffold, the same model, the same
    thinking/effort settings, the same max_tokens, and the same underlying
    dataset. The ONLY thing that differs is the tool surface each is given.
"""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field
from typing import Any, Callable

from anthropic import AnthropicBedrockMantle

from arms import amos_verbs, raw_sql
from dataset import Dataset, build_dataset
from tasks import Task, grade

MODEL = "anthropic.claude-opus-4-8"
MAX_TOKENS = 8000
MAX_ITERATIONS = 30

# ── tool schemas per arm ─────────────────────────────────────────────────────
AMOS_TOOLS = [
    {"name": "get_catalog", "description": "List the tenant's custom data collections and their fields. Start here to see what business data exists.",
     "input_schema": {"type": "object", "properties": {}}},
    {"name": "list_records", "description": "List records in a collection (e.g. 'customers' or 'deals'), optionally filtered by exact field match.",
     "input_schema": {"type": "object", "properties": {"collection": {"type": "string"}, "filter": {"type": "object"}}, "required": ["collection"]}},
    {"name": "money_flow", "description": "How the business's money actually flows over a window: charge revenue (cross-source, deduped), invoiced vs collected, collection cycle time vs the prior period, receivables aging, and customer concentration. Returns a plain-language summary plus the full model. Computed over stored finance data.",
     "input_schema": {"type": "object", "properties": {"window_days": {"type": "integer"}}}},
    {"name": "finance_leaks", "description": "The top issues costing money right now, each with a dollar impact, the specific invoices/customers behind it, and a suggested action.",
     "input_schema": {"type": "object", "properties": {"window_days": {"type": "integer"}}}},
    {"name": "search_documents", "description": "Hybrid search over the company's internal documents; returns grounded snippets with their source filenames.",
     "input_schema": {"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "integer"}}, "required": ["query"]}},
]

RAW_TOOLS = [
    {"name": "list_tables", "description": "List all tables in the business database.",
     "input_schema": {"type": "object", "properties": {}}},
    {"name": "describe_table", "description": "Show a table's columns (name + type) and two sample rows.",
     "input_schema": {"type": "object", "properties": {"table": {"type": "string"}}, "required": ["table"]}},
    {"name": "run_sql", "description": "Run a single read-only SQL SELECT query against the business database and return the raw rows (capped at 1000).",
     "input_schema": {"type": "object", "properties": {"sql": {"type": "string"}, "max_rows": {"type": "integer"}}, "required": ["sql"]}},
]

SYSTEM_AMOS = (
    "You operate a business (Northwind Labs) on AMOS, a governed control plane. "
    "You have curated tools that return pre-digested business answers. Prefer the most "
    "specific tool for the question. Answer the user's question directly and completely, "
    "citing concrete numbers and names from the tool results. Be concise."
)
SYSTEM_RAW = (
    "You are a business data analyst with direct SQL access to the company's database "
    "(Northwind Labs). Explore the schema, query the data, and reason from the raw rows to "
    "answer the user's question directly and completely, citing concrete numbers and names. "
    "Be concise."
)


@dataclass
class Usage:
    input_tokens: int = 0
    output_tokens: int = 0
    cache_creation_input_tokens: int = 0
    cache_read_input_tokens: int = 0

    def add(self, u: Any) -> None:
        self.input_tokens += getattr(u, "input_tokens", 0) or 0
        self.output_tokens += getattr(u, "output_tokens", 0) or 0
        self.cache_creation_input_tokens += getattr(u, "cache_creation_input_tokens", 0) or 0
        self.cache_read_input_tokens += getattr(u, "cache_read_input_tokens", 0) or 0

    @property
    def total(self) -> int:
        return (self.input_tokens + self.output_tokens
                + self.cache_creation_input_tokens + self.cache_read_input_tokens)


@dataclass
class TrialResult:
    arm: str
    task_id: str
    run: int
    passed: bool
    grade_reason: str
    iterations: int
    usage: dict[str, int]
    total_tokens: int
    answer: str = ""
    error: str = ""


def _dispatch(arm: str, ds: Dataset, name: str, args: dict[str, Any]) -> Any:
    if arm == "amos":
        fns: dict[str, Callable] = {
            "get_catalog": lambda: amos_verbs.get_catalog(ds),
            "list_records": lambda: amos_verbs.list_records(ds, args.get("collection", ""), args.get("filter")),
            "money_flow": lambda: amos_verbs.money_flow(ds, args.get("window_days", 90)),
            "finance_leaks": lambda: amos_verbs.finance_leaks(ds, args.get("window_days", 90)),
            "search_documents": lambda: amos_verbs.search_documents(ds, args.get("query", ""), args.get("limit", 3)),
        }
    else:
        fns = {
            "list_tables": lambda: raw_sql.list_tables(ds),
            "describe_table": lambda: raw_sql.describe_table(ds, args.get("table", "")),
            "run_sql": lambda: raw_sql.run_sql(ds, args.get("sql", ""), args.get("max_rows", 1000)),
        }
    fn = fns.get(name)
    if not fn:
        return {"error": f"unknown tool '{name}'"}
    return fn()


def run_trial(client: AnthropicBedrockMantle, arm: str, task: Task, run: int) -> TrialResult:
    ds = build_dataset()  # fresh identical dataset per trial
    tools = AMOS_TOOLS if arm == "amos" else RAW_TOOLS
    system = SYSTEM_AMOS if arm == "amos" else SYSTEM_RAW
    messages: list[dict[str, Any]] = [{"role": "user", "content": task.prompt}]
    usage = Usage()

    try:
        for iteration in range(1, MAX_ITERATIONS + 1):
            # Stream + get_final_message per the SDK guidance for long tool loops.
            with client.messages.stream(
                model=MODEL,
                max_tokens=MAX_TOKENS,
                thinking={"type": "adaptive"},
                system=system,
                tools=tools,
                messages=messages,
            ) as stream:
                resp = stream.get_final_message()
            usage.add(resp.usage)

            if resp.stop_reason != "tool_use":
                answer = "".join(b.text for b in resp.content if b.type == "text")
                passed, reason = grade(task, answer)
                return TrialResult(arm, task.id, run, passed, reason, iteration,
                                   asdict(usage), usage.total, answer)

            messages.append({"role": "assistant", "content": resp.content})
            results = []
            for block in resp.content:
                if block.type == "tool_use":
                    out = _dispatch(arm, ds, block.name, dict(block.input))
                    results.append({"type": "tool_result", "tool_use_id": block.id,
                                    "content": json.dumps(out, default=str)})
            messages.append({"role": "user", "content": results})

        return TrialResult(arm, task.id, run, False, "max_iterations", MAX_ITERATIONS,
                           asdict(usage), usage.total, error="hit MAX_ITERATIONS")
    except Exception as e:  # capture provider/SDK errors without aborting the sweep
        return TrialResult(arm, task.id, run, False, "error", 0, asdict(usage),
                           usage.total, error=f"{type(e).__name__}: {e}")
