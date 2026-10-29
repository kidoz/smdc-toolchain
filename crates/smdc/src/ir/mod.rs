//! Intermediate Representation module
//!
//! Three-address code IR for optimization and code generation.

mod builder;
mod inst;
mod opt;

pub use builder::IrBuilder;
pub use inst::*;
pub use opt::optimize_module;
