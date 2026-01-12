//! Frontend trait and implementations
//!
//! Frontends are responsible for:
//! 1. Lexing source code into tokens
//! 2. Parsing tokens into an AST
//! 3. Performing semantic analysis
//! 4. Lowering to the shared IR

pub mod c;
pub mod rust;

use crate::common::{CompileResult, DiagnosticReporter};
use crate::ir::IrModule;

pub use c::CFrontend;
pub use rust::RustFrontend;

/// Configuration options passed to frontends
#[derive(Debug, Clone, Default)]
pub struct FrontendConfig {
    pub dump_tokens: bool,
    pub dump_ast: bool,
    pub dump_mir: bool,  // For frontends with intermediate representations
    pub verbose: bool,
}

/// Compilation context providing access to diagnostics and file info
pub struct CompileContext<'a> {
    pub filename: String,
    pub file_id: usize,
    pub reporter: &'a DiagnosticReporter,
}

impl<'a> CompileContext<'a> {
    pub fn new(filename: String, file_id: usize, reporter: &'a DiagnosticReporter) -> Self {
        Self { filename, file_id, reporter }
    }
}

/// Trait for language frontends
///
/// A frontend is responsible for taking source code and producing IR.
pub trait Frontend: Send + Sync {
    /// The name of this frontend (e.g., "c", "rust")
    fn name(&self) -> &'static str;

    /// File extensions this frontend handles (e.g., &[".c", ".h"] or &[".rs"])
    fn extensions(&self) -> &'static [&'static str];

    /// Compile source code to IR
    ///
    /// This is the main entry point that orchestrates the entire
    /// frontend pipeline: lex -> parse -> analyze -> lower
    fn compile(
        &self,
        source: &str,
        ctx: &CompileContext,
        config: &FrontendConfig,
    ) -> CompileResult<IrModule>;

    /// Optional: dump tokens for debugging
    fn dump_tokens(&self, source: &str) -> CompileResult<String> {
        let _ = source;
        Ok(String::new())
    }

    /// Optional: dump AST for debugging
    fn dump_ast(&self, source: &str) -> CompileResult<String> {
        let _ = source;
        Ok(String::new())
    }
}

/// Registry of available frontends
pub struct FrontendRegistry {
    frontends: Vec<Box<dyn Frontend>>,
}

impl FrontendRegistry {
    pub fn new() -> Self {
        Self { frontends: Vec::new() }
    }

    pub fn register(&mut self, frontend: Box<dyn Frontend>) {
        self.frontends.push(frontend);
    }

    pub fn find_by_extension(&self, ext: &str) -> Option<&dyn Frontend> {
        self.frontends.iter()
            .find(|f| f.extensions().iter().any(|e| *e == ext))
            .map(|f| f.as_ref())
    }

    pub fn find_by_name(&self, name: &str) -> Option<&dyn Frontend> {
        self.frontends.iter()
            .find(|f| f.name() == name)
            .map(|f| f.as_ref())
    }

    pub fn list(&self) -> impl Iterator<Item = &dyn Frontend> {
        self.frontends.iter().map(|f| f.as_ref())
    }
}

impl Default for FrontendRegistry {
    fn default() -> Self {
        Self::new()
    }
}
