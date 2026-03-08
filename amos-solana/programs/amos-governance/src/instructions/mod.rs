// AMOS Governance Program - Instructions Module
// Exports all instruction handlers

pub mod governance;
pub mod proposals;
pub mod gates;
pub mod rewards;
pub mod research;
pub mod priority;

pub use governance::*;
pub use proposals::*;
pub use gates::*;
pub use rewards::*;
pub use research::*;
pub use priority::*;
