//! SMD Compiler - C and Rust compiler for Sega Megadrive/Genesis
//!
//! This library provides C and Rust compilers targeting the Motorola 68000 CPU
//! used in the Sega Megadrive/Genesis console.
//!
//! ## Architecture
//!
//! The compiler is organized into:
//! - **Frontends** (`frontend/`): Language-specific parsing and analysis (C, Rust)
//! - **IR** (`ir/`): Shared intermediate representation
//! - **Backends** (`backend/`): Target-specific code generation (M68k, ROM)
//! - **Common** (`common/`): Shared infrastructure (errors, spans)
//! - **Types** (`types/`): Language-agnostic type system

pub mod common;
pub mod types;
pub mod frontend;
pub mod backend;
pub mod driver;
pub mod ir;

// Re-exports for convenience
pub use common::{CompileError, CompileResult, DiagnosticReporter, Span};
pub use frontend::{Frontend, FrontendConfig, FrontendRegistry, CompileContext};
pub use backend::{Backend, BackendConfig, BackendRegistry, BackendOutput, OutputFormat};
