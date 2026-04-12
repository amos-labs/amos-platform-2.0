-- Rename protocol fee ledger columns to match AMOS-only 50/40/10 split
-- Old: treasury_share (was 20% to treasury), ops_burn_share (was 10% burn+ops)
-- New: burn_share (40% burned), labs_share (10% to AMOS Labs)

ALTER TABLE protocol_fee_ledger RENAME COLUMN treasury_share TO burn_share;
ALTER TABLE protocol_fee_ledger RENAME COLUMN ops_burn_share TO labs_share;
