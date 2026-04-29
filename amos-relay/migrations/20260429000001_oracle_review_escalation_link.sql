-- Stop Oracle from re-reviewing the same escalated bounty every tick.
--
-- When Oracle's review path returns `escalate`, the bounty's status doesn't
-- change (still submitted+verified), so /pending-review keeps returning it
-- and Oracle re-reviews on every 60s tick — costly Bedrock calls + duplicate
-- decisions in the corpus.
--
-- Fix: link bounty ↔ active review escalation. /pending-review filters out
-- bounties with an active link. Council resolution clears the link.

ALTER TABLE relay_bounties
    ADD COLUMN IF NOT EXISTS oracle_review_escalation_id UUID
        REFERENCES oracle_escalations(escalation_id);

ALTER TABLE oracle_escalations
    ADD COLUMN IF NOT EXISTS bounty_id UUID REFERENCES relay_bounties(id);

CREATE INDEX IF NOT EXISTS idx_relay_bounties_oracle_review_escalation
    ON relay_bounties (oracle_review_escalation_id)
    WHERE oracle_review_escalation_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_oracle_escalations_bounty
    ON oracle_escalations (bounty_id)
    WHERE bounty_id IS NOT NULL;
