"""Tests for AGENT_CONTEXT.md parser."""

import math
import sys
from pathlib import Path

# Add parent to path so we can import the parser
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from agent_context_parser import (
    AgentContext,
    compute_decay_rate,
    compute_emission,
    get_tools_for_trust_level,
    parse_agent_context,
)

# Path to the real AGENT_CONTEXT.md in the repo root
AGENT_CONTEXT_PATH = Path(__file__).resolve().parent.parent.parent.parent / "AGENT_CONTEXT.md"


def test_file_exists():
    assert AGENT_CONTEXT_PATH.exists(), f"AGENT_CONTEXT.md not found at {AGENT_CONTEXT_PATH}"


def test_parse_returns_agent_context():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert isinstance(ctx, AgentContext)


# ── Token Parameters ──────────────────────────────────────────────────────────


def test_token_supply():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.token.total_supply == 100_000_000


def test_token_allocation():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.token.bounty_treasury == 95_000_000
    assert ctx.token.emergency_reserve == 5_000_000
    assert ctx.token.bounty_treasury + ctx.token.emergency_reserve == ctx.token.total_supply


def test_token_blockchain():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.token.blockchain == "Solana"
    assert ctx.token.standard == "SPL"


def test_token_pricing():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.token.initial_price == 0.02
    assert ctx.token.initial_fdv == 2_000_000
    assert ctx.token.initial_dex == "Raydium"


# ── Revenue Distribution ──────────────────────────────────────────────────────


def test_protocol_fee():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.revenue.protocol_fee_pct == 0.03


def test_fee_split():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.revenue.staked_holders_pct == 0.50
    assert ctx.revenue.burned_pct == 0.40
    assert ctx.revenue.labs_pct == 0.10
    total = ctx.revenue.staked_holders_pct + ctx.revenue.burned_pct + ctx.revenue.labs_pct
    assert abs(total - 1.0) < 0.001


# ── Decay Mechanics ───────────────────────────────────────────────────────────


def test_decay_base_params():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.decay.min_rate == 0.02
    assert ctx.decay.max_rate == 0.25
    assert ctx.decay.default_rate == 0.05


def test_decay_grace_periods():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.decay.new_stake_grace_days == 365
    assert ctx.decay.inactivity_grace_days == 90
    assert ctx.decay.inactivity_threshold_days == 90


def test_decay_redistribution():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.decay.treasury_redistribution_pct == 0.90
    assert ctx.decay.burned_redistribution_pct == 0.10


def test_decay_minimum_preserved():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.decay.minimum_preserved_pct == 0.10


def test_compute_decay_rate():
    assert compute_decay_rate(0.0) == 0.10  # Base rate
    assert compute_decay_rate(1.0) == 0.05  # 10% - 5% = 5%
    assert compute_decay_rate(2.0) == 0.02  # Clamped to min
    assert compute_decay_rate(-3.0) == 0.25  # Clamped to max


def test_vaults():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert "bronze" in ctx.vaults
    assert "silver" in ctx.vaults
    assert "gold" in ctx.vaults
    assert "permanent" in ctx.vaults
    assert ctx.vaults["bronze"].lockup_days == 30
    assert ctx.vaults["bronze"].decay_reduction_pct == 0.20
    assert ctx.vaults["permanent"].lockup_days is None
    assert ctx.vaults["permanent"].decay_reduction_pct == 0.95


# ── Trust System ──────────────────────────────────────────────────────────────


def test_trust_max_level():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.trust.max_level == 5


def test_trust_levels_parsed():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert len(ctx.trust.levels) == 5
    levels_by_num = {l.level: l for l in ctx.trust.levels}
    assert levels_by_num[1].max_points == 100
    assert levels_by_num[1].daily_bounty_limit == 3
    assert levels_by_num[5].max_points == 2000
    assert levels_by_num[5].daily_bounty_limit == 25


def test_trust_upgrades():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert len(ctx.trust.upgrades) == 4  # 1→2, 2→3, 3→4, 4→5
    upgrade_1_2 = next(u for u in ctx.trust.upgrades if u.from_level == 1)
    assert upgrade_1_2.completions == 3
    assert upgrade_1_2.min_reputation_bps == 5500


# ── Emission ──────────────────────────────────────────────────────────────────


def test_emission_params():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.emission.ceiling == 16_000
    assert ctx.emission.floor == 100
    assert ctx.emission.midpoint_days == 1460
    assert ctx.emission.k_scaled == 50


