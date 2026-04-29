-- One-time backfill so existing open code-class bounties flow through the
-- META-007 contract layer cleanly.
--
-- Code-class = category in ('infrastructure', 'research'). The phase 5 strict
-- gate at /approve applies to these categories. Bounties created before the
-- `policy` column existed have policy=NULL; their workers won't know about
-- the proof_receipt requirement either.
--
-- Two changes per row:
--   1. Set a sane default `policy` (forbids reasoning-substrate paths, scopes
--      via category constraint, self_modifying=false).
--   2. Append a note to `description` pointing at the proof-carrying spec +
--      example receipt so workers know to emit a receipt on submit.
--
-- Scope: status IN ('open','claimed') only — already-submitted bounties stay
-- as-is and flow via override_reason at approve time.
-- Idempotent: re-running won't double-append the description note.

-- ── Policy backfill ──────────────────────────────────────────────────────
UPDATE relay_bounties
SET policy = jsonb_build_object(
    'forbidden_paths', jsonb_build_array(
        'amos-oracle/prompts/**',
        'amos-oracle/src/agent.rs',
        'amos-oracle/src/intake.rs',
        'amos-oracle/src/review.rs'
    ),
    'required_paths_subset', jsonb_build_array(),
    'scope_constraint_ids', jsonb_build_array('category:' || category),
    'minimum_coverage_pct', null::int,
    'max_file_size_bytes', null::int,
    'self_modifying', false
)
WHERE category IN ('infrastructure', 'research')
  AND status IN ('open', 'claimed')
  AND policy IS NULL;

-- ── Description note backfill ────────────────────────────────────────────
UPDATE relay_bounties
SET description = description || E'\n\n---\n\n**Proof-carrying contract (AMOS-META-007):** Code bounty submissions should include a `proof_receipt` JSON field on `POST /api/v1/bounties/{id}/submit` describing intent, policy compliance, validation plan, execution evidence, and the GitHub PR + head SHA. See `docs/AMOS_PROOF_CARRYING_DEV_PIPELINE.md` for the canonical schema and `docs/EXAMPLE_PROOF_RECEIPT.json` for a working template. Approval requires the receipt or an explicit `override_reason` (≥40 chars). Submitting without a receipt is permitted (back-compat) but will block at the approval gate.'
WHERE category IN ('infrastructure', 'research')
  AND status IN ('open', 'claimed')
  AND description NOT LIKE '%Proof-carrying contract (AMOS-META-007)%';
