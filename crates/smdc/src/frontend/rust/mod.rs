//! Rust frontend for SMD Compiler
//!
//! This module provides a Rust language frontend that compiles a subset
//! of Rust ("Genesis Rust") to the shared IR, enabling Rust code to
//! target the Sega Megadrive/Genesis.

pub mod lexer;
pub mod ast;
pub mod parser;
pub mod sema;
pub mod mir;

pub use lexer::RustLexer;
pub use parser::RustParser;
pub use sema::RustAnalyzer;

use crate::common::CompileResult;
use crate::frontend::{CompileContext, Frontend, FrontendConfig};
use crate::ir::IrModule;

/// Rust language frontend
pub struct RustFrontend;

impl RustFrontend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RustFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl Frontend for RustFrontend {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".rs"]
    }

    fn compile(
        &self,
        source: &str,
        ctx: &CompileContext,
        config: &FrontendConfig,
    ) -> CompileResult<IrModule> {
        use ast::ItemKind;
        use mir::{MirLowerer, MirToIr};

        // Phase 1: Lexing (optional token dump)
        if config.dump_tokens {
            let lexer = RustLexer::new(source);
            match lexer.tokenize_all() {
                Ok(tokens) => {
                    eprintln!("=== Rust Tokens ===");
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
            eprintln!("Parsing Rust...");
        }

        let mut parser = RustParser::new(source);
        let mut module = match parser.parse_module() {
            Ok(m) => m,
            Err(e) => {
                ctx.reporter.report_error(ctx.file_id, &e);
                return Err(e);
            }
        };

        if config.dump_ast {
            eprintln!("=== Rust AST ===");
            eprintln!("{:#?}", module);
            eprintln!("=== End AST ===\n");
        }

        // Phase 3: Semantic Analysis
        if config.verbose {
            eprintln!("Analyzing Rust...");
        }

        let mut analyzer = RustAnalyzer::new();
        if let Err(e) = analyzer.analyze(&mut module) {
            ctx.reporter.report_error(ctx.file_id, &e);
            return Err(e);
        }

        // Phase 4: MIR Generation and conversion to shared IR
        if config.verbose {
            eprintln!("Generating IR...");
        }

        let mut ir_module = IrModule::new();

        // Process each function in the module
        for item in &module.items {
            if let ItemKind::Fn(func) = &item.kind {
                if func.body.is_some() {
                    // Get return type
                    let return_type = func.return_type.clone()
                        .unwrap_or_else(|| ast::RustType::unit(func.span));

                    // Lower to MIR
                    let lowerer = MirLowerer::new(return_type);
                    let mir_body = match lowerer.lower_function(func) {
                        Ok(m) => m,
                        Err(e) => {
                            ctx.reporter.report_error(ctx.file_id, &e);
                            return Err(e);
                        }
                    };

                    if config.dump_mir {
                        eprintln!("=== MIR for {} ===", func.name);
                        eprintln!("{:#?}", mir_body);
                        eprintln!("=== End MIR ===\n");
                    }

                    // Convert MIR to shared IR
                    let mut converter = MirToIr::new();
                    let ir_func = converter.convert(func.name.clone(), &mir_body);
                    ir_module.functions.push(ir_func);
                }
            }
        }

        Ok(ir_module)
    }

    fn dump_tokens(&self, source: &str) -> CompileResult<String> {
        let lexer = RustLexer::new(source);
        let tokens = lexer.tokenize_all()?;
        let mut output = String::new();
        for token in &tokens {
            output.push_str(&format!("{:?}\n", token));
        }
        Ok(output)
    }

    fn dump_ast(&self, source: &str) -> CompileResult<String> {
        let mut parser = RustParser::new(source);
        let module = parser.parse_module()?;
        Ok(format!("{:#?}", module))
    }
}
