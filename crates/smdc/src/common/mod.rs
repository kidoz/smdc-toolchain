//! Common infrastructure shared across frontends and backends

mod error;
mod span;

pub use error::{CompileError, CompileResult, DiagnosticReporter};
pub use span::{Span, byte_offset_to_line};
