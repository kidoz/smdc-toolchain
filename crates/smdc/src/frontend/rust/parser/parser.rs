//! Rust recursive descent parser

use crate::common::{CompileError, CompileResult, Span};
use crate::frontend::rust::lexer::{RustLexer, RustToken, RustTokenKind};
use crate::frontend::rust::ast::*;

/// Rust parser
pub struct RustParser<'a> {
    lexer: RustLexer<'a>,
    /// Whether struct literals (e.g., `Foo { field: value }`) are allowed in the current context
    struct_literal_allowed: bool,
}

impl<'a> RustParser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            lexer: RustLexer::new(source),
            struct_literal_allowed: true,
        }
    }

    /// Parse a complete module (file)
    pub fn parse_module(&mut self) -> CompileResult<RustModule> {
        let start = self.lexer.peek()?.span;
        let mut items = Vec::new();

        while !self.check(&RustTokenKind::Eof)? {
            items.push(self.parse_item()?);
        }

        let end = self.lexer.peek()?.span;
        Ok(RustModule::new(items, Span::new(start.start, end.end)))
    }

    // ==================== Items ====================

    fn parse_item(&mut self) -> CompileResult<Item> {
        let attrs = self.parse_attributes()?;
        let visibility = self.parse_visibility()?;
        let start = self.lexer.peek()?.span;

        let kind = if self.check(&RustTokenKind::Fn)? ||
                     self.check(&RustTokenKind::Unsafe)? ||
                     self.check_const_fn()? {
            self.parse_fn_item()?
        } else if self.check(&RustTokenKind::Const)? {
            self.parse_const_item()?
        } else if self.check(&RustTokenKind::Struct)? {
            self.parse_struct_item()?
        } else if self.check(&RustTokenKind::Enum)? {
            self.parse_enum_item()?
        } else if self.check(&RustTokenKind::Impl)? {
            self.parse_impl_item()?
        } else if self.check(&RustTokenKind::Type)? {
            self.parse_type_alias()?
        } else if self.check(&RustTokenKind::Const)? {
            self.parse_const_item()?
        } else if self.check(&RustTokenKind::Static)? {
            self.parse_static_item()?
        } else if self.check(&RustTokenKind::Mod)? {
            self.parse_mod_item()?
        } else if self.check(&RustTokenKind::Use)? {
            self.parse_use_item()?
        } else {
            let token = self.lexer.next_token()?;
            return Err(CompileError::parser(
                format!("expected item, found {}", token.kind),
                token.span,
            ));
        };

        let end = self.lexer.peek()?.span;
        let mut item = Item::new(kind, visibility, Span::new(start.start, end.start));
        item.attrs = attrs;
        Ok(item)
    }

    fn parse_attributes(&mut self) -> CompileResult<Vec<Attribute>> {
        let mut attrs = Vec::new();
        while self.match_token(&RustTokenKind::Hash)? {
            self.expect(RustTokenKind::LBracket)?;
            let path = self.parse_path()?;

            let args = if self.match_token(&RustTokenKind::LParen)? {
                let mut depth = 1;
                let mut args = String::new();
                while depth > 0 {
                    let token = self.lexer.next_token()?;
                    match &token.kind {
                        RustTokenKind::LParen => { depth += 1; args.push('('); }
                        RustTokenKind::RParen => {
                            depth -= 1;
                            if depth > 0 { args.push(')'); }
                        }
                        _ => args.push_str(&token.kind.to_string()),
                    }
                }
                Some(args)
            } else {
                None
            };

            let end = self.expect(RustTokenKind::RBracket)?;
            attrs.push(Attribute {
                path,
                args,
                span: end.span,
            });
        }
        Ok(attrs)
    }

    fn parse_visibility(&mut self) -> CompileResult<Visibility> {
        if self.match_token(&RustTokenKind::Pub)? {
            if self.match_token(&RustTokenKind::LParen)? {
                let vis = if self.match_token(&RustTokenKind::Crate)? {
                    Visibility::Crate
                } else if self.match_token(&RustTokenKind::Super)? {
                    Visibility::Super
                } else {
                    Visibility::Public
                };
                self.expect(RustTokenKind::RParen)?;
                Ok(vis)
            } else {
                Ok(Visibility::Public)
            }
        } else {
            Ok(Visibility::Private)
        }
    }

    fn parse_fn_item(&mut self) -> CompileResult<ItemKind> {
        let is_const = self.match_token(&RustTokenKind::Const)?;
        let is_unsafe = self.match_token(&RustTokenKind::Unsafe)?;
        let start = self.expect(RustTokenKind::Fn)?.span;

        let name = self.expect_identifier()?;
        self.expect(RustTokenKind::LParen)?;

        let params = self.parse_fn_params()?;
        self.expect(RustTokenKind::RParen)?;

        let return_type = if self.match_token(&RustTokenKind::Arrow)? {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = if self.check(&RustTokenKind::LBrace)? {
            Some(self.parse_block()?)
        } else {
            self.expect(RustTokenKind::Semi)?;
            None
        };

        let end_span = body.as_ref().map(|b| b.span).unwrap_or(start);
        let mut decl = FnDecl::new(name, params, return_type, body, Span::new(start.start, end_span.end));
        decl.is_unsafe = is_unsafe;
        decl.is_const = is_const;

        Ok(ItemKind::Fn(decl))
    }

    fn parse_fn_params(&mut self) -> CompileResult<Vec<FnParam>> {
        let mut params = Vec::new();

        // Handle self parameter
        if self.check(&RustTokenKind::SelfValue)? ||
           self.check(&RustTokenKind::Amp)? ||
           self.check(&RustTokenKind::Mut)? {
            // Skip self param parsing for now - simplified
            if self.match_token(&RustTokenKind::Amp)? {
                self.match_token(&RustTokenKind::Mut)?;
            }
            if self.match_token(&RustTokenKind::Mut)? {
            }
            if self.match_token(&RustTokenKind::SelfValue)? {
                // self parameter handled
                if self.check(&RustTokenKind::Comma)? {
                    self.lexer.next_token()?;
                }
            }
        }

        while !self.check(&RustTokenKind::RParen)? {
            let start = self.lexer.peek()?.span;
            let pattern = self.parse_pattern()?;
            self.expect(RustTokenKind::Colon)?;
            let ty = self.parse_type()?;
            let end = self.lexer.peek()?.span;

            params.push(FnParam::new(pattern, ty, Span::new(start.start, end.start)));

            if !self.match_token(&RustTokenKind::Comma)? {
                break;
            }
        }

        Ok(params)
    }

    fn parse_struct_item(&mut self) -> CompileResult<ItemKind> {
        let start = self.expect(RustTokenKind::Struct)?.span;
        let name = self.expect_identifier()?;

        let kind = if self.match_token(&RustTokenKind::Semi)? {
            StructKind::Unit
        } else if self.match_token(&RustTokenKind::LParen)? {
            let fields = self.parse_tuple_fields()?;
            self.expect(RustTokenKind::RParen)?;
            self.expect(RustTokenKind::Semi)?;
            StructKind::Tuple(fields)
        } else {
            self.expect(RustTokenKind::LBrace)?;
            let fields = self.parse_struct_fields()?;
            self.expect(RustTokenKind::RBrace)?;
            StructKind::Named(fields)
        };

        let end = self.lexer.peek()?.span;
        Ok(ItemKind::Struct(StructDecl {
            name,
            kind,
            span: Span::new(start.start, end.start),
        }))
    }

    fn parse_struct_fields(&mut self) -> CompileResult<Vec<StructField>> {
        let mut fields = Vec::new();

        while !self.check(&RustTokenKind::RBrace)? {
            let vis = self.parse_visibility()?;
            let start = self.lexer.peek()?.span;
            let name = self.expect_identifier()?;
            self.expect(RustTokenKind::Colon)?;
            let ty = self.parse_type()?;
            let end = self.lexer.peek()?.span;

            fields.push(StructField {
                name,
                ty,
                visibility: vis,
                span: Span::new(start.start, end.start),
            });

            if !self.match_token(&RustTokenKind::Comma)? {
                break;
            }
        }

        Ok(fields)
    }

    fn parse_tuple_fields(&mut self) -> CompileResult<Vec<TupleField>> {
        let mut fields = Vec::new();

        while !self.check(&RustTokenKind::RParen)? {
            let vis = self.parse_visibility()?;
            let start = self.lexer.peek()?.span;
            let ty = self.parse_type()?;
            let end = self.lexer.peek()?.span;

            fields.push(TupleField {
                ty,
                visibility: vis,
                span: Span::new(start.start, end.start),
            });

            if !self.match_token(&RustTokenKind::Comma)? {
                break;
            }
        }

        Ok(fields)
    }

    fn parse_enum_item(&mut self) -> CompileResult<ItemKind> {
        let start = self.expect(RustTokenKind::Enum)?.span;
        let name = self.expect_identifier()?;
        self.expect(RustTokenKind::LBrace)?;

        let mut variants = Vec::new();
        while !self.check(&RustTokenKind::RBrace)? {
            let vstart = self.lexer.peek()?.span;
            let vname = self.expect_identifier()?;

            let kind = if self.match_token(&RustTokenKind::LParen)? {
                let fields = self.parse_tuple_fields()?;
                self.expect(RustTokenKind::RParen)?;
                VariantKind::Tuple(fields)
            } else if self.match_token(&RustTokenKind::LBrace)? {
                let fields = self.parse_struct_fields()?;
                self.expect(RustTokenKind::RBrace)?;
                VariantKind::Struct(fields)
            } else {
                VariantKind::Unit
            };

            let discriminant = if self.match_token(&RustTokenKind::Eq)? {
                Some(self.parse_expr()?)
            } else {
                None
            };

            let vend = self.lexer.peek()?.span;
            variants.push(EnumVariant {
                name: vname,
                kind,
                discriminant,
                span: Span::new(vstart.start, vend.start),
            });

            if !self.match_token(&RustTokenKind::Comma)? {
                break;
            }
        }

        let end = self.expect(RustTokenKind::RBrace)?.span;
        Ok(ItemKind::Enum(EnumDecl {
            name,
            variants,
            span: Span::new(start.start, end.end),
        }))
    }

    fn parse_impl_item(&mut self) -> CompileResult<ItemKind> {
        let start = self.expect(RustTokenKind::Impl)?.span;
        let self_ty = self.parse_type()?;
        self.expect(RustTokenKind::LBrace)?;

        let mut items = Vec::new();
        while !self.check(&RustTokenKind::RBrace)? {
            items.push(self.parse_item()?);
        }

        let end = self.expect(RustTokenKind::RBrace)?.span;
        Ok(ItemKind::Impl(ImplDecl {
            self_ty,
            items,
            span: Span::new(start.start, end.end),
        }))
    }

    fn parse_type_alias(&mut self) -> CompileResult<ItemKind> {
        let start = self.expect(RustTokenKind::Type)?.span;
        let name = self.expect_identifier()?;
        self.expect(RustTokenKind::Eq)?;
        let ty = self.parse_type()?;
        let end = self.expect(RustTokenKind::Semi)?.span;

        Ok(ItemKind::TypeAlias(TypeAliasDecl {
            name,
            ty,
            span: Span::new(start.start, end.end),
        }))
    }

    fn parse_const_item(&mut self) -> CompileResult<ItemKind> {
        let start = self.expect(RustTokenKind::Const)?.span;
        let name = self.expect_identifier()?;
        self.expect(RustTokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(RustTokenKind::Eq)?;
        let value = self.parse_expr()?;
        let end = self.expect(RustTokenKind::Semi)?.span;

        Ok(ItemKind::Const(ConstDecl {
            name,
            ty,
            value,
            span: Span::new(start.start, end.end),
        }))
    }

    fn parse_static_item(&mut self) -> CompileResult<ItemKind> {
        let start = self.expect(RustTokenKind::Static)?.span;
        let mutable = self.match_token(&RustTokenKind::Mut)?;
        let name = self.expect_identifier()?;
        self.expect(RustTokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(RustTokenKind::Eq)?;
        let value = self.parse_expr()?;
        let end = self.expect(RustTokenKind::Semi)?.span;

        Ok(ItemKind::Static(StaticDecl {
            name,
            ty,
            value,
            mutable,
            span: Span::new(start.start, end.end),
        }))
    }

    fn parse_mod_item(&mut self) -> CompileResult<ItemKind> {
        let start = self.expect(RustTokenKind::Mod)?.span;
        let name = self.expect_identifier()?;

        let items = if self.match_token(&RustTokenKind::Semi)? {
            None
        } else {
            self.expect(RustTokenKind::LBrace)?;
            let mut items = Vec::new();
            while !self.check(&RustTokenKind::RBrace)? {
                items.push(self.parse_item()?);
            }
            self.expect(RustTokenKind::RBrace)?;
            Some(items)
        };

        let end = self.lexer.peek()?.span;
        Ok(ItemKind::Mod(ModDecl {
            name,
            items,
            span: Span::new(start.start, end.start),
        }))
    }

    fn parse_use_item(&mut self) -> CompileResult<ItemKind> {
        let start = self.expect(RustTokenKind::Use)?.span;
        let tree = self.parse_use_tree()?;
        let end = self.expect(RustTokenKind::Semi)?.span;

        Ok(ItemKind::Use(UseDecl {
            tree,
            span: Span::new(start.start, end.end),
        }))
    }

    fn parse_use_tree(&mut self) -> CompileResult<UseTree> {
        let path = self.parse_path()?;

        if self.match_token(&RustTokenKind::ColonColon)? {
            if self.match_token(&RustTokenKind::Star)? {
                Ok(UseTree::Glob(path))
            } else if self.match_token(&RustTokenKind::LBrace)? {
                let mut trees = Vec::new();
                while !self.check(&RustTokenKind::RBrace)? {
                    trees.push(self.parse_use_tree()?);
                    if !self.match_token(&RustTokenKind::Comma)? {
                        break;
                    }
                }
                self.expect(RustTokenKind::RBrace)?;
                Ok(UseTree::Nested { prefix: path, trees })
            } else {
                let tree = self.parse_use_tree()?;
                Ok(UseTree::Path { prefix: path, tree: Some(Box::new(tree)) })
            }
        } else if self.match_token(&RustTokenKind::As)? {
            let alias = self.expect_identifier()?;
            Ok(UseTree::Rename { path, alias })
        } else {
            Ok(UseTree::Path { prefix: path, tree: None })
        }
    }

    // ==================== Types ====================

    fn parse_type(&mut self) -> CompileResult<RustType> {
        let start = self.lexer.peek()?.span;

        let kind = if self.match_token(&RustTokenKind::LParen)? {
            // Unit or tuple type
            if self.match_token(&RustTokenKind::RParen)? {
                RustTypeKind::Unit
            } else {
                let mut types = vec![self.parse_type()?];
                while self.match_token(&RustTokenKind::Comma)? {
                    if self.check(&RustTokenKind::RParen)? {
                        break;
                    }
                    types.push(self.parse_type()?);
                }
                self.expect(RustTokenKind::RParen)?;
                if types.len() == 1 {
                    return Ok(types.remove(0)); // Parenthesized type
                }
                RustTypeKind::Tuple(types)
            }
        } else if self.match_token(&RustTokenKind::LBracket)? {
            // Array or slice
            let element = self.parse_type()?;
            if self.match_token(&RustTokenKind::Semi)? {
                let size = self.parse_array_size()?;
                self.expect(RustTokenKind::RBracket)?;
                RustTypeKind::Array { element: Box::new(element), size }
            } else {
                self.expect(RustTokenKind::RBracket)?;
                RustTypeKind::Slice { element: Box::new(element) }
            }
        } else if self.match_token(&RustTokenKind::Amp)? {
            // Reference
            let mutable = self.match_token(&RustTokenKind::Mut)?;
            let inner = self.parse_type()?;
            RustTypeKind::Reference { mutable, inner: Box::new(inner) }
        } else if self.match_token(&RustTokenKind::Star)? {
            // Raw pointer
            let mutable = if self.match_token(&RustTokenKind::Mut)? {
                true
            } else {
                self.match_token(&RustTokenKind::Const)?;
                false
            };
            let inner = self.parse_type()?;
            RustTypeKind::Pointer { mutable, inner: Box::new(inner) }
        } else if self.match_token(&RustTokenKind::Bang)? {
            // Never type
            RustTypeKind::Never
        } else if let Some(prim) = self.try_parse_primitive_type()? {
            RustTypeKind::Primitive(prim)
        } else {
            // Named type
            let path = self.parse_path()?;
            RustTypeKind::Named(path)
        };

        let end = self.lexer.peek()?.span;
        Ok(RustType::new(kind, Span::new(start.start, end.start)))
    }

    fn try_parse_primitive_type(&mut self) -> CompileResult<Option<PrimitiveType>> {
        let prim = match &self.lexer.peek()?.kind {
            RustTokenKind::I8 => Some(PrimitiveType::I8),
            RustTokenKind::I16 => Some(PrimitiveType::I16),
            RustTokenKind::I32 => Some(PrimitiveType::I32),
            RustTokenKind::I64 => Some(PrimitiveType::I64),
            RustTokenKind::U8 => Some(PrimitiveType::U8),
            RustTokenKind::U16 => Some(PrimitiveType::U16),
            RustTokenKind::U32 => Some(PrimitiveType::U32),
            RustTokenKind::U64 => Some(PrimitiveType::U64),
            RustTokenKind::Isize => Some(PrimitiveType::Isize),
            RustTokenKind::Usize => Some(PrimitiveType::Usize),
            RustTokenKind::F32 => Some(PrimitiveType::F32),
            RustTokenKind::F64 => Some(PrimitiveType::F64),
            RustTokenKind::Bool => Some(PrimitiveType::Bool),
            RustTokenKind::Char => Some(PrimitiveType::Char),
            _ => None,
        };

        if prim.is_some() {
            self.lexer.next_token()?;
        }

        Ok(prim)
    }

    fn parse_array_size(&mut self) -> CompileResult<usize> {
        let token = self.lexer.next_token()?;
        match &token.kind {
            RustTokenKind::IntLiteral(s) => {
                s.replace('_', "").parse().map_err(|_| {
                    CompileError::parser("invalid array size", token.span)
                })
            }
            _ => Err(CompileError::parser(
                format!("expected array size, found {}", token.kind),
                token.span,
            )),
        }
    }

    fn parse_path(&mut self) -> CompileResult<TypePath> {
        let mut segments = Vec::new();

        // Handle leading ::
        if self.match_token(&RustTokenKind::ColonColon)? {
            segments.push(String::new()); // Root marker
        }

        segments.push(self.expect_identifier()?);

        while self.check(&RustTokenKind::ColonColon)? {
            // Look ahead to see if this is a type path continuation
            // This is simplified - real Rust needs more context
            self.lexer.next_token()?; // consume ::
            if let Ok(name) = self.expect_identifier() {
                segments.push(name);
            } else {
                break;
            }
        }

        Ok(TypePath::new(segments))
    }

    // ==================== Statements ====================

    fn parse_block(&mut self) -> CompileResult<Block> {
        let start = self.expect(RustTokenKind::LBrace)?.span;
        let mut stmts = Vec::new();
        let mut expr = None;

        while !self.check(&RustTokenKind::RBrace)? {
            if self.check(&RustTokenKind::Let)? {
                stmts.push(self.parse_let_stmt()?);
            } else if self.is_item_start()? {
                let item = self.parse_item()?;
                stmts.push(Stmt::new(StmtKind::Item(item), start));
            } else {
                let e = self.parse_expr()?;

                if self.check(&RustTokenKind::RBrace)? {
                    // Trailing expression without semicolon
                    expr = Some(e);
                } else if self.match_token(&RustTokenKind::Semi)? {
                    stmts.push(Stmt::new(StmtKind::Expr(e), start));
                } else if self.is_block_expr(&e) {
                    // Block expressions don't need semicolons
                    if self.check(&RustTokenKind::RBrace)? {
                        expr = Some(e);
                    } else {
                        stmts.push(Stmt::new(StmtKind::Expr(e), start));
                    }
                } else {
                    // Missing semicolon - treat as expression
                    if self.check(&RustTokenKind::RBrace)? {
                        expr = Some(e);
                    } else {
                        return Err(CompileError::parser(
                            "expected `;` after expression",
                            e.span,
                        ));
                    }
                }
            }
        }

        let end = self.expect(RustTokenKind::RBrace)?.span;
        Ok(Block::new(stmts, expr, Span::new(start.start, end.end)))
    }

    fn is_block_expr(&self, expr: &Expr) -> bool {
        matches!(
            expr.kind,
            ExprKind::Block(_) | ExprKind::If { .. } | ExprKind::Loop { .. } |
            ExprKind::While { .. } | ExprKind::For { .. } | ExprKind::Match { .. } |
            ExprKind::Unsafe(_)
        )
    }

    fn is_item_start(&mut self) -> CompileResult<bool> {
        Ok(matches!(
            &self.lexer.peek()?.kind,
            RustTokenKind::Fn | RustTokenKind::Struct | RustTokenKind::Enum |
            RustTokenKind::Impl | RustTokenKind::Type | RustTokenKind::Const |
            RustTokenKind::Static | RustTokenKind::Mod | RustTokenKind::Use |
            RustTokenKind::Pub | RustTokenKind::Unsafe
        ))
    }

    fn parse_let_stmt(&mut self) -> CompileResult<Stmt> {
        let start = self.expect(RustTokenKind::Let)?.span;
        let pattern = self.parse_pattern()?;

        let ty = if self.match_token(&RustTokenKind::Colon)? {
            Some(self.parse_type()?)
        } else {
            None
        };

        let init = if self.match_token(&RustTokenKind::Eq)? {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let end = self.expect(RustTokenKind::Semi)?.span;

        Ok(Stmt::new(
            StmtKind::Let { pattern, ty, init },
            Span::new(start.start, end.end),
        ))
    }

    // ==================== Patterns ====================

    fn parse_pattern(&mut self) -> CompileResult<Pattern> {
        let start = self.lexer.peek()?.span;

        // Handle or-patterns at top level
        let mut patterns = vec![self.parse_pattern_atom()?];

        while self.match_token(&RustTokenKind::Pipe)? {
            patterns.push(self.parse_pattern_atom()?);
        }

        if patterns.len() == 1 {
            Ok(patterns.remove(0))
        } else {
            let end = self.lexer.peek()?.span;
            Ok(Pattern::new(
                PatternKind::Or(patterns),
                Span::new(start.start, end.start),
            ))
        }
    }

    fn parse_pattern_atom(&mut self) -> CompileResult<Pattern> {
        let start = self.lexer.peek()?.span;

        let kind = if self.check(&RustTokenKind::Identifier(String::new()))? {
            let name = self.expect_identifier()?;
            if name == "_" {
                PatternKind::Wildcard
            } else {
                PatternKind::Binding {
                    name,
                    mutable: false,
                    subpattern: None,
                }
            }
        } else if self.match_token(&RustTokenKind::Mut)? {
            let name = self.expect_identifier()?;
            PatternKind::Binding {
                name,
                mutable: true,
                subpattern: None,
            }
        } else if self.match_token(&RustTokenKind::Ref)? {
            let mutable = self.match_token(&RustTokenKind::Mut)?;
            let inner = self.parse_pattern_atom()?;
            PatternKind::Reference {
                mutable,
                pattern: Box::new(inner),
            }
        } else if self.match_token(&RustTokenKind::Amp)? {
            let mutable = self.match_token(&RustTokenKind::Mut)?;
            let inner = self.parse_pattern_atom()?;
            PatternKind::Reference {
                mutable,
                pattern: Box::new(inner),
            }
        } else if self.match_token(&RustTokenKind::LParen)? {
            // Tuple pattern
            let mut patterns = Vec::new();
            if !self.check(&RustTokenKind::RParen)? {
                patterns.push(self.parse_pattern()?);
                while self.match_token(&RustTokenKind::Comma)? {
                    if self.check(&RustTokenKind::RParen)? {
                        break;
                    }
                    patterns.push(self.parse_pattern()?);
                }
            }
            self.expect(RustTokenKind::RParen)?;

            if patterns.len() == 1 {
                PatternKind::Paren(Box::new(patterns.remove(0)))
            } else {
                PatternKind::Tuple(patterns)
            }
        } else if self.match_token(&RustTokenKind::LBracket)? {
            // Slice pattern
            let mut patterns = Vec::new();
            while !self.check(&RustTokenKind::RBracket)? {
                patterns.push(self.parse_pattern()?);
                if !self.match_token(&RustTokenKind::Comma)? {
                    break;
                }
            }
            self.expect(RustTokenKind::RBracket)?;
            PatternKind::Slice(patterns)
        } else if self.match_token(&RustTokenKind::DotDot)? {
            PatternKind::Rest
        } else if self.check(&RustTokenKind::IntLiteral(String::new()))? ||
                  self.check(&RustTokenKind::True)? ||
                  self.check(&RustTokenKind::False)? ||
                  self.check(&RustTokenKind::CharLiteral(String::new()))? ||
                  self.check(&RustTokenKind::StringLiteral(String::new()))? {
            let lit = self.parse_literal_expr()?;
            PatternKind::Literal(Box::new(lit))
        } else {
            let token = self.lexer.next_token()?;
            return Err(CompileError::parser(
                format!("expected pattern, found {}", token.kind),
                token.span,
            ));
        };

        let end = self.lexer.peek()?.span;
        Ok(Pattern::new(kind, Span::new(start.start, end.start)))
    }

    // ==================== Expressions ====================

    fn parse_expr(&mut self) -> CompileResult<Expr> {
        self.parse_expr_with_precedence(0)
    }

    fn parse_expr_with_precedence(&mut self, min_prec: u8) -> CompileResult<Expr> {
        let mut left = self.parse_unary_expr()?;

        while let Some(op) = self.peek_binary_op()? {
            let prec = op.precedence();
            if prec < min_prec {
                break;
            }

            self.lexer.next_token()?; // consume operator
            let right = self.parse_expr_with_precedence(prec + 1)?;

            let span = Span::new(left.span.start, right.span.end);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        // Handle assignment
        if let Some(assign_op) = self.peek_assign_op()? {
            self.lexer.next_token()?;
            let value = self.parse_expr()?;
            let span = Span::new(left.span.start, value.span.end);
            left = Expr::new(
                ExprKind::Assign {
                    target: Box::new(left),
                    op: assign_op,
                    value: Box::new(value),
                },
                span,
            );
        }

        Ok(left)
    }

    fn peek_binary_op(&mut self) -> CompileResult<Option<BinOp>> {
        Ok(match &self.lexer.peek()?.kind {
            RustTokenKind::Plus => Some(BinOp::Add),
            RustTokenKind::Minus => Some(BinOp::Sub),
            RustTokenKind::Star => Some(BinOp::Mul),
            RustTokenKind::Slash => Some(BinOp::Div),
            RustTokenKind::Percent => Some(BinOp::Rem),
            RustTokenKind::Amp => Some(BinOp::BitAnd),
            RustTokenKind::Pipe => Some(BinOp::BitOr),
            RustTokenKind::Caret => Some(BinOp::BitXor),
            RustTokenKind::Shl => Some(BinOp::Shl),
            RustTokenKind::Shr => Some(BinOp::Shr),
            RustTokenKind::AmpAmp => Some(BinOp::And),
            RustTokenKind::PipePipe => Some(BinOp::Or),
            RustTokenKind::EqEq => Some(BinOp::Eq),
            RustTokenKind::NotEq => Some(BinOp::Ne),
            RustTokenKind::Lt => Some(BinOp::Lt),
            RustTokenKind::LtEq => Some(BinOp::Le),
            RustTokenKind::Gt => Some(BinOp::Gt),
            RustTokenKind::GtEq => Some(BinOp::Ge),
            _ => None,
        })
    }

    fn peek_assign_op(&mut self) -> CompileResult<Option<Option<BinOp>>> {
        Ok(match &self.lexer.peek()?.kind {
            RustTokenKind::Eq => Some(None),
            RustTokenKind::PlusEq => Some(Some(BinOp::Add)),
            RustTokenKind::MinusEq => Some(Some(BinOp::Sub)),
            RustTokenKind::StarEq => Some(Some(BinOp::Mul)),
            RustTokenKind::SlashEq => Some(Some(BinOp::Div)),
            RustTokenKind::PercentEq => Some(Some(BinOp::Rem)),
            RustTokenKind::AmpEq => Some(Some(BinOp::BitAnd)),
            RustTokenKind::PipeEq => Some(Some(BinOp::BitOr)),
            RustTokenKind::CaretEq => Some(Some(BinOp::BitXor)),
            RustTokenKind::ShlEq => Some(Some(BinOp::Shl)),
            RustTokenKind::ShrEq => Some(Some(BinOp::Shr)),
            _ => None,
        })
    }

    fn parse_unary_expr(&mut self) -> CompileResult<Expr> {
        let start = self.lexer.peek()?.span;

        if self.match_token(&RustTokenKind::Minus)? {
            let operand = self.parse_unary_expr()?;
            let span = Span::new(start.start, operand.span.end);
            return Ok(Expr::new(
                ExprKind::Unary { op: UnaryOp::Neg, operand: Box::new(operand) },
                span,
            ));
        }

        if self.match_token(&RustTokenKind::Bang)? {
            let operand = self.parse_unary_expr()?;
            let span = Span::new(start.start, operand.span.end);
            return Ok(Expr::new(
                ExprKind::Unary { op: UnaryOp::Not, operand: Box::new(operand) },
                span,
            ));
        }

        if self.match_token(&RustTokenKind::Star)? {
            let operand = self.parse_unary_expr()?;
            let span = Span::new(start.start, operand.span.end);
            return Ok(Expr::new(ExprKind::Dereference(Box::new(operand)), span));
        }

        if self.match_token(&RustTokenKind::Amp)? {
            let mutable = self.match_token(&RustTokenKind::Mut)?;
            let operand = self.parse_unary_expr()?;
            let span = Span::new(start.start, operand.span.end);
            return Ok(Expr::new(
                ExprKind::Reference { mutable, operand: Box::new(operand) },
                span,
            ));
        }

        self.parse_postfix_expr()
    }

    fn parse_postfix_expr(&mut self) -> CompileResult<Expr> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            if self.match_token(&RustTokenKind::Dot)? {
                // Field access or method call
                let field = self.expect_identifier()?;

                if self.match_token(&RustTokenKind::LParen)? {
                    // Method call
                    let args = self.parse_call_args()?;
                    self.expect(RustTokenKind::RParen)?;
                    let span = Span::new(expr.span.start, self.lexer.peek()?.span.start);
                    expr = Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(expr),
                            method: field,
                            args,
                        },
                        span,
                    );
                } else {
                    // Field access
                    let span = Span::new(expr.span.start, self.lexer.peek()?.span.start);
                    expr = Expr::new(
                        ExprKind::Field {
                            object: Box::new(expr),
                            field,
                        },
                        span,
                    );
                }
            } else if self.match_token(&RustTokenKind::LBracket)? {
                // Index
                let index = self.parse_expr()?;
                self.expect(RustTokenKind::RBracket)?;
                let span = Span::new(expr.span.start, self.lexer.peek()?.span.start);
                expr = Expr::new(
                    ExprKind::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                    },
                    span,
                );
            } else if self.match_token(&RustTokenKind::LParen)? {
                // Function call
                let args = self.parse_call_args()?;
                self.expect(RustTokenKind::RParen)?;
                let span = Span::new(expr.span.start, self.lexer.peek()?.span.start);
                expr = Expr::new(
                    ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                    span,
                );
            } else if self.match_token(&RustTokenKind::As)? {
                // Type cast
                let ty = self.parse_type()?;
                let span = Span::new(expr.span.start, ty.span.end);
                expr = Expr::new(
                    ExprKind::Cast {
                        expr: Box::new(expr),
                        ty,
                    },
                    span,
                );
            } else if self.match_token(&RustTokenKind::Question)? {
                // ? operator - desugar to match (simplified)
                let span = Span::new(expr.span.start, self.lexer.peek()?.span.start);
                // For now, just wrap in a special form
                // Real implementation would desugar to Try trait
                expr = Expr::new(
                    ExprKind::MethodCall {
                        receiver: Box::new(expr),
                        method: "try_unwrap".to_string(),
                        args: vec![],
                    },
                    span,
                );
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_call_args(&mut self) -> CompileResult<Vec<Expr>> {
        let mut args = Vec::new();

        if !self.check(&RustTokenKind::RParen)? {
            args.push(self.parse_expr()?);
            while self.match_token(&RustTokenKind::Comma)? {
                if self.check(&RustTokenKind::RParen)? {
                    break;
                }
                args.push(self.parse_expr()?);
            }
        }

        Ok(args)
    }

    fn parse_primary_expr(&mut self) -> CompileResult<Expr> {
        let start = self.lexer.peek()?.span;

        // Literals
        if self.check(&RustTokenKind::IntLiteral(String::new()))? ||
           self.check(&RustTokenKind::HexLiteral(String::new()))? ||
           self.check(&RustTokenKind::OctalLiteral(String::new()))? ||
           self.check(&RustTokenKind::BinaryLiteral(String::new()))? ||
           self.check(&RustTokenKind::FloatLiteral(String::new()))? ||
           self.check(&RustTokenKind::True)? ||
           self.check(&RustTokenKind::False)? ||
           self.check(&RustTokenKind::CharLiteral(String::new()))? ||
           self.check(&RustTokenKind::StringLiteral(String::new()))? ||
           self.check(&RustTokenKind::ByteLiteral(String::new()))? ||
           self.check(&RustTokenKind::ByteStringLiteral(String::new()))? {
            return self.parse_literal_expr();
        }

        // Control flow expressions
        if self.check(&RustTokenKind::If)? {
            return self.parse_if_expr();
        }
        if self.check(&RustTokenKind::Loop)? {
            return self.parse_loop_expr();
        }
        if self.check(&RustTokenKind::While)? {
            return self.parse_while_expr();
        }
        if self.check(&RustTokenKind::For)? {
            return self.parse_for_expr();
        }
        if self.check(&RustTokenKind::Match)? {
            return self.parse_match_expr();
        }
        if self.check(&RustTokenKind::Unsafe)? {
            return self.parse_unsafe_expr();
        }

        // Break, continue, return
        if self.match_token(&RustTokenKind::Break)? {
            let label = if self.check(&RustTokenKind::Lifetime(String::new()))? {
                Some(self.parse_lifetime()?)
            } else {
                None
            };
            let value = if !self.check(&RustTokenKind::Semi)? &&
                         !self.check(&RustTokenKind::RBrace)? &&
                         !self.check(&RustTokenKind::Comma)? {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            let end = self.lexer.peek()?.span;
            return Ok(Expr::new(
                ExprKind::Break { label, value },
                Span::new(start.start, end.start),
            ));
        }

        if self.match_token(&RustTokenKind::Continue)? {
            let label = if self.check(&RustTokenKind::Lifetime(String::new()))? {
                Some(self.parse_lifetime()?)
            } else {
                None
            };
            let end = self.lexer.peek()?.span;
            return Ok(Expr::new(
                ExprKind::Continue { label },
                Span::new(start.start, end.start),
            ));
        }

        if self.match_token(&RustTokenKind::Return)? {
            let value = if !self.check(&RustTokenKind::Semi)? &&
                         !self.check(&RustTokenKind::RBrace)? {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            let end = self.lexer.peek()?.span;
            return Ok(Expr::new(
                ExprKind::Return(value),
                Span::new(start.start, end.start),
            ));
        }

        // Block
        if self.check(&RustTokenKind::LBrace)? {
            let block = self.parse_block()?;
            let span = block.span;
            return Ok(Expr::new(ExprKind::Block(block), span));
        }

        // Tuple or parenthesized
        if self.match_token(&RustTokenKind::LParen)? {
            if self.match_token(&RustTokenKind::RParen)? {
                // Unit
                let end = self.lexer.peek()?.span;
                return Ok(Expr::new(
                    ExprKind::Tuple(vec![]),
                    Span::new(start.start, end.start),
                ));
            }

            let first = self.parse_expr()?;

            if self.match_token(&RustTokenKind::Comma)? {
                // Tuple
                let mut exprs = vec![first];
                while !self.check(&RustTokenKind::RParen)? {
                    exprs.push(self.parse_expr()?);
                    if !self.match_token(&RustTokenKind::Comma)? {
                        break;
                    }
                }
                let end = self.expect(RustTokenKind::RParen)?.span;
                return Ok(Expr::new(
                    ExprKind::Tuple(exprs),
                    Span::new(start.start, end.end),
                ));
            }

            // Parenthesized expression
            self.expect(RustTokenKind::RParen)?;
            let end = self.lexer.peek()?.span;
            return Ok(Expr::new(
                ExprKind::Paren(Box::new(first)),
                Span::new(start.start, end.start),
            ));
        }

        // Array
        if self.match_token(&RustTokenKind::LBracket)? {
            if self.match_token(&RustTokenKind::RBracket)? {
                let end = self.lexer.peek()?.span;
                return Ok(Expr::new(
                    ExprKind::Array(vec![]),
                    Span::new(start.start, end.start),
                ));
            }

            let first = self.parse_expr()?;

            if self.match_token(&RustTokenKind::Semi)? {
                // Array repeat: [value; count]
                let count = self.parse_expr()?;
                let end = self.expect(RustTokenKind::RBracket)?.span;
                return Ok(Expr::new(
                    ExprKind::ArrayRepeat {
                        value: Box::new(first),
                        count: Box::new(count),
                    },
                    Span::new(start.start, end.end),
                ));
            }

            // Array literal
            let mut exprs = vec![first];
            while self.match_token(&RustTokenKind::Comma)? {
                if self.check(&RustTokenKind::RBracket)? {
                    break;
                }
                exprs.push(self.parse_expr()?);
            }
            let end = self.expect(RustTokenKind::RBracket)?.span;
            return Ok(Expr::new(
                ExprKind::Array(exprs),
                Span::new(start.start, end.end),
            ));
        }

        // Range expressions starting with ..
        if self.match_token(&RustTokenKind::DotDot)? {
            let inclusive = self.match_token(&RustTokenKind::Eq)?;
            let end_expr = if !self.check(&RustTokenKind::Semi)? &&
                             !self.check(&RustTokenKind::RBrace)? &&
                             !self.check(&RustTokenKind::Comma)? &&
                             !self.check(&RustTokenKind::RParen)? &&
                             !self.check(&RustTokenKind::RBracket)? {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            let end = self.lexer.peek()?.span;
            return Ok(Expr::new(
                ExprKind::Range { start: None, end: end_expr, inclusive },
                Span::new(start.start, end.start),
            ));
        }

        // Identifier or path
        if self.check(&RustTokenKind::Identifier(String::new()))? ||
           self.check(&RustTokenKind::SelfValue)? ||
           self.check(&RustTokenKind::Super)? ||
           self.check(&RustTokenKind::Crate)? ||
           self.check(&RustTokenKind::ColonColon)? {
            let path = self.parse_path()?;

            // Check for struct literal
            if self.check(&RustTokenKind::LBrace)? && !self.is_block_context()? {
                return self.parse_struct_expr(path);
            }

            let end = self.lexer.peek()?.span;
            if path.segments.len() == 1 {
                return Ok(Expr::new(
                    ExprKind::Identifier(path.segments[0].clone()),
                    Span::new(start.start, end.start),
                ));
            }
            return Ok(Expr::new(
                ExprKind::Path(path),
                Span::new(start.start, end.start),
            ));
        }

        let token = self.lexer.next_token()?;
        Err(CompileError::parser(
            format!("expected expression, found {}", token.kind),
            token.span,
        ))
    }

    /// Parse an expression without allowing struct literals
    /// This is used in contexts where { would be ambiguous (e.g., match scrutinee)
    fn parse_expr_without_struct(&mut self) -> CompileResult<Expr> {
        self.struct_literal_allowed = false;
        let result = self.parse_expr();
        self.struct_literal_allowed = true;
        result
    }

    fn is_block_context(&self) -> CompileResult<bool> {
        // Returns true when struct literals are forbidden
        Ok(!self.struct_literal_allowed)
    }

    fn parse_literal_expr(&mut self) -> CompileResult<Expr> {
        let token = self.lexer.next_token()?;
        let span = token.span;

        let kind = match token.kind {
            RustTokenKind::IntLiteral(s) |
            RustTokenKind::HexLiteral(s) |
            RustTokenKind::OctalLiteral(s) |
            RustTokenKind::BinaryLiteral(s) => {
                let value = self.parse_int_literal(&s)?;
                ExprKind::IntLiteral(value)
            }
            RustTokenKind::FloatLiteral(s) => {
                let value = s.replace('_', "").trim_end_matches(|c| c == 'f' || c == 'F')
                    .parse().map_err(|_| CompileError::parser("invalid float literal", span))?;
                ExprKind::FloatLiteral(value)
            }
            RustTokenKind::True => ExprKind::BoolLiteral(true),
            RustTokenKind::False => ExprKind::BoolLiteral(false),
            RustTokenKind::CharLiteral(s) => {
                let c = self.parse_char_literal(&s)?;
                ExprKind::CharLiteral(c)
            }
            RustTokenKind::StringLiteral(s) => {
                let content = self.parse_string_literal(&s)?;
                ExprKind::StringLiteral(content)
            }
            RustTokenKind::ByteLiteral(s) => {
                let b = self.parse_byte_literal(&s)?;
                ExprKind::ByteLiteral(b)
            }
            RustTokenKind::ByteStringLiteral(s) => {
                let bytes = self.parse_byte_string_literal(&s)?;
                ExprKind::ByteStringLiteral(bytes)
            }
            _ => return Err(CompileError::parser(
                format!("expected literal, found {}", token.kind),
                span,
            )),
        };

        Ok(Expr::new(kind, span))
    }

    /// Strip integer type suffix from a literal (i8, i16, i32, i64, isize, u8, u16, u32, u64, usize)
    fn strip_int_suffix(s: &str) -> &str {
        // List of valid integer suffixes, ordered longest first
        const SUFFIXES: &[&str] = &[
            "isize", "usize",
            "i128", "u128",
            "i64", "u64",
            "i32", "u32",
            "i16", "u16",
            "i8", "u8",
        ];

        for suffix in SUFFIXES {
            if s.ends_with(suffix) {
                return &s[..s.len() - suffix.len()];
            }
        }
        s
    }

    fn parse_int_literal(&self, s: &str) -> CompileResult<i64> {
        let s = s.replace('_', "");

        // Strip type suffix properly
        let s = Self::strip_int_suffix(&s);

        if s.starts_with("0x") || s.starts_with("0X") {
            i64::from_str_radix(&s[2..], 16)
                .map_err(|_| CompileError::parser("invalid hex literal", Span::default()))
        } else if s.starts_with("0o") || s.starts_with("0O") {
            i64::from_str_radix(&s[2..], 8)
                .map_err(|_| CompileError::parser("invalid octal literal", Span::default()))
        } else if s.starts_with("0b") || s.starts_with("0B") {
            i64::from_str_radix(&s[2..], 2)
                .map_err(|_| CompileError::parser("invalid binary literal", Span::default()))
        } else {
            s.parse()
                .map_err(|_| CompileError::parser("invalid integer literal", Span::default()))
        }
    }

    fn parse_char_literal(&self, s: &str) -> CompileResult<char> {
        let inner = &s[1..s.len()-1]; // Remove quotes
        self.parse_escape_char(inner)
    }

    fn parse_escape_char(&self, s: &str) -> CompileResult<char> {
        let mut chars = s.chars();
        match chars.next() {
            Some('\\') => match chars.next() {
                Some('n') => Ok('\n'),
                Some('r') => Ok('\r'),
                Some('t') => Ok('\t'),
                Some('\\') => Ok('\\'),
                Some('\'') => Ok('\''),
                Some('"') => Ok('"'),
                Some('0') => Ok('\0'),
                Some('x') => {
                    let hex: String = chars.take(2).collect();
                    let code = u8::from_str_radix(&hex, 16)
                        .map_err(|_| CompileError::parser("invalid hex escape", Span::default()))?;
                    Ok(code as char)
                }
                _ => Err(CompileError::parser("invalid escape sequence", Span::default())),
            },
            Some(c) => Ok(c),
            None => Err(CompileError::parser("empty character literal", Span::default())),
        }
    }

    fn parse_string_literal(&self, s: &str) -> CompileResult<String> {
        let inner = &s[1..s.len()-1]; // Remove quotes
        let mut result = String::new();
        let mut chars = inner.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('r') => result.push('\r'),
                    Some('t') => result.push('\t'),
                    Some('\\') => result.push('\\'),
                    Some('\'') => result.push('\''),
                    Some('"') => result.push('"'),
                    Some('0') => result.push('\0'),
                    Some('x') => {
                        let hex: String = chars.by_ref().take(2).collect();
                        let code = u8::from_str_radix(&hex, 16)
                            .map_err(|_| CompileError::parser("invalid hex escape", Span::default()))?;
                        result.push(code as char);
                    }
                    _ => return Err(CompileError::parser("invalid escape sequence", Span::default())),
                }
            } else {
                result.push(c);
            }
        }

        Ok(result)
    }

    fn parse_byte_literal(&self, s: &str) -> CompileResult<u8> {
        let inner = &s[2..s.len()-1]; // Remove b' and '
        let c = self.parse_escape_char(inner)?;
        if c as u32 > 255 {
            return Err(CompileError::parser("byte literal out of range", Span::default()));
        }
        Ok(c as u8)
    }

    fn parse_byte_string_literal(&self, s: &str) -> CompileResult<Vec<u8>> {
        let inner = &s[2..s.len()-1]; // Remove b" and "
        let string = self.parse_string_literal(&format!("\"{}\"", inner))?;
        Ok(string.into_bytes())
    }

    fn parse_lifetime(&mut self) -> CompileResult<String> {
        let token = self.lexer.next_token()?;
        match token.kind {
            RustTokenKind::Lifetime(name) => Ok(name),
            _ => Err(CompileError::parser(
                format!("expected lifetime, found {}", token.kind),
                token.span,
            )),
        }
    }

    fn parse_if_expr(&mut self) -> CompileResult<Expr> {
        let start = self.expect(RustTokenKind::If)?.span;
        // Use restricted expression parser to avoid ambiguity with struct literals
        let condition = self.parse_expr_without_struct()?;
        let then_block = self.parse_block()?;

        let else_block = if self.match_token(&RustTokenKind::Else)? {
            if self.check(&RustTokenKind::If)? {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                let block = self.parse_block()?;
                let span = block.span;
                Some(Box::new(Expr::new(ExprKind::Block(block), span)))
            }
        } else {
            None
        };

        let end = self.lexer.peek()?.span;
        Ok(Expr::new(
            ExprKind::If {
                condition: Box::new(condition),
                then_block,
                else_block,
            },
            Span::new(start.start, end.start),
        ))
    }

    fn parse_loop_expr(&mut self) -> CompileResult<Expr> {
        let start = self.expect(RustTokenKind::Loop)?.span;
        let body = self.parse_block()?;
        let end = body.span;

        Ok(Expr::new(
            ExprKind::Loop { label: None, body },
            Span::new(start.start, end.end),
        ))
    }

    fn parse_while_expr(&mut self) -> CompileResult<Expr> {
        let start = self.expect(RustTokenKind::While)?.span;
        // Use restricted expression parser to avoid ambiguity with struct literals
        let condition = self.parse_expr_without_struct()?;
        let body = self.parse_block()?;
        let end = body.span;

        Ok(Expr::new(
            ExprKind::While {
                label: None,
                condition: Box::new(condition),
                body,
            },
            Span::new(start.start, end.end),
        ))
    }

    fn parse_for_expr(&mut self) -> CompileResult<Expr> {
        let start = self.expect(RustTokenKind::For)?.span;
        let pattern = self.parse_pattern()?;
        self.expect(RustTokenKind::In)?;
        // Use restricted expression parser to avoid ambiguity with struct literals
        let iter = self.parse_expr_without_struct()?;
        let body = self.parse_block()?;
        let end = body.span;

        Ok(Expr::new(
            ExprKind::For {
                label: None,
                pattern,
                iter: Box::new(iter),
                body,
            },
            Span::new(start.start, end.end),
        ))
    }

    fn parse_match_expr(&mut self) -> CompileResult<Expr> {
        let start = self.expect(RustTokenKind::Match)?.span;
        // Use restricted expression parser to avoid ambiguity with struct literals
        let scrutinee = self.parse_expr_without_struct()?;
        self.expect(RustTokenKind::LBrace)?;

        let mut arms = Vec::new();
        while !self.check(&RustTokenKind::RBrace)? {
            let arm_start = self.lexer.peek()?.span;
            let pattern = self.parse_pattern()?;

            let guard = if self.match_token(&RustTokenKind::If)? {
                Some(self.parse_expr()?)
            } else {
                None
            };

            self.expect(RustTokenKind::FatArrow)?;
            let body = self.parse_expr()?;

            let arm_end = self.lexer.peek()?.span;
            arms.push(MatchArm {
                pattern,
                guard,
                body,
                span: Span::new(arm_start.start, arm_end.start),
            });

            if !self.match_token(&RustTokenKind::Comma)? {
                break;
            }
        }

        let end = self.expect(RustTokenKind::RBrace)?.span;
        Ok(Expr::new(
            ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            Span::new(start.start, end.end),
        ))
    }

    fn parse_unsafe_expr(&mut self) -> CompileResult<Expr> {
        let start = self.expect(RustTokenKind::Unsafe)?.span;
        let block = self.parse_block()?;
        let end = block.span;

        Ok(Expr::new(ExprKind::Unsafe(block), Span::new(start.start, end.end)))
    }

    fn parse_struct_expr(&mut self, path: TypePath) -> CompileResult<Expr> {
        let start_span = self.lexer.peek()?.span;
        self.expect(RustTokenKind::LBrace)?;

        let mut fields = Vec::new();
        let mut rest = None;

        while !self.check(&RustTokenKind::RBrace)? {
            if self.match_token(&RustTokenKind::DotDot)? {
                rest = Some(Box::new(self.parse_expr()?));
                break;
            }

            let field_start = self.lexer.peek()?.span;
            let name = self.expect_identifier()?;

            let value = if self.match_token(&RustTokenKind::Colon)? {
                Some(self.parse_expr()?)
            } else {
                None
            };

            let field_end = self.lexer.peek()?.span;
            fields.push(FieldInit {
                name,
                value,
                span: Span::new(field_start.start, field_end.start),
            });

            if !self.match_token(&RustTokenKind::Comma)? {
                break;
            }
        }

        let end = self.expect(RustTokenKind::RBrace)?.span;
        Ok(Expr::new(
            ExprKind::Struct { path, fields, rest },
            Span::new(start_span.start, end.end),
        ))
    }

    // ==================== Helpers ====================

    fn check(&mut self, expected: &RustTokenKind) -> CompileResult<bool> {
        self.lexer.check(expected)
    }

    /// Check if current position is `const fn` (lookahead)
    fn check_const_fn(&mut self) -> CompileResult<bool> {
        if !self.check(&RustTokenKind::Const)? {
            return Ok(false);
        }
        // Look ahead to see if next token is 'fn'
        self.lexer.check_lookahead(&RustTokenKind::Fn)
    }

    fn match_token(&mut self, expected: &RustTokenKind) -> CompileResult<bool> {
        self.lexer.match_token(expected)
    }

    fn expect(&mut self, expected: RustTokenKind) -> CompileResult<RustToken> {
        self.lexer.expect(expected)
    }

    fn expect_identifier(&mut self) -> CompileResult<String> {
        let token = self.lexer.next_token()?;
        match token.kind {
            RustTokenKind::Identifier(name) => Ok(name),
            RustTokenKind::SelfValue => Ok("self".to_string()),
            _ => Err(CompileError::parser(
                format!("expected identifier, found {}", token.kind),
                token.span,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function() {
        let source = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let mut parser = RustParser::new(source);
        let module = parser.parse_module().unwrap();

        assert_eq!(module.items.len(), 1);
        match &module.items[0].kind {
            ItemKind::Fn(f) => {
                assert_eq!(f.name, "add");
                assert_eq!(f.params.len(), 2);
                assert!(f.body.is_some());
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_struct() {
        let source = "struct Point { x: i32, y: i32 }";
        let mut parser = RustParser::new(source);
        let module = parser.parse_module().unwrap();

        assert_eq!(module.items.len(), 1);
        match &module.items[0].kind {
            ItemKind::Struct(s) => {
                assert_eq!(s.name, "Point");
                match &s.kind {
                    StructKind::Named(fields) => {
                        assert_eq!(fields.len(), 2);
                    }
                    _ => panic!("expected named struct"),
                }
            }
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn test_parse_enum() {
        let source = "enum Option { None, Some(i32) }";
        let mut parser = RustParser::new(source);
        let module = parser.parse_module().unwrap();

        assert_eq!(module.items.len(), 1);
        match &module.items[0].kind {
            ItemKind::Enum(e) => {
                assert_eq!(e.name, "Option");
                assert_eq!(e.variants.len(), 2);
            }
            _ => panic!("expected enum"),
        }
    }

    #[test]
    fn test_parse_let() {
        let source = "fn main() { let x: i32 = 5; let y = 10; }";
        let mut parser = RustParser::new(source);
        let module = parser.parse_module().unwrap();

        assert_eq!(module.items.len(), 1);
        match &module.items[0].kind {
            ItemKind::Fn(f) => {
                let body = f.body.as_ref().unwrap();
                assert_eq!(body.stmts.len(), 2);
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_if_else() {
        let source = "fn main() { if x > 0 { 1 } else { 0 } }";
        let mut parser = RustParser::new(source);
        let module = parser.parse_module().unwrap();

        assert_eq!(module.items.len(), 1);
    }

    #[test]
    fn test_parse_match() {
        let source = r#"
            fn main() {
                match x {
                    0 => true,
                    _ => false,
                }
            }
        "#;
        let mut parser = RustParser::new(source);
        let module = parser.parse_module().unwrap();

        assert_eq!(module.items.len(), 1);
    }

    #[test]
    fn test_parse_impl() {
        let source = r#"
            impl Point {
                fn new(x: i32, y: i32) -> Point {
                    Point { x, y }
                }
            }
        "#;
        let mut parser = RustParser::new(source);
        let module = parser.parse_module().unwrap();

        assert_eq!(module.items.len(), 1);
        match &module.items[0].kind {
            ItemKind::Impl(i) => {
                assert_eq!(i.items.len(), 1);
            }
            _ => panic!("expected impl"),
        }
    }
}