def test_emission_curve():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    day_0 = compute_emission(ctx, 0)
    day_mid = compute_emission(ctx, 1460)
    day_late = compute_emission(ctx, 3650)

    # Day 0 should be near ceiling
    assert day_0 > 15_000
    # Midpoint should be roughly halfway
    assert abs(day_mid - 8050) < 100
    # Day 3650 (~10 years) should be approaching floor
    assert day_late < 500
    # Monotonically decreasing
    assert day_0 > day_mid > day_late


# ── Pool Separation ───────────────────────────────────────────────────────────


def test_pool_separation():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.pool_separation.ceiling_bps == 2000
    assert ctx.pool_separation.floor_bps == 300
    assert ctx.pool_separation.midpoint_days == 540
    assert len(ctx.pool_separation.technical_categories) > 0
    assert len(ctx.pool_separation.growth_categories) > 0
    assert "infrastructure" in ctx.pool_separation.technical_categories
    assert "signup" in ctx.pool_separation.growth_categories


# ── Bounty Parameters ────────────────────────────────────────────────────────


def test_bounty_quality():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.bounty.min_quality_score == 30
    assert ctx.bounty.max_bounty_points == 2000


def test_bounty_claim_timeouts():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert ctx.bounty.claim_timeout_default_hours == 72
    assert ctx.bounty.claim_timeout_min_hours == 1
    assert ctx.bounty.claim_timeout_max_hours == 720


def test_bounty_multipliers():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    m = ctx.bounty.contribution_multipliers
    assert m["infrastructure"] == 130
    assert m["feature"] == 100
    assert m["documentation"] == 80
    assert m["support"] == 70


def test_concurrent_claim_limits():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    limits = ctx.bounty.concurrent_claim_limits
    assert limits[1] == 3
    assert limits[5] == 20


# ── Tools ─────────────────────────────────────────────────────────────────────


def test_tools_parsed():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert len(ctx.tool_categories) > 0


def test_tools_trust_levels():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    trust_levels = {cat.trust_level for cat in ctx.tool_categories}
    assert 1 in trust_levels
    assert 2 in trust_levels
    assert 3 in trust_levels


def test_tools_system_category():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    system_cats = [c for c in ctx.tool_categories if c.name == "system"]
    assert len(system_cats) == 1
    assert "bash" in system_cats[0].tools
    assert "read_file" in system_cats[0].tools
    assert system_cats[0].trust_level == 1


def test_get_tools_for_trust_level():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    # Trust 1 should only get L1 tools
    t1 = get_tools_for_trust_level(ctx, 1)
    assert "system" in t1
    assert "schema" not in t1  # L2

    # Trust 3 should get L1, L2, and L3
    t3 = get_tools_for_trust_level(ctx, 3)
    assert "system" in t3
    assert "schema" in t3
    assert "openclaw" in t3
    # bounty_agent tools are in the openclaw category (same code block in md)
    assert "claim_bounty" in t3["openclaw"]


# ── Codebase References ──────────────────────────────────────────────────────


def test_codebase_refs():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert "token_economics" in ctx.codebase_refs
    assert "economics.rs" in ctx.codebase_refs["token_economics"]
    assert "agent_loop" in ctx.codebase_refs


# ── Raw Sections ──────────────────────────────────────────────────────────────


def test_raw_sections():
    ctx = parse_agent_context(AGENT_CONTEXT_PATH)
    assert len(ctx.raw_sections) > 0
    # Should have major section headers
    section_names = " ".join(ctx.raw_sections.keys()).lower()
    assert "token" in section_names
    assert "trust" in section_names
    assert "bounty" in section_names


# ── Error Handling ────────────────────────────────────────────────────────────


def test_invalid_file():
    import tempfile

    with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False) as f:
        f.write("# Not an AGENT_CONTEXT file\n\nJust some random markdown.")
        f.flush()
        try:
            parse_agent_context(f.name)
            assert False, "Should have raised ValueError"
        except ValueError:
            pass
        finally:
            Path(f.name).unlink()


def test_missing_file():
    try:
        parse_agent_context("/nonexistent/path/AGENT_CONTEXT.md")
        assert False, "Should have raised FileNotFoundError"
    except FileNotFoundError:
        pass


if __name__ == "__main__":
    # Simple test runner
    import traceback

    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    passed = 0
    failed = 0
    for test in tests:
        try:
            test()
            passed += 1
            print(f"  PASS  {test.__name__}")
        except Exception as e:
            failed += 1
            print(f"  FAIL  {test.__name__}: {e}")
            traceback.print_exc()

    print(f"\n{passed} passed, {failed} failed, {passed + failed} total")
    sys.exit(1 if failed else 0)
