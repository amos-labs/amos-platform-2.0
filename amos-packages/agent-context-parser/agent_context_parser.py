"""
AGENT_CONTEXT.md Parser Library

Parses the AMOS AGENT_CONTEXT.md document into structured configuration objects.
Extracts: token parameters, decay mechanics, trust levels, bounty system rules,
tool categories, contribution multipliers, emission schedule, and more.

Usage:
    from agent_context_parser import parse_agent_context, AgentContext

    ctx = parse_agent_context("path/to/AGENT_CONTEXT.md")
    print(ctx.token.total_supply)       # 100_000_000
    print(ctx.decay.base_rate)          # 0.10
    print(ctx.trust.max_level)          # 5
    print(ctx.bounty.min_quality_score) # 30
    print(ctx.tools["system"])          # ["bash", "read_file", ...]
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


# ── Data classes ──────────────────────────────────────────────────────────────


@dataclass
class TokenParams:
    blockchain: str = "Solana"
    standard: str = "SPL"
    total_supply: int = 0
    initial_price: float = 0.0
    initial_fdv: float = 0.0
    initial_dex: str = ""
    bounty_treasury: int = 0
    emergency_reserve: int = 0


@dataclass
class RevenueSplit:
    protocol_fee_pct: float = 0.0
    staked_holders_pct: float = 0.0
    burned_pct: float = 0.0
    labs_pct: float = 0.0


@dataclass
class DecayParams:
    base_rate: float = 0.10
    multiplier: float = 0.05
    min_rate: float = 0.02
    max_rate: float = 0.25
    default_rate: float = 0.05
    inactivity_threshold_days: int = 90
    new_stake_grace_days: int = 365
    inactivity_grace_days: int = 90
    treasury_redistribution_pct: float = 0.90
    burned_redistribution_pct: float = 0.10
    minimum_preserved_pct: float = 0.10
    tenure_decay_floor: dict = field(default_factory=dict)
    tenure_decay_reduction: dict = field(default_factory=dict)


@dataclass
class VaultTier:
    lockup_days: Optional[int]  # None = permanent
    decay_reduction_pct: float


@dataclass
class TrustLevel:
    level: int
    max_points: int
    daily_bounty_limit: int


@dataclass
class TrustUpgrade:
    from_level: int
    to_level: int
    completions: int
    min_reputation_bps: int


@dataclass
class TrustParams:
    max_level: int = 5
    levels: list[TrustLevel] = field(default_factory=list)
    upgrades: list[TrustUpgrade] = field(default_factory=list)


@dataclass
class EmissionParams:
    ceiling: int = 16_000
    floor: int = 100
    midpoint_days: int = 1460
    k_scaled: int = 50


@dataclass
class PoolSeparation:
    ceiling_bps: int = 2000
    floor_bps: int = 300
    midpoint_days: int = 540
    k_scaled: int = 100
    technical_categories: list[str] = field(default_factory=list)
    growth_categories: list[str] = field(default_factory=list)


@dataclass
class BountyParams:
    min_quality_score: int = 30
    max_bounty_points: int = 2000
    max_daily_bounties: int = 50
    reviewer_reward_pct: float = 0.05
    claim_timeout_default_hours: int = 72
    claim_timeout_min_hours: int = 1
    claim_timeout_max_hours: int = 720
    dispute_window_hours: int = 48
    dispute_stake_bps: int = 500
    concurrent_claim_limits: dict[int, int] = field(default_factory=dict)
    contribution_multipliers: dict[str, int] = field(default_factory=dict)


@dataclass
class ToolCategory:
    name: str
    trust_level: int
    tools: list[str]


@dataclass
class AgentContext:
    token: TokenParams = field(default_factory=TokenParams)
    revenue: RevenueSplit = field(default_factory=RevenueSplit)
    decay: DecayParams = field(default_factory=DecayParams)
    vaults: dict[str, VaultTier] = field(default_factory=dict)
    trust: TrustParams = field(default_factory=TrustParams)
    emission: EmissionParams = field(default_factory=EmissionParams)
    pool_separation: PoolSeparation = field(default_factory=PoolSeparation)
    bounty: BountyParams = field(default_factory=BountyParams)
    tool_categories: list[ToolCategory] = field(default_factory=list)
    codebase_refs: dict[str, str] = field(default_factory=dict)
    raw_sections: dict[str, str] = field(default_factory=dict)


# ── Parser helpers ────────────────────────────────────────────────────────────


def _extract_yaml_value(text: str, key: str) -> Optional[str]:
    """Extract a value from YAML-like text by key."""
    pattern = rf"^\s*{re.escape(key)}:\s*(.+?)(?:\s*#.*)?$"
    m = re.search(pattern, text, re.MULTILINE)
    return m.group(1).strip() if m else None


def _parse_int(s: str) -> int:
    """Parse an integer, stripping commas and dollar signs."""
    return int(s.replace(",", "").replace("_", "").lstrip("$"))


def _parse_float(s: str) -> float:
    """Parse a float, stripping dollar signs and percent signs."""
    return float(s.replace(",", "").replace("_", "").lstrip("$").rstrip("%"))


def _extract_code_blocks(text: str) -> list[str]:
    """Extract all code blocks from a markdown section."""
    return re.findall(r"```(?:yaml|python|rust|typescript)?\n(.*?)```", text, re.DOTALL)


def _split_sections(text: str) -> dict[str, str]:
    """Split markdown into sections by ## headings."""
    sections = {}
    parts = re.split(r"\n## (\d+\.\s+.+)\n", text)
    for i in range(1, len(parts), 2):
        header = parts[i].strip()
        body = parts[i + 1] if i + 1 < len(parts) else ""
        sections[header] = body
    return sections


