# amos-core

Foundation crate for the AMOS workspace. Every other crate depends on this one.

## Contents

- **AppConfig** -- Hierarchical configuration (env vars, files, defaults) using `AMOS__` prefix
- **DeploymentMode** -- Managed vs self-hosted deployment configuration
- **AmosError / Result** -- Unified error types used across all crates
- **Domain Types** -- `ToolDefinition`, `ContentBlock`, `Message`, `Role`, etc.
- **Token Economics** -- AMOS token distribution, vesting, and reward calculations
- **Points Economy** -- Contribution tracking with multipliers for different activity types

## Design Principles

- No dependencies on other workspace crates
- No database, no network -- pure types and logic
- All shared types live here so crates can communicate without depending on each other
