-- Auto-merge tracking columns for OPS-AUTOMERGE-001.
--
-- Closes the META-007 phase 6 gap: "approved + settled" is contract-level
-- truth, but until the worker's PR actually lands on main the integration
-- isn't real (see feedback_settled_neq_merged.md — the prior pre-META-007
-- incident where 3 bounties were settled but their fixes were never on main).
--
-- The auto-merge bot watches relay for bounties with
--   status='approved' AND settlement_status='settled' AND merge_commit_sha IS NULL
-- For each: verifies PR head SHA matches the proof_receipt's pinned SHA,
-- verifies CI is green, runs `gh pr merge --squash --delete-branch`, then
-- POSTs back to /api/v1/bounties/{id}/record-merge with the merge SHA.
--
-- This makes "settled = code-on-main" a structural invariant going forward,
-- rather than a manual cross-check the founder has to remember.

ALTER TABLE relay_bounties
    ADD COLUMN merge_commit_sha TEXT,
    ADD COLUMN merged_at TIMESTAMPTZ,
    ADD COLUMN merged_by TEXT;

-- Partial index for the auto-merge bot's poll query. Most bounty rows are
-- not settled-and-unmerged, so the partial index is much smaller than a
-- full one and the bot's query stays fast as the table grows.
CREATE INDEX idx_bounties_settled_unmerged
    ON relay_bounties (approved_at)
    WHERE status = 'approved'
      AND settlement_status = 'settled'
      AND merge_commit_sha IS NULL;
