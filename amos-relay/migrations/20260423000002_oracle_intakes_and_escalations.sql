-- Oracle-facing tables: intakes (submissions the Oracle evaluates before
-- commissioning as bounties) and escalations (decisions the Oracle routed
-- to council instead of self-authorizing).
--
-- Together with oracle_decisions + oracle_outcomes (migration
-- 20260423000001), these complete the set of tables the autonomous Oracle
-- daemon needs to operate.

-- ────────────────────────────────────────────────────────────────────────
-- Intake submissions
-- ────────────────────────────────────────────────────────────────────────
-- A submission is a request that MAY become a bounty. The Oracle evaluates
-- each pending submission and produces a verdict (commission/reject/
-- refine/escalate). Commission creates a new relay_bounty; refine bounces
-- back with structured feedback; reject or escalate terminate the flow.
CREATE TABLE IF NOT EXISTS oracle_intakes (
    submission_id           UUID PRIMARY KEY,
    title                   TEXT NOT NULL,
    body                    TEXT NOT NULL,
    submitter               TEXT NOT NULL,
    parent_submission_id    UUID REFERENCES oracle_intakes(submission_id),
    suggested_category      TEXT,
    suggested_capabilities  JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Lifecycle: pending → evaluated. Terminal sub-states captured in
    -- `verdict` once evaluated.
    status                  TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'evaluated')),
    verdict                 TEXT
        CHECK (verdict IS NULL OR verdict IN (
            'commission', 'reject', 'refine', 'escalate'
        )),

    -- Joined-up references populated at evaluation time
    decision_id             UUID REFERENCES oracle_decisions(decision_id),
    commissioned_bounty_id  UUID REFERENCES relay_bounties(id),

    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    evaluated_at            TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_oracle_intakes_status_created
    ON oracle_intakes (status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_oracle_intakes_submitter
    ON oracle_intakes (submitter);

CREATE INDEX IF NOT EXISTS idx_oracle_intakes_parent
    ON oracle_intakes (parent_submission_id)
    WHERE parent_submission_id IS NOT NULL;

-- ────────────────────────────────────────────────────────────────────────
-- Council escalation queue
-- ────────────────────────────────────────────────────────────────────────
-- When the Oracle escalates (low confidence, above ceiling, novel territory,
-- reasoning-substrate touching, etc.) the decision lands here for council.
-- Council resolves with a verdict + reasoning; the resolution joins back to
-- oracle_outcomes as a `council_override` entry.
CREATE TABLE IF NOT EXISTS oracle_escalations (
    escalation_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    decision_id       UUID NOT NULL REFERENCES oracle_decisions(decision_id),
    path              TEXT NOT NULL CHECK (path IN ('intake', 'review')),
    reason            TEXT NOT NULL,

    status            TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'resolved')),
    council_verdict   TEXT,
    council_reasoning TEXT,
    resolved_by       TEXT,
    resolved_at       TIMESTAMPTZ,

    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_oracle_escalations_status_created
    ON oracle_escalations (status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_oracle_escalations_decision
    ON oracle_escalations (decision_id);
