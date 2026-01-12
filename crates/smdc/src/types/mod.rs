//! Unified type system for the IR
//!
//! Language-agnostic types used in the intermediate representation,
//! with M68k-specific size and alignment considerations.

mod ir_type;

pub use ir_type::{IrType, IrTypeKind};
