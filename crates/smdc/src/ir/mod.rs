//! Intermediate Representation module
//!
//! Three-address code IR for optimization and code generation.

mod builder;
mod inst;

pub use builder::IrBuilder;
pub use inst::*;
