//! Token economics module — single source of truth for all AMOS tokenomics.
//!
//! This module encodes the exact mathematics from the whitepaper and
//! `token_economy_equations.md`. Every constant and formula here must match
//! the on-chain Anchor programs bit-for-bit.

pub mod economics;
pub mod decay;
pub mod revenue;
pub mod emission;
pub mod trust;
pub mod points;
