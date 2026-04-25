-- Oracle event log: durable record of every intake + review decision, plus
-- downstream outcomes (council overrides, settlement, claim lifecycle) joined
-- back via decision_id.
--
-- Schema supports:
--   1. Precedent retrieval — MVP queries by path + recency; pgvector upgrade
--      path left open by keeping `payload` as JSONB.
--   2. Drift monitoring — joined view of decisions + outcomes reveals
--      calibration (predicted confidence vs. actual council-match rate).
--
-- Decision shape matches amos-oracle::decision::Decision exactly. Full struct
-- is stored as `payload` JSONB; indexed columns pulled out for hot queries.

CREATE TABLE IF NOT EXISTS oracle_decisions (
    decision_id      UUID PRIMARY KEY,
    path             TEXT NOT NULL CHECK (path IN ('intake', 'review')),
    verdict          TEXT NOT NULL,
    confidence       DOUBLE PRECISION NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
    prompt_version   TEXT NOT NULL,
    model_version    TEXT NOT NULL,
    decided_at       TIMESTAMPTZ NOT NULL,
    payload          JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_oracle_decisions_path_decided_at
    ON oracle_decisions (path, decided_at DESC);

CREATE INDEX IF NOT EXISTS idx_oracle_decisions_decided_at
    ON oracle_decisions (decided_at DESC);

CREATE INDEX IF NOT EXISTS idx_oracle_decisions_verdict
    ON oracle_decisions (verdict);

-- GIN index enables JSONB containment + key lookup queries on the full payload
-- (used by precedent retrieval for category / capability matching).
CREATE INDEX IF NOT EXISTS idx_oracle_decisions_payload_gin
    ON oracle_decisions USING GIN (payload jsonb_path_ops);

-- Outcomes joined back to decisions. One decision can have multiple outcomes
-- over time (e.g. CommissionedBountyClaimed → CommissionedBountySettled).
CREATE TABLE IF NOT EXISTS oracle_outcomes (
    outcome_id    UUID PRIMARY KEY,
    decision_id   UUID NOT NULL REFERENCES oracle_decisions(decision_id) ON DELETE CASCADE,
    outcome_kind  TEXT NOT NULL,
    payload       JSONB NOT NULL,
    recorded_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_oracle_outcomes_decision_id
    ON oracle_outcomes (decision_id);

CREATE INDEX IF NOT EXISTS idx_oracle_outcomes_kind
    ON oracle_outcomes (outcome_kind);

CREATE INDEX IF NOT EXISTS idx_oracle_outcomes_recorded_at
    ON oracle_outcomes (recorded_at DESC);
