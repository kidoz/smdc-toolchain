//! Intermediate Representation module
//!
//! Three-address code IR for optimization and code generation.

mod inst;
mod builder;

pub use inst::*;
pub use builder::IrBuilder;
