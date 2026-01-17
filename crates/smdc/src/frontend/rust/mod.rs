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
use crate::ir::{IrModule, IrGlobal};
use crate::frontend::c::ast::CType;
use crate::common::Span;
use std::collections::{HashMap, HashSet};

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

        // First pass: collect const values and add statics as globals
        let mut const_values: HashMap<String, i64> = HashMap::new();
        let mut static_names: HashSet<String> = HashSet::new();

        for item in &module.items {
            match &item.kind {
                ItemKind::Const(c) => {
                    // Evaluate const value (only integer literals for now)
                    if let Some(value) = Self::eval_const_expr(&c.value) {
                        const_values.insert(c.name.clone(), value);
                    }
                }
                ItemKind::Static(s) => {
                    static_names.insert(s.name.clone());

                    // Add to IR globals with initial value
                    let init_value = Self::eval_const_expr(&s.value);
                    let init_bytes = init_value.map(|v| {
                        // Convert i64 to 4 bytes (i32)
                        (v as i32).to_be_bytes().to_vec()
                    });

                    ir_module.globals.push(IrGlobal {
                        name: s.name.clone(),
                        ty: CType::int(Span::default()), // Use i32 for now
                        init: init_bytes,
                    });
                }
                _ => {}
            }
        }

        // Second pass: process functions with const/static info
        for item in &module.items {
            if let ItemKind::Fn(func) = &item.kind {
                if func.body.is_some() {
                    // Get return type
                    let return_type = func.return_type.clone()
                        .unwrap_or_else(|| ast::RustType::unit(func.span));

                    // Lower to MIR with const values and static names
                    let lowerer = MirLowerer::with_constants(
                        return_type,
                        const_values.clone(),
                        static_names.clone(),
                    );
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

impl RustFrontend {
    /// Evaluate a constant expression to an integer value
    fn eval_const_expr(expr: &ast::Expr) -> Option<i64> {
        use ast::ExprKind;
        use ast::BinOp;
        use ast::UnaryOp;

        match &expr.kind {
            ExprKind::IntLiteral(v) => Some(*v),
            ExprKind::BoolLiteral(b) => Some(if *b { 1 } else { 0 }),
            ExprKind::CharLiteral(c) => Some(*c as i64),
            ExprKind::Unary { op, operand } => {
                let val = Self::eval_const_expr(operand)?;
                match op {
                    UnaryOp::Neg => Some(-val),
                    UnaryOp::Not => Some(!val),
                    _ => None,
                }
            }
            ExprKind::Binary { op, left, right } => {
                let l = Self::eval_const_expr(left)?;
                let r = Self::eval_const_expr(right)?;
                match op {
                    BinOp::Add => Some(l + r),
                    BinOp::Sub => Some(l - r),
                    BinOp::Mul => Some(l * r),
                    BinOp::Div => Some(l / r),
                    BinOp::Rem => Some(l % r),
                    BinOp::BitAnd => Some(l & r),
                    BinOp::BitOr => Some(l | r),
                    BinOp::BitXor => Some(l ^ r),
                    BinOp::Shl => Some(l << r),
                    BinOp::Shr => Some(l >> r),
                    _ => None,
                }
            }
            ExprKind::Cast { expr, .. } => {
                // For casts, just pass through the value (simplification)
                Self::eval_const_expr(expr)
            }
            _ => None,
        }
    }
}
