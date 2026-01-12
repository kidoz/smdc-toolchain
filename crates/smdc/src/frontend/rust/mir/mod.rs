//! Mid-level IR for Rust frontend
//!
//! MIR is an intermediate representation between the Rust AST and
//! the shared IR. It makes control flow explicit and desugars patterns.

mod types;
mod lower;
mod to_ir;

pub use types::*;
pub use lower::MirLowerer;
pub use to_ir::MirToIr;