# ── Section parsers ───────────────────────────────────────────────────────────


def _parse_token_params(section: str) -> TokenParams:
    tp = TokenParams()
    blocks = _extract_code_blocks(section)
    if not blocks:
        return tp

    text = blocks[0]
    v = _extract_yaml_value(text, "blockchain")
    if v:
        tp.blockchain = v
    v = _extract_yaml_value(text, "standard")
    if v:
        tp.standard = v
    v = _extract_yaml_value(text, "total_supply")
    if v:
        tp.total_supply = _parse_int(v)
    v = _extract_yaml_value(text, "initial_price")
    if v:
        tp.initial_price = _parse_float(v)
    v = _extract_yaml_value(text, "initial_fdv")
    if v:
        tp.initial_fdv = _parse_float(v)
    v = _extract_yaml_value(text, "initial_dex")
    if v:
        tp.initial_dex = v
    v = _extract_yaml_value(text, "bounty_treasury")
    if v:
        tp.bounty_treasury = _parse_int(v)
    v = _extract_yaml_value(text, "emergency_reserve")
    if v:
        tp.emergency_reserve = _parse_int(v)

    return tp


def _parse_revenue(section: str) -> RevenueSplit:
    rs = RevenueSplit()
    blocks = _extract_code_blocks(section)
    if not blocks:
        return rs

    text = blocks[0]
    v = _extract_yaml_value(text, "protocol_fee")
    if v:
        rs.protocol_fee_pct = _parse_float(v) / 100.0

    # Fee split block
    v = _extract_yaml_value(text, "staked_holders")
    if v:
        rs.staked_holders_pct = _parse_float(v) / 100.0
    v = _extract_yaml_value(text, "burned")
    if v:
        rs.burned_pct = _parse_float(v) / 100.0
    v = _extract_yaml_value(text, "labs")
    if v:
        rs.labs_pct = _parse_float(v) / 100.0

    return rs


def _parse_decay(section: str) -> DecayParams:
    dp = DecayParams()
    blocks = _extract_code_blocks(section)

    for block in blocks:
        # Activity definition
        v = _extract_yaml_value(block, "inactivity_threshold")
        if v:
            m = re.search(r"(\d+)", v)
            if m:
                dp.inactivity_threshold_days = int(m.group(1))

        v = _extract_yaml_value(block, "new_stake_grace")
        if v:
            m = re.search(r"(\d+)", v)
            if m:
                dp.new_stake_grace_days = int(m.group(1))

        v = _extract_yaml_value(block, "inactivity_grace")
        if v:
            m = re.search(r"(\d+)", v)
            if m:
                dp.inactivity_grace_days = int(m.group(1))

        # Redistribution
        v = _extract_yaml_value(block, "to_treasury")
        if v:
            dp.treasury_redistribution_pct = _parse_float(v) / 100.0
        v = _extract_yaml_value(block, "burned")
        if v and "redistribution" not in dp.__dict__.get("_parsed", ""):
            dp.burned_redistribution_pct = _parse_float(v) / 100.0

        v = _extract_yaml_value(block, "minimum_preserved")
        if v:
            dp.minimum_preserved_pct = _parse_float(v) / 100.0

        # Tenure decay floor
        for year_key in ["year_0_to_1", "year_1_to_2", "year_2_to_5", "year_5_plus"]:
            v = _extract_yaml_value(block, year_key)
            if v:
                pct = _parse_float(v) / 100.0
                # Distinguish floor vs reduction by checking which sub-section
                if "tenure_decay_floor" in block:
                    dp.tenure_decay_floor[year_key] = pct
                elif "tenure_decay_reduction" in block:
                    dp.tenure_decay_reduction[year_key] = pct
                else:
                    # Heuristic: if value > 0.25, it's likely reduction
                    if pct > 0.30:
                        dp.tenure_decay_reduction[year_key] = pct
                    else:
                        dp.tenure_decay_floor[year_key] = pct

    return dp


