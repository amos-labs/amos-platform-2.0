"""Entry point: run the token-efficiency benchmark sweep and aggregate results.

    python benchmark.py --runs 3 [--region us-east-1] [--tasks cashflow,leaks]

Runs N trials per (arm × task), reads exact token usage from every response in
each tool-use loop, and reports per-task mean±spread, the A/B ratio, and a
dollar translation at Opus 4.8 rates ($5/$25 per 1M in/out).
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import sys
from pathlib import Path

from anthropic import AnthropicBedrockMantle

from runner import TrialResult, run_trial
from tasks import TASKS

# Opus 4.8 rates, per the claude-api model table.
RATE_IN_PER_TOK = 5.0 / 1_000_000
RATE_OUT_PER_TOK = 25.0 / 1_000_000
ARMS = ["amos", "raw"]


def dollars(r: TrialResult) -> float:
    u = r.usage
    # cache reads bill ~0.1x, cache writes ~1.25x; here caching is effectively
    # off (fresh per-trial prompt), so treat cache tokens as input-priced — a
    # conservative, uniform rule applied identically to both arms.
    inp = u["input_tokens"] + u["cache_creation_input_tokens"] + u["cache_read_input_tokens"]
    return inp * RATE_IN_PER_TOK + u["output_tokens"] * RATE_OUT_PER_TOK


def summarize(results: list[TrialResult]) -> dict:
    by = {}
    for arm in ARMS:
        for task in TASKS:
            passed = [r for r in results if r.arm == arm and r.task_id == task.id and r.passed]
            allr = [r for r in results if r.arm == arm and r.task_id == task.id]
            toks = [r.total_tokens for r in passed]
            usd = [dollars(r) for r in passed]
            by[(arm, task.id)] = {
                "n_total": len(allr),
                "n_passed": len(passed),
                "mean_tokens": round(statistics.mean(toks), 1) if toks else None,
                "stdev_tokens": round(statistics.stdev(toks), 1) if len(toks) > 1 else 0.0,
                "min_tokens": min(toks) if toks else None,
                "max_tokens": max(toks) if toks else None,
                "mean_usd": round(statistics.mean(usd), 6) if usd else None,
            }

    tasks_out = {}
    for task in TASKS:
        a = by[("amos", task.id)]
        b = by[("raw", task.id)]
        ratio = (round(b["mean_tokens"] / a["mean_tokens"], 2)
                 if a["mean_tokens"] and b["mean_tokens"] else None)
        tasks_out[task.id] = {"amos": a, "raw": b, "raw_over_amos_ratio": ratio}

    # aggregate over tasks where both arms have a passing mean
    a_tot = [by[("amos", t.id)]["mean_tokens"] for t in TASKS if by[("amos", t.id)]["mean_tokens"]]
    b_tot = [by[("raw", t.id)]["mean_tokens"] for t in TASKS
             if by[("raw", t.id)]["mean_tokens"] and by[("amos", t.id)]["mean_tokens"]]
    overall = None
    if a_tot and b_tot and len(a_tot) == len(b_tot):
        overall = round(statistics.mean(b_tot) / statistics.mean(a_tot), 2)
    return {"per_task": tasks_out, "overall_raw_over_amos_ratio": overall}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=3, help="trials per (arm × task), N>=3 recommended")
    ap.add_argument("--region", default=os.environ.get("AWS_REGION", "us-east-1"))
    ap.add_argument("--tasks", default="", help="comma-separated task ids (default: all)")
    ap.add_argument("--out", default=str(Path(__file__).parent / "results" / "results.json"))
    args = ap.parse_args()

    tasks = TASKS if not args.tasks else [t for t in TASKS if t.id in args.tasks.split(",")]
    client = AnthropicBedrockMantle(aws_region=args.region)

    results: list[TrialResult] = []
    for task in tasks:
        for arm in ARMS:
            for run in range(1, args.runs + 1):
                r = run_trial(client, arm, task, run)
                flag = "ok " if r.passed else "FAIL"
                print(f"[{flag}] {arm:5s} {task.id:12s} run {run}: "
                      f"{r.total_tokens:6d} tok, {r.iterations} iters"
                      f"{'  (' + r.grade_reason + ')' if not r.passed else ''}",
                      file=sys.stderr, flush=True)
                results.append(r)

    summary = summarize(results)
    out = {
        "model": "anthropic.claude-opus-4-8",
        "runs_per_cell": args.runs,
        "rates": {"input_per_mtok": 5.0, "output_per_mtok": 25.0},
        "summary": summary,
        "trials": [vars(r) for r in results],
    }
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(out, indent=2))
    print("\n=== SUMMARY ===", file=sys.stderr)
    print(json.dumps(summary, indent=2), file=sys.stderr)
    print(f"\nwrote {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
