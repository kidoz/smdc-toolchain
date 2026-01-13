//! C language frontend
//!
//! This frontend handles:
//! - Preprocessing (#include directives)
//! - Lexing C source into tokens
//! - Parsing tokens into a C AST
//! - Semantic analysis (type checking, symbol resolution)
//! - Lowering to the shared IR

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod preprocessor;
pub mod sema;

use std::path::Path;

use crate::common::{CompileError, CompileResult};
use crate::frontend::{CompileContext, Frontend, FrontendConfig};
use crate::ir::IrModule;

pub use ast::*;
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::Parser;
pub use sema::SemanticAnalyzer;

/// C language frontend
pub struct CFrontend;

impl CFrontend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl Frontend for CFrontend {
    fn name(&self) -> &'static str {
        "c"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".c", ".h"]
    }

    fn compile(
        &self,
        source: &str,
        ctx: &CompileContext,
        config: &FrontendConfig,
    ) -> CompileResult<IrModule> {
        // Phase 0: Preprocessing (#include expansion)
        let source_path = Path::new(&ctx.filename);
        if config.verbose {
            eprintln!("Preprocessing...");
        }
        let processed_source = match preprocessor::preprocess(
            source,
            source_path,
            config.include_paths.clone(),
        ) {
            Ok(s) => s,
            Err(e) => {
                ctx.reporter.report_error(ctx.file_id, &e);
                return Err(e);
            }
        };

        let source = &processed_source;

        // Phase 1: Lexing (optional token dump)
        if config.dump_tokens {
            let lexer = Lexer::new(source);
            match lexer.tokenize_all() {
                Ok(tokens) => {
                    eprintln!("=== C Tokens ===");
                    for token in &tokens {
                        eprintln!("{:?}", token);
                    }
                    eprintln!("=== End Tokens ===\n");
                }
                Err(e) => {
                    ctx.reporter.report_error(ctx.file_id, &e);
                    return Err(e);
                }
            }
        }

        // Phase 2: Parsing
        if config.verbose {
            eprintln!("Parsing C...");
        }

        let mut parser = match Parser::new(source) {
            Ok(p) => p,
            Err(e) => {
                ctx.reporter.report_error(ctx.file_id, &e);
                return Err(e);
            }
        };

        let mut ast = match parser.parse() {
            Ok(ast) => ast,
            Err(e) => {
                ctx.reporter.report_error(ctx.file_id, &e);
                return Err(CompileError::parser("syntax error", crate::common::Span::default()));
            }
        };

        if config.dump_ast {
            eprintln!("=== C AST ===");
            eprintln!("{:#?}", ast);
            eprintln!("=== End AST ===\n");
        }

        // Phase 3: Semantic Analysis
        if config.verbose {
            eprintln!("Analyzing...");
        }

        let mut analyzer = SemanticAnalyzer::new();
        if let Err(e) = analyzer.analyze(&mut ast) {
            ctx.reporter.report_error(ctx.file_id, &e);
            return Err(e);
        }

        // Phase 4: IR Generation
        if config.verbose {
            eprintln!("Generating IR...");
        }

        let mut ir_builder = crate::ir::IrBuilder::new();
        let ir_module = match ir_builder.build(&ast) {
            Ok(m) => m,
            Err(e) => {
                ctx.reporter.report_error(ctx.file_id, &e);
                return Err(e);
            }
        };

        Ok(ir_module)
    }

    fn dump_tokens(&self, source: &str) -> CompileResult<String> {
        let lexer = Lexer::new(source);
        let tokens = lexer.tokenize_all()?;
        let mut output = String::new();
        for token in &tokens {
            output.push_str(&format!("{:?}\n", token));
        }
        Ok(output)
    }

    fn dump_ast(&self, source: &str) -> CompileResult<String> {
        let mut parser = Parser::new(source)?;
        let ast = parser.parse()?;
        Ok(format!("{:#?}", ast))
    }
}