def _parse_vaults(section: str) -> dict[str, VaultTier]:
    vaults = {}
    blocks = _extract_code_blocks(section)
    for block in blocks:
        for m in re.finditer(
            r"(\w+):\s*\{\s*lockup:\s*([\w]+)\s*(?:days?)?,?\s*decay_reduction:\s*(\d+)%",
            block,
        ):
            name = m.group(1)
            lockup_str = m.group(2).rstrip(",")
            reduction = float(m.group(3)) / 100.0
            lockup_days = None if lockup_str == "no_unlock" else int(lockup_str)
            vaults[name] = VaultTier(lockup_days=lockup_days, decay_reduction_pct=reduction)
    return vaults


def _parse_trust(section: str) -> TrustParams:
    tp = TrustParams()
    blocks = _extract_code_blocks(section)
    if not blocks:
        return tp

    text = blocks[0]
    v = _extract_yaml_value(text, "max_trust_level")
    if v:
        tp.max_level = int(v)

    # Parse level parameters
    for m in re.finditer(
        r"level_(\d+):\s*\{\s*max_points:\s*(\d+),?\s*daily_bounty_limit:\s*(\d+)\s*\}",
        text,
    ):
        tp.levels.append(
            TrustLevel(
                level=int(m.group(1)),
                max_points=int(m.group(2)),
                daily_bounty_limit=int(m.group(3)),
            )
        )

    # Parse upgrade requirements
    for m in re.finditer(
        r"level_(\d+)_to_(\d+):\s*\{\s*completions:\s*(\d+),?\s*min_reputation_bps:\s*(\d+)\s*\}",
        text,
    ):
        tp.upgrades.append(
            TrustUpgrade(
                from_level=int(m.group(1)),
                to_level=int(m.group(2)),
                completions=int(m.group(3)),
                min_reputation_bps=int(m.group(4)),
            )
        )

    return tp


def _parse_emission(section: str) -> EmissionParams:
    ep = EmissionParams()
    blocks = _extract_code_blocks(section)
    for block in blocks:
        v = _extract_yaml_value(block, "emission_ceiling")
        if v:
            m = re.search(r"([\d,_]+)", v)
            if m:
                ep.ceiling = _parse_int(m.group(1))
        v = _extract_yaml_value(block, "emission_floor")
        if v:
            m = re.search(r"([\d,_]+)", v)
            if m:
                ep.floor = _parse_int(m.group(1))
        v = _extract_yaml_value(block, "emission_midpoint_days")
        if v:
            m = re.search(r"([\d,_]+)", v)
            if m:
                ep.midpoint_days = _parse_int(m.group(1))
        v = _extract_yaml_value(block, "emission_k_scaled")
        if v:
            ep.k_scaled = int(v)
    return ep


def _parse_pool_separation(section: str) -> PoolSeparation:
    ps = PoolSeparation()
    blocks = _extract_code_blocks(section)
    for block in blocks:
        v = _extract_yaml_value(block, "ceiling_bps")
        if v:
            ps.ceiling_bps = _parse_int(v)
        v = _extract_yaml_value(block, "floor_bps")
        if v:
            ps.floor_bps = _parse_int(v)
        v = _extract_yaml_value(block, "midpoint_days")
        if v:
            ps.midpoint_days = _parse_int(v)
        v = _extract_yaml_value(block, "k_scaled")
        if v:
            ps.k_scaled = _parse_int(v)

        # Pool categories
        m = re.search(r"technical:\s*\[([^\]]+)\]", block)
        if m:
            ps.technical_categories = [c.strip() for c in m.group(1).split(",")]
        m = re.search(r"growth:\s*\[([^\]]+)\]", block)
        if m:
            ps.growth_categories = [c.strip() for c in m.group(1).split(",")]

    return ps


