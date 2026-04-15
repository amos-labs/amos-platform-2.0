-- Add retry count for failed on-chain settlements.
-- The background settlement retry task uses this to implement
-- exponential backoff and give up after MAX_RETRIES attempts.

ALTER TABLE relay_bounties
    ADD COLUMN IF NOT EXISTS settlement_retry_count INT DEFAULT 0;

-- Also add reviewer_wallet so settlement retries know who approved
ALTER TABLE relay_bounties
    ADD COLUMN IF NOT EXISTS reviewer_wallet VARCHAR(64);
