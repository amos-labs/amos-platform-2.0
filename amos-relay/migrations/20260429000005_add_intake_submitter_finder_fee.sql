-- Bug-report rewards: a finder's fee for the intake submitter when their
-- report leads to a commissioned bounty that ultimately settles. Pays from
-- the bounty's reward pool (split, not additional) — typically 5% to
-- submitter, 95% to worker.
--
-- Anti-spam: payment is gated on Oracle commissioning AND the bounty actually
-- settling. Junk reports cost an Oracle inference; Oracle's intake filter
-- rejects/escalates them. Real reports that turn into real fixes pay out.
--
-- V1 records the obligation; on-chain disbursement (separate tx via SPL
-- transfer) is queued as 'pending' for an admin-runnable batch until the
-- bounty program supports a third payout recipient natively.

ALTER TABLE oracle_intakes
    ADD COLUMN IF NOT EXISTS submitter_wallet TEXT;

COMMENT ON COLUMN oracle_intakes.submitter_wallet IS
    'Optional Solana wallet — when set, submitter is eligible for finder''s '
    'fee on commissioned + settled bounties derived from this intake.';

ALTER TABLE relay_bounties
    ADD COLUMN IF NOT EXISTS intake_submitter_wallet TEXT,
    ADD COLUMN IF NOT EXISTS intake_submitter_payout_bps SMALLINT,
    ADD COLUMN IF NOT EXISTS intake_submitter_payout_status TEXT,
    ADD COLUMN IF NOT EXISTS intake_submitter_payout_tx TEXT,
    ADD COLUMN IF NOT EXISTS intake_submitter_payout_at TIMESTAMPTZ;

COMMENT ON COLUMN relay_bounties.intake_submitter_wallet IS
    'Copied from oracle_intakes.submitter_wallet at Oracle-commission time. '
    'NULL if the intake had no submitter wallet (anonymous report) or the '
    'bounty was not commissioned via intake.';
COMMENT ON COLUMN relay_bounties.intake_submitter_payout_bps IS
    'Basis points of reward_tokens routed to intake submitter. Default 500 '
    '(5%) when wallet present; NULL when not. Worker payout = '
    'reward_tokens * (10000 - bps) / 10000.';
COMMENT ON COLUMN relay_bounties.intake_submitter_payout_status IS
    'NULL until bounty settles; then ''pending'' until disbursement, '
    '''paid'' after, ''failed'' on error.';

-- Pending-payout query lane for the disbursement worker.
CREATE INDEX IF NOT EXISTS idx_bounties_finder_fee_pending
    ON relay_bounties (approved_at)
    WHERE intake_submitter_wallet IS NOT NULL
      AND intake_submitter_payout_status = 'pending';