def _parse_bounty_params(section: str) -> BountyParams:
    bp = BountyParams()
    blocks = _extract_code_blocks(section)
    for block in blocks:
        v = _extract_yaml_value(block, "min_quality_score")
        if v:
            bp.min_quality_score = _parse_int(v)
        v = _extract_yaml_value(block, "max_bounty_points")
        if v:
            bp.max_bounty_points = _parse_int(v)
        v = _extract_yaml_value(block, "max_daily_bounties")
        if v:
            bp.max_daily_bounties = _parse_int(v)
        v = _extract_yaml_value(block, "reviewer_reward")
        if v:
            bp.reviewer_reward_pct = _parse_float(v) / 100.0

        # Claim timeouts
        v = _extract_yaml_value(block, "default_hours")
        if v:
            bp.claim_timeout_default_hours = int(v)
        v = _extract_yaml_value(block, "min_hours")
        if v:
            bp.claim_timeout_min_hours = int(v)
        v = _extract_yaml_value(block, "max_hours")
        if v:
            bp.claim_timeout_max_hours = int(v)

        # Dispute
        m = re.search(r"dispute_window:\s*\n\s*hours:\s*(\d+)", block)
        if m:
            bp.dispute_window_hours = int(m.group(1))
        v = _extract_yaml_value(block, "stake_bps")
        if v:
            bp.dispute_stake_bps = _parse_int(v)

        # Concurrent claim limits
        for m in re.finditer(r"trust_level_(\d+):\s*(\d+)", block):
            bp.concurrent_claim_limits[int(m.group(1))] = int(m.group(2))

        # Contribution multipliers
        for m in re.finditer(r"(\w+):\s*(\d+)%\s*#", block):
            name = m.group(1)
            if name not in ("protocol_fee", "burned", "staked_holders", "labs"):
                bp.contribution_multipliers[name] = int(m.group(2))

    return bp


def _parse_tools(section: str) -> list[ToolCategory]:
    """Parse tool categories from sections 8 (Available Harness Tools)."""
    categories = []

    # Split by ### Trust Level headers
    parts = re.split(r"### Trust Level (\d+)", section)
    for i in range(1, len(parts), 2):
        trust_level = int(parts[i])
        body = parts[i + 1] if i + 1 < len(parts) else ""

        # Find each category block within this trust level
        blocks = _extract_code_blocks(body)
        for block in blocks:
            # Parse category names and their tools
            current_cat = None
            tools = []
            for line in block.split("\n"):
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                # Category header: "system:" or "web:"
                cat_match = re.match(r"^(\w+):$", line)
                if cat_match:
                    if current_cat and tools:
                        categories.append(
                            ToolCategory(name=current_cat, trust_level=trust_level, tools=tools)
                        )
                    current_cat = cat_match.group(1)
                    tools = []
                    continue
                # Tool entry: "  - bash  # description"
                tool_match = re.match(r"-\s+(\w+)", line)
                if tool_match:
                    tools.append(tool_match.group(1))

            if current_cat and tools:
                categories.append(
                    ToolCategory(name=current_cat, trust_level=trust_level, tools=tools)
                )

    return categories


def _parse_codebase_refs(section: str) -> dict[str, str]:
    refs = {}
    blocks = _extract_code_blocks(section)
    for block in blocks:
        for line in block.split("\n"):
            line = line.strip()
            if ":" in line and not line.startswith("#"):
                key, _, val = line.partition(":")
                refs[key.strip()] = val.strip()
    return refs


# ── Main parser ───────────────────────────────────────────────────────────────


def parse_agent_context(path: str | Path) -> AgentContext:
    """Parse an AGENT_CONTEXT.md file into a structured AgentContext object.

    Args:
        path: Path to the AGENT_CONTEXT.md file.

    Returns:
        AgentContext with all extracted parameters.

    Raises:
        FileNotFoundError: If the file doesn't exist.
        ValueError: If the file doesn't look like a valid AGENT_CONTEXT.md.
    """
    text = Path(path).read_text(encoding="utf-8")

    if "AMOS Agent Context" not in text[:200]:
        raise ValueError(f"{path} does not appear to be a valid AGENT_CONTEXT.md")

    sections = _split_sections(text)
    ctx = AgentContext(raw_sections=sections)

    for header, body in sections.items():
        header_lower = header.lower()
        if "token parameters" in header_lower:
            ctx.token = _parse_token_params(body)
        elif "revenue distribution" in header_lower:
            ctx.revenue = _parse_revenue(body)
        elif "decay mechanics" in header_lower:
            ctx.decay = _parse_decay(body)
            ctx.vaults = _parse_vaults(body)
        elif "trust system" in header_lower:
            ctx.trust = _parse_trust(body)
        elif "bounty system" in header_lower:
            ctx.bounty = _parse_bounty_params(body)
            ctx.emission = _parse_emission(body)
            ctx.pool_separation = _parse_pool_separation(body)
        elif "available harness tools" in header_lower:
            ctx.tool_categories = _parse_tools(body)
        elif "key codebase references" in header_lower:
            ctx.codebase_refs = _parse_codebase_refs(body)

    return ctx


