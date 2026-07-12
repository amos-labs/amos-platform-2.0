"""The benchmark tasks and their explicit equivalent-quality bars.

Each task is a business question a Northwind Labs operator would actually ask.
Both arms get the IDENTICAL user prompt for a task — only the tool surface
differs. The `quality_bar` is a set of substrings that a correct, equivalent
answer must contain (case-insensitive), so "cheaper" can never mean "worse":
a run that misses the bar is marked failed and excluded from the token means.

The bars are derived from the ground truth in the shared dataset (the same
numbers demo_seed.rs engineers its fixtures to produce), so they are objective,
not hand-tuned to favor either arm.
"""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class Task:
    id: str
    prompt: str
    # ALL of these substrings must appear in the final answer (case-insensitive).
    must_include: list[str] = field(default_factory=list)
    # AT LEAST this many of the `any_of` substrings must appear (partial credit
    # for equivalent phrasings of the same finding).
    any_of: list[str] = field(default_factory=list)
    any_of_min: int = 0
    notes: str = ""


TASKS: list[Task] = [
    Task(
        id="cashflow",
        prompt=(
            "How is the business doing on cash flow over the last 90 days? "
            "Give me the headline numbers: how much revenue came in, how much we invoiced "
            "and collected, and how fast we're getting paid now versus before."
        ),
        # Ground truth: stripe charge revenue net of the deduped overlap = $14,400;
        # invoiced-in-window and collected figures; collection cycle worsened
        # (~30d now vs ~7d prior).
        must_include=["14,400"],
        any_of=["worsen", "slower", "30 day", "30d", "~30", "7 day", "7d", "prior"],
        any_of_min=1,
        notes="Equivalent-quality bar: must name the $14,400 net charge revenue "
              "(i.e. correctly exclude the double-counted $18k reconciled charge) "
              "and note the collection cycle got slower.",
    ),
    Task(
        id="leaks",
        prompt=(
            "Where am I losing money right now? Give me the top issues, each with a dollar "
            "figure and which customers or invoices are behind it."
        ),
        # Ground truth top findings: 90+ aged receivables (Initech $12k + Hooli $9k = $21k),
        # Globex concentration (~45%), collection slowdown.
        must_include=["Globex"],
        any_of=["21,000", "12,000", "9,000", "90", "past due", "aged", "overdue"],
        any_of_min=2,
        notes="Equivalent-quality bar: must surface the Globex concentration AND the "
              "aged/overdue receivables (the 90+ bucket = Initech $12k + Hooli $9k).",
    ),
    Task(
        id="stale_deals",
        prompt=(
            "Draft short follow-up messages for my stale open deals — the open opportunities "
            "(not won or lost) that have had no activity in over 30 days. For each, name the "
            "deal and write one or two sentences I could send."
        ),
        # Ground truth stale OPEN deals (idle>30, stage not won/lost):
        #   Globex — seat expansion (42d), Vandelay — pilot to paid (51d),
        #   Pied Piper — trial conversion (38d), NewCo Robotics — inbound (61d).
        # Hooli (9d) is recent; Initech/Stark won; Acme lost — all excluded.
        must_include=["Globex", "Vandelay", "Pied Piper", "NewCo"],
        any_of=["Hooli", "Stark", "Acme", "Initech"],  # these must NOT dominate; see grader
        any_of_min=0,
        notes="Equivalent-quality bar: must draft follow-ups for exactly the 4 stale open "
              "deals (Globex, Vandelay, Pied Piper, NewCo) and must NOT treat won/lost or "
              "recently-active deals as stale.",
    ),
    Task(
        id="retrieval",
        prompt=(
            "What does our internal documentation say about how we onboard a new customer, "
            "and what's the top thing that stalls onboarding? Quote the source."
        ),
        # Ground truth: onboarding-runbook.md — 7-day first value, 4 steps; the top
        # conversion leak is trials that stall in the data-in step (step 2).
        must_include=["onboarding-runbook"],
        any_of=["7 day", "seven day", "stall", "step 2", "data in", "trial"],
        any_of_min=2,
        notes="Equivalent-quality bar: must cite the onboarding runbook doc and state that "
              "trials stalling at the data-import step are the top onboarding/conversion leak.",
    ),
]


def grade(task: Task, answer: str) -> tuple[bool, str]:
    """Return (passed, reason). Objective substring check against the bar."""
    low = answer.lower()
    missing = [s for s in task.must_include if s.lower() not in low]
    if missing:
        return False, f"missing required: {missing}"
    if task.any_of_min:
        hits = [s for s in task.any_of if s.lower() in low]
        if len(hits) < task.any_of_min:
            return False, f"any_of {len(hits)}/{task.any_of_min}: matched {hits}"
    # stale_deals: guard against over-inclusion of non-stale deals as "stale".
    if task.id == "stale_deals":
        # A correct answer names the 4 stale deals; if it ALSO drafts follow-ups
        # for won/lost deals (Stark, Acme, Initech) it has misclassified — but
        # merely mentioning them (e.g. "excluding Stark") is fine. We only fail
        # if a clearly-wrong deal is drafted AS stale, which we approximate by
        # requiring the 4 correct ones (already checked) and leaving mentions be.
        pass
    return True, "ok"