def compute_emission(ctx: AgentContext, day: int) -> float:
    """Compute the daily emission for a given day since launch.

    Uses the sigmoid formula from AGENT_CONTEXT.md:
        emission(t) = floor + (ceiling - floor) / (1 + e^(k * (t - midpoint)))
    """
    import math

    e = ctx.emission
    k = e.k_scaled / 10000.0  # k_scaled = 50 → k = 0.005
    exponent = k * (day - e.midpoint_days)
    return e.floor + (e.ceiling - e.floor) / (1.0 + math.exp(exponent))


def compute_decay_rate(profit_ratio: float) -> float:
    """Compute the decay rate for a given profit ratio.

    Formula: rate = 10% - (P × 5%), clamped to [2%, 25%]
    """
    rate = 0.10 - (profit_ratio * 0.05)
    return max(0.02, min(0.25, rate))


def get_tools_for_trust_level(ctx: AgentContext, trust_level: int) -> dict[str, list[str]]:
    """Get all tools available to an agent at a given trust level."""
    result = {}
    for cat in ctx.tool_categories:
        if cat.trust_level <= trust_level:
            result[cat.name] = cat.tools
    return result


if __name__ == "__main__":
    import sys

    path = sys.argv[1] if len(sys.argv) > 1 else "AGENT_CONTEXT.md"
    ctx = parse_agent_context(path)

    print(f"Token: {ctx.token.total_supply:,} {ctx.token.standard} on {ctx.token.blockchain}")
    print(f"  Treasury: {ctx.token.bounty_treasury:,} ({ctx.token.bounty_treasury/ctx.token.total_supply*100:.0f}%)")
    print(f"  Reserve:  {ctx.token.emergency_reserve:,} ({ctx.token.emergency_reserve/ctx.token.total_supply*100:.0f}%)")
    print(f"\nRevenue: {ctx.revenue.protocol_fee_pct*100:.0f}% fee → "
          f"{ctx.revenue.staked_holders_pct*100:.0f}% stakers, "
          f"{ctx.revenue.burned_pct*100:.0f}% burned, "
          f"{ctx.revenue.labs_pct*100:.0f}% labs")
    print(f"\nDecay: base={ctx.decay.base_rate*100:.0f}%, "
          f"min={ctx.decay.min_rate*100:.0f}%, max={ctx.decay.max_rate*100:.0f}%")
    print(f"  Grace: {ctx.decay.new_stake_grace_days}d new stake, "
          f"{ctx.decay.inactivity_grace_days}d inactivity")
    print(f"\nTrust: {ctx.trust.max_level} levels, "
          f"{len(ctx.trust.levels)} parsed, "
          f"{len(ctx.trust.upgrades)} upgrades")
    for lvl in ctx.trust.levels:
        print(f"  L{lvl.level}: {lvl.max_points} pts, {lvl.daily_bounty_limit}/day")
    print(f"\nEmission: {ctx.emission.ceiling:,}/day → {ctx.emission.floor}/day "
          f"(midpoint day {ctx.emission.midpoint_days:,})")
    print(f"  Year 1: ~{compute_emission(ctx, 182):.0f}/day")
    print(f"  Year 4: ~{compute_emission(ctx, 1460):.0f}/day")
    print(f"  Year 10: ~{compute_emission(ctx, 3650):.0f}/day")
    print(f"\nBounty: min_quality={ctx.bounty.min_quality_score}, "
          f"max_pts={ctx.bounty.max_bounty_points}, "
          f"timeout={ctx.bounty.claim_timeout_default_hours}h")
    print(f"  Multipliers: {len(ctx.bounty.contribution_multipliers)}")
    for name, pct in sorted(ctx.bounty.contribution_multipliers.items(), key=lambda x: -x[1]):
        print(f"    {name}: {pct}%")
    print(f"\nTools: {len(ctx.tool_categories)} categories")
    for cat in ctx.tool_categories:
        print(f"  L{cat.trust_level} {cat.name}: {', '.join(cat.tools)}")
    print(f"\nVaults: {len(ctx.vaults)}")
    for name, v in ctx.vaults.items():
        lockup = f"{v.lockup_days}d" if v.lockup_days else "permanent"
        print(f"  {name}: lockup={lockup}, reduction={v.decay_reduction_pct*100:.0f}%")
    print(f"\nCodebase refs: {len(ctx.codebase_refs)}")
