//! Recursive descent parser for C

use crate::frontend::c::ast::*;
use crate::common::{CompileError, CompileResult, Span};
use crate::frontend::c::lexer::{Lexer, Token, TokenKind};

/// Recursive descent parser for C
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
}

impl<'a> Parser<'a> {
    /// Create a new parser for the given source
    pub fn new(source: &'a str) -> CompileResult<Self> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token()?;
        Ok(Self { lexer, current })
    }

    /// Parse a complete translation unit
    pub fn parse(&mut self) -> CompileResult<TranslationUnit> {
        let mut declarations = Vec::new();

        while !self.at_end() {
            declarations.push(self.parse_external_declaration()?);
        }

        Ok(TranslationUnit::new(declarations))
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    fn at_end(&self) -> bool {
        matches!(self.current.kind, TokenKind::Eof)
    }

    fn advance(&mut self) -> CompileResult<Token> {
        let prev = std::mem::replace(&mut self.current, self.lexer.next_token()?);
        Ok(prev)
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.current.kind) == std::mem::discriminant(kind)
    }

    fn match_token(&mut self, kind: &TokenKind) -> CompileResult<bool> {
        if self.check(kind) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn expect(&mut self, kind: TokenKind) -> CompileResult<Token> {
        if self.check(&kind) {
            self.advance()
        } else {
            Err(CompileError::parser(
                format!("expected {}, found {}", kind, self.current.kind),
                self.current.span,
            ))
        }
    }

    #[allow(dead_code)]
    fn peek(&self) -> &TokenKind {
        &self.current.kind
    }

    // =========================================================================
    // External declarations (top-level)
    // =========================================================================

    fn parse_external_declaration(&mut self) -> CompileResult<Declaration> {
        let start_span = self.current.span;

        // Parse declaration specifiers
        let (storage_class, base_type) = self.parse_declaration_specifiers()?;

        // Check for struct/union/enum without declarator
        if self.check(&TokenKind::Semi) {
            self.advance()?;
            let span = start_span.merge(self.current.span);
            // This is a struct/union/enum definition only
            return match base_type.kind {
                TypeKind::Struct { name, members } => Ok(Declaration::new(
                    DeclKind::Struct(StructDecl::new(
                        name,
                        Some(members.into_iter().map(|(n, t)| StructMember::new(n, t, span)).collect()),
                        span,
                    )),
                    span,
                )),
                TypeKind::Union { name, members } => Ok(Declaration::new(
                    DeclKind::Union(UnionDecl::new(
                        name,
                        Some(members.into_iter().map(|(n, t)| StructMember::new(n, t, span)).collect()),
                        span,
                    )),
                    span,
                )),
                TypeKind::Enum { name, variants } => Ok(Declaration::new(
                    DeclKind::Enum(EnumDecl::new(
                        name,
                        Some(variants.into_iter().map(|(n, v)| {
                            EnumVariant::new(n, v.map(|val| Expr::new(ExprKind::IntLiteral(val), span)), span)
                        }).collect()),
                        span,
                    )),
                    span,
                )),
                _ => Err(CompileError::parser("expected declaration", span)),
            };
        }

        // Parse declarator
        let (name, ty) = self.parse_declarator(base_type)?;

        // Check if this is a function definition or declaration
        if self.check(&TokenKind::LBrace) {
            // Function definition (ANSI style)
            let body = self.parse_block()?;
            let span = start_span.merge(body.span);

            if let TypeKind::Function { return_type, params, variadic } = ty.kind {
                let params: Vec<ParamDecl> = params
                    .into_iter()
                    .map(|(name, ty)| ParamDecl::new(name, ty, span))
                    .collect();

                let mut func = FuncDecl::new(name, *return_type, params, span)
                    .with_body(body)
                    .with_variadic(variadic);

                if let Some(sc) = storage_class {
                    func = func.with_storage_class(sc);
                }

                Ok(Declaration::new(DeclKind::Function(func), span))
            } else {
                Err(CompileError::parser(
                    "expected function type for function definition",
                    span,
                ))
            }
        } else if self.check(&TokenKind::Semi) || self.check(&TokenKind::Comma) || self.check(&TokenKind::Eq) {
            // Variable declaration or function declaration
            self.parse_declaration_rest(name, ty, storage_class, start_span)
        } else if let TypeKind::Function { return_type, params, variadic } = &ty.kind {
            // K&R-style function definition - parameter declarations between ) and {
            // Parse K&R parameter declarations
            let mut kr_params = params.clone();

            while self.current.kind.can_start_declaration() && !self.check(&TokenKind::LBrace) {
                // Parse a parameter type declaration (e.g., "int a;")
                let (_, param_base_type) = self.parse_declaration_specifiers()?;

                // Parse declarator(s) - may be multiple like "int a, b;"
                loop {
                    let (param_name, param_type) = self.parse_declarator(param_base_type.clone())?;

                    // Find and update the corresponding parameter
                    for (name, param_ty) in &mut kr_params {
                        if name.as_ref() == Some(&param_name) {
                            *param_ty = param_type.clone();
                            break;
                        }
                    }

                    if !self.match_token(&TokenKind::Comma)? {
                        break;
                    }
                }

                self.expect(TokenKind::Semi)?;
            }

            // Now parse the function body
            let body = self.parse_block()?;
            let span = start_span.merge(body.span);

            let params: Vec<ParamDecl> = kr_params
                .into_iter()
                .map(|(name, ty)| ParamDecl::new(name, ty, span))
                .collect();

            let mut func = FuncDecl::new(name, *return_type.clone(), params, span)
                .with_body(body)
                .with_variadic(*variadic);

            if let Some(sc) = storage_class {
                func = func.with_storage_class(sc);
            }

            Ok(Declaration::new(DeclKind::Function(func), span))
        } else {
            Err(CompileError::parser(
                format!("unexpected token {} in declaration", self.current.kind),
                self.current.span,
            ))
        }
    }

    fn parse_declaration_rest(
        &mut self,
        first_name: String,
        first_type: CType,
        storage_class: Option<StorageClass>,
        start_span: Span,
    ) -> CompileResult<Declaration> {
        // Check for function declaration
        if let TypeKind::Function { return_type, params, variadic } = first_type.kind {
            self.expect(TokenKind::Semi)?;
            let span = start_span.merge(self.current.span);

            let params: Vec<ParamDecl> = params
                .into_iter()
                .map(|(name, ty)| ParamDecl::new(name, ty, span))
                .collect();

            let mut func = FuncDecl::new(first_name, *return_type, params, span)
                .with_variadic(variadic);

            if let Some(sc) = storage_class {
                func = func.with_storage_class(sc);
            }

            return Ok(Declaration::new(DeclKind::Function(func), span));
        }

        // Variable declaration - may have multiple declarators
        let init = if self.match_token(&TokenKind::Eq)? {
            Some(self.parse_initializer()?)
        } else {
            None
        };

        let mut var = VarDecl::new(first_name, first_type.clone(), start_span);
        if let Some(sc) = storage_class.clone() {
            var = var.with_storage_class(sc);
        }
        if let Some(init) = init {
            var = var.with_init(init);
        }

        let mut declarations = vec![var];

        // Handle multiple declarators (e.g., int a, b, c;)
        while self.match_token(&TokenKind::Comma)? {
            // Get base type without pointers/arrays (need to extract from first_type)
            let base_type = self.get_base_type(&first_type);
            let (name, ty) = self.parse_declarator(base_type)?;

            let init = if self.match_token(&TokenKind::Eq)? {
                Some(self.parse_initializer()?)
            } else {
                None
            };

            let mut var = VarDecl::new(name, ty, start_span);
            if let Some(sc) = storage_class.clone() {
                var = var.with_storage_class(sc);
            }
            if let Some(init) = init {
                var = var.with_init(init);
            }
            declarations.push(var);
        }

        self.expect(TokenKind::Semi)?;
        let span = start_span.merge(self.current.span);

        // If single declaration, return as before
        if declarations.len() == 1 {
            let mut var = declarations.remove(0);
            var.span = span;
            return Ok(Declaration::new(DeclKind::Variable(var), span));
        }

        // Multiple declarations - return as MultipleVariables
        for var in &mut declarations {
            var.span = span;
        }
        Ok(Declaration::new(DeclKind::MultipleVariables(declarations), span))
    }

    /// Extract base type (strip pointers and arrays from first declarator for subsequent declarators)
    fn get_base_type(&self, ty: &CType) -> CType {
        // For multiple declarators, we need the original base type
        // The base type is stored in the type before any pointer/array modifications
        // We reconstruct by looking at the innermost type
        match &ty.kind {
            TypeKind::Pointer(inner) => self.get_base_type(inner),
            TypeKind::Array { element, .. } => self.get_base_type(element),
            _ => ty.clone(),
        }
    }

    // =========================================================================
    // Declaration specifiers
    // =========================================================================

    fn parse_declaration_specifiers(&mut self) -> CompileResult<(Option<StorageClass>, CType)> {
        let start_span = self.current.span;
        let mut storage_class = None;
        let mut type_specs = Vec::new();
        let mut qualifiers = TypeQualifiers::default();
        let mut signed: Option<bool> = None;

        loop {
            match &self.current.kind {
                // Storage class
                TokenKind::Typedef => {
                    self.advance()?;
                    storage_class = Some(StorageClass::Typedef);
                }
                TokenKind::Extern => {
                    self.advance()?;
                    storage_class = Some(StorageClass::Extern);
                }
                TokenKind::Static => {
                    self.advance()?;
                    storage_class = Some(StorageClass::Static);
                }
                TokenKind::Auto => {
                    self.advance()?;
                    storage_class = Some(StorageClass::Auto);
                }
                TokenKind::Register => {
                    self.advance()?;
                    storage_class = Some(StorageClass::Register);
                }

                // Type qualifiers
                TokenKind::Const => {
                    self.advance()?;
                    qualifiers.is_const = true;
                }
                TokenKind::Volatile => {
                    self.advance()?;
                    qualifiers.is_volatile = true;
                }
                TokenKind::Restrict => {
                    self.advance()?;
                    qualifiers.is_restrict = true;
                }

                // Sign specifiers
                TokenKind::Signed => {
                    self.advance()?;
                    signed = Some(true);
                }
                TokenKind::Unsigned => {
                    self.advance()?;
                    signed = Some(false);
                }

                // Type specifiers
                TokenKind::Void => {
                    self.advance()?;
                    type_specs.push("void");
                }
                TokenKind::Char => {
                    self.advance()?;
                    type_specs.push("char");
                }
                TokenKind::Short => {
                    self.advance()?;
                    type_specs.push("short");
                }
                TokenKind::Int => {
                    self.advance()?;
                    type_specs.push("int");
                }
                TokenKind::Long => {
                    self.advance()?;
                    type_specs.push("long");
                }
                TokenKind::Float => {
                    self.advance()?;
                    type_specs.push("float");
                }
                TokenKind::Double => {
                    self.advance()?;
                    type_specs.push("double");
                }
                TokenKind::Bool => {
                    self.advance()?;
                    type_specs.push("_Bool");
                }

                // Struct/union/enum
                TokenKind::Struct => {
                    self.advance()?;
                    let ty = self.parse_struct_or_union(true)?;
                    let span = start_span.merge(self.current.span);
                    return Ok((storage_class, CType::new(ty, span).with_qualifiers(qualifiers)));
                }
                TokenKind::Union => {
                    self.advance()?;
                    let ty = self.parse_struct_or_union(false)?;
                    let span = start_span.merge(self.current.span);
                    return Ok((storage_class, CType::new(ty, span).with_qualifiers(qualifiers)));
                }
                TokenKind::Enum => {
                    self.advance()?;
                    let ty = self.parse_enum()?;
                    let span = start_span.merge(self.current.span);
                    return Ok((storage_class, CType::new(ty, span).with_qualifiers(qualifiers)));
                }

                // Typedef name (identifier that is a type)
                TokenKind::Identifier(_) if type_specs.is_empty() && signed.is_none() => {
                    // Could be a typedef name, but we don't track those yet
                    // For now, break out and let it be parsed as a declarator
                    break;
                }

                _ => break,
            }
        }

        // Build type from specifiers
        let kind = self.type_from_specifiers(&type_specs, signed)?;
        let span = start_span.merge(self.current.span);

        Ok((storage_class, CType::new(kind, span).with_qualifiers(qualifiers)))
    }

    fn type_from_specifiers(&self, specs: &[&str], signed: Option<bool>) -> CompileResult<TypeKind> {
        let signed = signed.unwrap_or(true);

        match specs.as_ref() {
            [] | ["int"] => Ok(TypeKind::Int { signed }),
            ["void"] => Ok(TypeKind::Void),
            ["char"] => Ok(TypeKind::Char { signed }),
            ["short"] | ["short", "int"] => Ok(TypeKind::Short { signed }),
            ["long"] | ["long", "int"] => Ok(TypeKind::Long { signed }),
            ["long", "long"] | ["long", "long", "int"] => Ok(TypeKind::LongLong { signed }),
            ["float"] => Ok(TypeKind::Float),
            ["double"] => Ok(TypeKind::Double),
            ["long", "double"] => Ok(TypeKind::Double), // Treat as double
            ["_Bool"] => Ok(TypeKind::Char { signed: false }), // _Bool as unsigned char
            _ => Err(CompileError::parser(
                format!("invalid type specifier combination: {:?}", specs),
                self.current.span,
            )),
        }
    }

    fn parse_struct_or_union(&mut self, is_struct: bool) -> CompileResult<TypeKind> {
        let name = if let TokenKind::Identifier(name) = &self.current.kind {
            let n = name.clone();
            self.advance()?;
            Some(n)
        } else {
            None
        };

        let members = if self.check(&TokenKind::LBrace) {
            self.advance()?;
            let mut members = Vec::new();

            while !self.check(&TokenKind::RBrace) {
                let (_, base_type) = self.parse_declaration_specifiers()?;
                let (member_name, member_type) = self.parse_declarator(base_type)?;
                members.push((member_name, member_type));
                self.expect(TokenKind::Semi)?;
            }

            self.expect(TokenKind::RBrace)?;
            members
        } else if name.is_none() {
            return Err(CompileError::parser(
                "expected struct/union name or body",
                self.current.span,
            ));
        } else {
            Vec::new() // Forward declaration
        };

        if is_struct {
            Ok(TypeKind::Struct { name, members })
        } else {
            Ok(TypeKind::Union { name, members })
        }
    }

    fn parse_enum(&mut self) -> CompileResult<TypeKind> {
        let name = if let TokenKind::Identifier(name) = &self.current.kind {
            let n = name.clone();
            self.advance()?;
            Some(n)
        } else {
            None
        };

        let variants = if self.check(&TokenKind::LBrace) {
            self.advance()?;
            let mut variants = Vec::new();
            let mut next_value: i64 = 0;

            while !self.check(&TokenKind::RBrace) {
                let variant_name = if let TokenKind::Identifier(name) = &self.current.kind {
                    let n = name.clone();
                    self.advance()?;
                    n
                } else {
                    return Err(CompileError::parser(
                        "expected enum variant name",
                        self.current.span,
                    ));
                };

                let value = if self.match_token(&TokenKind::Eq)? {
                    let expr = self.parse_constant_expression()?;
                    // TODO: Evaluate constant expression
                    if let ExprKind::IntLiteral(v) = expr.kind {
                        next_value = v;
                    }
                    Some(next_value)
                } else {
                    Some(next_value)
                };

                variants.push((variant_name, value));
                next_value += 1;

                if !self.check(&TokenKind::RBrace) {
                    self.expect(TokenKind::Comma)?;
                    // Allow trailing comma
                    if self.check(&TokenKind::RBrace) {
                        break;
                    }
                }
            }

            self.expect(TokenKind::RBrace)?;
            variants
        } else if name.is_none() {
            return Err(CompileError::parser(
                "expected enum name or body",
                self.current.span,
            ));
        } else {
            Vec::new() // Forward declaration
        };

        Ok(TypeKind::Enum { name, variants })
    }

    // =========================================================================
    // Declarators
    // =========================================================================

    fn parse_declarator(&mut self, base_type: CType) -> CompileResult<(String, CType)> {
        // Handle pointer prefix
        let mut ty = base_type;
        while self.match_token(&TokenKind::Star)? {
            let mut ptr_qualifiers = TypeQualifiers::default();
            loop {
                match &self.current.kind {
                    TokenKind::Const => {
                        self.advance()?;
                        ptr_qualifiers.is_const = true;
                    }
                    TokenKind::Volatile => {
                        self.advance()?;
                        ptr_qualifiers.is_volatile = true;
                    }
                    TokenKind::Restrict => {
                        self.advance()?;
                        ptr_qualifiers.is_restrict = true;
                    }
                    _ => break,
                }
            }
            ty = CType::pointer_to(ty, self.current.span).with_qualifiers(ptr_qualifiers);
        }

        // Parse direct declarator
        self.parse_direct_declarator(ty)
    }

    fn parse_direct_declarator(&mut self, base_type: CType) -> CompileResult<(String, CType)> {
        // Get the name
        let name = if let TokenKind::Identifier(name) = &self.current.kind {
            let n = name.clone();
            self.advance()?;
            n
        } else if self.check(&TokenKind::LParen) {
            // Parenthesized declarator
            self.advance()?;
            let (name, inner_type) = self.parse_declarator(base_type.clone())?;
            self.expect(TokenKind::RParen)?;
            // Continue parsing suffix
            let ty = self.parse_declarator_suffix(inner_type)?;
            return Ok((name, ty));
        } else {
            return Err(CompileError::parser(
                format!("expected identifier in declarator, found {}", self.current.kind),
                self.current.span,
            ));
        };

        // Parse suffix (array or function)
        let ty = self.parse_declarator_suffix(base_type)?;
        Ok((name, ty))
    }

    fn parse_declarator_suffix(&mut self, base_type: CType) -> CompileResult<CType> {
        let span = self.current.span;

        if self.match_token(&TokenKind::LBracket)? {
            // Array
            let size = if self.check(&TokenKind::RBracket) {
                None
            } else {
                let expr = self.parse_constant_expression()?;
                if let ExprKind::IntLiteral(n) = expr.kind {
                    Some(n as usize)
                } else {
                    None // VLA or unsupported
                }
            };
            self.expect(TokenKind::RBracket)?;

            // Recursively parse more suffixes
            let element_type = self.parse_declarator_suffix(base_type)?;

            Ok(CType::new(
                TypeKind::Array {
                    element: Box::new(element_type),
                    size,
                },
                span,
            ))
        } else if self.match_token(&TokenKind::LParen)? {
            // Function
            let (params, variadic) = self.parse_parameter_list()?;
            self.expect(TokenKind::RParen)?;

            Ok(CType::new(
                TypeKind::Function {
                    return_type: Box::new(base_type),
                    params,
                    variadic,
                },
                span,
            ))
        } else {
            Ok(base_type)
        }
    }

    fn parse_parameter_list(&mut self) -> CompileResult<(Vec<(Option<String>, CType)>, bool)> {
        let mut params = Vec::new();
        let mut variadic = false;

        if self.check(&TokenKind::RParen) {
            return Ok((params, variadic));
        }

        // Check for (void)
        if self.check(&TokenKind::Void) {
            let next = self.lexer.peek()?;
            if matches!(next.kind, TokenKind::RParen) {
                self.advance()?; // consume void
                return Ok((params, variadic));
            }
        }

        loop {
            if self.match_token(&TokenKind::Ellipsis)? {
                variadic = true;
                break;
            }

            let (_, base_type) = self.parse_declaration_specifiers()?;

            // Try to parse declarator (might be abstract)
            let (name, ty) = if self.check(&TokenKind::Star)
                || matches!(self.current.kind, TokenKind::Identifier(_))
                || self.check(&TokenKind::LParen)
                || self.check(&TokenKind::LBracket)
            {
                let (name, ty) = self.parse_declarator(base_type)?;
                (Some(name), ty)
            } else {
                (None, base_type)
            };

            params.push((name, ty));

            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }

        Ok((params, variadic))
    }

    // =========================================================================
    // Statements
    // =========================================================================

    fn parse_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;

        match &self.current.kind {
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Stmt::new(StmtKind::Block(block), start_span.merge(self.current.span)))
            }

            TokenKind::If => self.parse_if_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::Do => self.parse_do_while_statement(),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::Switch => self.parse_switch_statement(),
            TokenKind::Case => self.parse_case_statement(),
            TokenKind::Default => self.parse_default_statement(),
            TokenKind::Break => self.parse_break_statement(),
            TokenKind::Continue => self.parse_continue_statement(),
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::Goto => self.parse_goto_statement(),

            TokenKind::Semi => {
                self.advance()?;
                Ok(Stmt::new(StmtKind::Empty, start_span))
            }

            // Check for labeled statement
            TokenKind::Identifier(_) => {
                // Peek ahead to see if this is a label
                let name = if let TokenKind::Identifier(n) = &self.current.kind {
                    n.clone()
                } else {
                    unreachable!()
                };

                let next = self.lexer.peek()?;
                if matches!(next.kind, TokenKind::Colon) {
                    self.advance()?; // identifier
                    self.advance()?; // colon
                    let stmt = self.parse_statement()?;
                    let span = start_span.merge(stmt.span);
                    return Ok(Stmt::new(
                        StmtKind::Label {
                            name,
                            stmt: Box::new(stmt),
                        },
                        span,
                    ));
                }

                // Otherwise, expression statement
                self.parse_expression_statement()
            }

            // Declaration in block
            _ if self.current.kind.can_start_declaration() => {
                let decl = self.parse_block_declaration()?;
                let span = decl.span;
                Ok(Stmt::new(StmtKind::Declaration(decl), span))
            }

            // Expression statement
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_block(&mut self) -> CompileResult<Block> {
        let start_span = self.current.span;
        self.expect(TokenKind::LBrace)?;

        let mut items = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.at_end() {
            if self.current.kind.can_start_declaration() {
                let decl = self.parse_block_declaration()?;
                items.push(BlockItem::Declaration(decl));
            } else {
                let stmt = self.parse_statement()?;
                items.push(BlockItem::Statement(stmt));
            }
        }

        self.expect(TokenKind::RBrace)?;
        let span = start_span.merge(self.current.span);

        Ok(Block::new(items, span))
    }

    fn parse_block_declaration(&mut self) -> CompileResult<Declaration> {
        let start_span = self.current.span;
        let (storage_class, base_type) = self.parse_declaration_specifiers()?;
        let (name, ty) = self.parse_declarator(base_type)?;

        let init = if self.match_token(&TokenKind::Eq)? {
            Some(self.parse_initializer()?)
        } else {
            None
        };

        self.expect(TokenKind::Semi)?;
        let span = start_span.merge(self.current.span);

        let mut var = VarDecl::new(name, ty, span);
        if let Some(sc) = storage_class {
            var = var.with_storage_class(sc);
        }
        if let Some(init) = init {
            var = var.with_init(init);
        }

        Ok(Declaration::new(DeclKind::Variable(var), span))
    }

    fn parse_if_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::If)?;
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression()?;
        self.expect(TokenKind::RParen)?;

        let then_branch = Box::new(self.parse_statement()?);

        let else_branch = if self.match_token(&TokenKind::Else)? {
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };

        let span = start_span.merge(self.current.span);
        Ok(Stmt::new(
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            },
            span,
        ))
    }

    fn parse_while_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::While)?;
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression()?;
        self.expect(TokenKind::RParen)?;

        let body = Box::new(self.parse_statement()?);
        let span = start_span.merge(self.current.span);

        Ok(Stmt::new(StmtKind::While { condition, body }, span))
    }

    fn parse_do_while_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::Do)?;

        let body = Box::new(self.parse_statement()?);

        self.expect(TokenKind::While)?;
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression()?;
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::Semi)?;

        let span = start_span.merge(self.current.span);
        Ok(Stmt::new(StmtKind::DoWhile { body, condition }, span))
    }

    fn parse_for_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::For)?;
        self.expect(TokenKind::LParen)?;

        // Init
        let init = if self.check(&TokenKind::Semi) {
            self.advance()?;
            None
        } else if self.current.kind.can_start_declaration() {
            let decl = self.parse_block_declaration()?;
            Some(ForInit::Declaration(decl))
        } else {
            let expr = self.parse_expression()?;
            self.expect(TokenKind::Semi)?;
            Some(ForInit::Expr(expr))
        };

        // Condition
        let condition = if self.check(&TokenKind::Semi) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(TokenKind::Semi)?;

        // Update
        let update = if self.check(&TokenKind::RParen) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(TokenKind::RParen)?;

        let body = Box::new(self.parse_statement()?);
        let span = start_span.merge(self.current.span);

        Ok(Stmt::new(
            StmtKind::For {
                init,
                condition,
                update,
                body,
            },
            span,
        ))
    }

    fn parse_switch_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::Switch)?;
        self.expect(TokenKind::LParen)?;
        let expr = self.parse_expression()?;
        self.expect(TokenKind::RParen)?;

        let body = Box::new(self.parse_statement()?);
        let span = start_span.merge(self.current.span);

        Ok(Stmt::new(StmtKind::Switch { expr, body }, span))
    }

    fn parse_case_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::Case)?;
        let value = self.parse_constant_expression()?;
        self.expect(TokenKind::Colon)?;
        let stmt = Box::new(self.parse_statement()?);
        let span = start_span.merge(self.current.span);

        Ok(Stmt::new(StmtKind::Case { value, stmt }, span))
    }

    fn parse_default_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::Default)?;
        self.expect(TokenKind::Colon)?;
        let stmt = Box::new(self.parse_statement()?);
        let span = start_span.merge(self.current.span);

        Ok(Stmt::new(StmtKind::Default(stmt), span))
    }

    fn parse_break_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::Break)?;
        self.expect(TokenKind::Semi)?;
        Ok(Stmt::new(StmtKind::Break, start_span.merge(self.current.span)))
    }

    fn parse_continue_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::Continue)?;
        self.expect(TokenKind::Semi)?;
        Ok(Stmt::new(StmtKind::Continue, start_span.merge(self.current.span)))
    }

    fn parse_return_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::Return)?;

        let value = if self.check(&TokenKind::Semi) {
            None
        } else {
            Some(self.parse_expression()?)
        };

        self.expect(TokenKind::Semi)?;
        let span = start_span.merge(self.current.span);

        Ok(Stmt::new(StmtKind::Return(value), span))
    }

    fn parse_goto_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        self.expect(TokenKind::Goto)?;

        let label = if let TokenKind::Identifier(name) = &self.current.kind {
            let n = name.clone();
            self.advance()?;
            n
        } else {
            return Err(CompileError::parser("expected label name", self.current.span));
        };

        self.expect(TokenKind::Semi)?;
        let span = start_span.merge(self.current.span);

        Ok(Stmt::new(StmtKind::Goto(label), span))
    }

    fn parse_expression_statement(&mut self) -> CompileResult<Stmt> {
        let start_span = self.current.span;
        let expr = self.parse_expression()?;
        self.expect(TokenKind::Semi)?;
        let span = start_span.merge(self.current.span);

        Ok(Stmt::new(StmtKind::Expr(expr), span))
    }

    // =========================================================================
    // Expressions
    // =========================================================================

    fn parse_expression(&mut self) -> CompileResult<Expr> {
        self.parse_assignment_expression()
    }

    fn parse_constant_expression(&mut self) -> CompileResult<Expr> {
        self.parse_conditional_expression()
    }

    fn parse_assignment_expression(&mut self) -> CompileResult<Expr> {
        let start_span = self.current.span;
        let left = self.parse_conditional_expression()?;

        if let Some(op) = self.get_assignment_op() {
            self.advance()?;
            let right = self.parse_assignment_expression()?;
            let span = start_span.merge(right.span);

            return Ok(Expr::new(
                ExprKind::Assign {
                    op,
                    target: Box::new(left),
                    value: Box::new(right),
                },
                span,
            ));
        }

        Ok(left)
    }

    fn get_assignment_op(&self) -> Option<AssignOp> {
        match &self.current.kind {
            TokenKind::Eq => Some(AssignOp::Assign),
            TokenKind::PlusEq => Some(AssignOp::AddAssign),
            TokenKind::MinusEq => Some(AssignOp::SubAssign),
            TokenKind::StarEq => Some(AssignOp::MulAssign),
            TokenKind::SlashEq => Some(AssignOp::DivAssign),
            TokenKind::PercentEq => Some(AssignOp::ModAssign),
            TokenKind::AmpEq => Some(AssignOp::AndAssign),
            TokenKind::PipeEq => Some(AssignOp::OrAssign),
            TokenKind::CaretEq => Some(AssignOp::XorAssign),
            TokenKind::LtLtEq => Some(AssignOp::ShlAssign),
            TokenKind::GtGtEq => Some(AssignOp::ShrAssign),
            _ => None,
        }
    }

    fn parse_conditional_expression(&mut self) -> CompileResult<Expr> {
        let start_span = self.current.span;
        let condition = self.parse_logical_or_expression()?;

        if self.match_token(&TokenKind::Question)? {
            let then_expr = self.parse_expression()?;
            self.expect(TokenKind::Colon)?;
            let else_expr = self.parse_conditional_expression()?;
            let span = start_span.merge(else_expr.span);

            return Ok(Expr::new(
                ExprKind::Ternary {
                    condition: Box::new(condition),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                },
                span,
            ));
        }

        Ok(condition)
    }

    fn parse_logical_or_expression(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_logical_and_expression()?;

        while self.match_token(&TokenKind::PipePipe)? {
            let right = self.parse_logical_and_expression()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op: BinaryOp::LogOr,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    fn parse_logical_and_expression(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_bitwise_or_expression()?;

        while self.match_token(&TokenKind::AmpAmp)? {
            let right = self.parse_bitwise_or_expression()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op: BinaryOp::LogAnd,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    fn parse_bitwise_or_expression(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_bitwise_xor_expression()?;

        while self.match_token(&TokenKind::Pipe)? {
            let right = self.parse_bitwise_xor_expression()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op: BinaryOp::BitOr,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    fn parse_bitwise_xor_expression(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_bitwise_and_expression()?;

        while self.match_token(&TokenKind::Caret)? {
            let right = self.parse_bitwise_and_expression()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op: BinaryOp::BitXor,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    fn parse_bitwise_and_expression(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_equality_expression()?;

        while self.match_token(&TokenKind::Amp)? {
            let right = self.parse_equality_expression()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op: BinaryOp::BitAnd,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    fn parse_equality_expression(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_relational_expression()?;

        loop {
            let op = match &self.current.kind {
                TokenKind::EqEq => BinaryOp::Eq,
                TokenKind::NotEq => BinaryOp::Ne,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_relational_expression()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    fn parse_relational_expression(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_shift_expression()?;

        loop {
            let op = match &self.current.kind {
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::LtEq => BinaryOp::Le,
                TokenKind::GtEq => BinaryOp::Ge,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_shift_expression()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    fn parse_shift_expression(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_additive_expression()?;

        loop {
            let op = match &self.current.kind {
                TokenKind::LtLt => BinaryOp::Shl,
                TokenKind::GtGt => BinaryOp::Shr,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_additive_expression()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    fn parse_additive_expression(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_multiplicative_expression()?;

        loop {
            let op = match &self.current.kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_multiplicative_expression()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    fn parse_multiplicative_expression(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_unary_expression()?;

        loop {
            let op = match &self.current.kind {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_unary_expression()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        Ok(left)
    }

    fn parse_unary_expression(&mut self) -> CompileResult<Expr> {
        let start_span = self.current.span;

        match &self.current.kind {
            TokenKind::PlusPlus => {
                self.advance()?;
                let operand = self.parse_unary_expression()?;
                let span = start_span.merge(operand.span);
                Ok(Expr::new(ExprKind::PreIncrement(Box::new(operand)), span))
            }
            TokenKind::MinusMinus => {
                self.advance()?;
                let operand = self.parse_unary_expression()?;
                let span = start_span.merge(operand.span);
                Ok(Expr::new(ExprKind::PreDecrement(Box::new(operand)), span))
            }
            TokenKind::Amp => {
                self.advance()?;
                let operand = self.parse_unary_expression()?;
                let span = start_span.merge(operand.span);
                Ok(Expr::new(ExprKind::AddrOf(Box::new(operand)), span))
            }
            TokenKind::Star => {
                self.advance()?;
                let operand = self.parse_unary_expression()?;
                let span = start_span.merge(operand.span);
                Ok(Expr::new(ExprKind::Deref(Box::new(operand)), span))
            }
            TokenKind::Plus => {
                self.advance()?;
                // Unary + is a no-op
                self.parse_unary_expression()
            }
            TokenKind::Minus => {
                self.advance()?;
                let operand = self.parse_unary_expression()?;
                let span = start_span.merge(operand.span);
                Ok(Expr::new(
                    ExprKind::Unary {
                        op: UnaryOp::Neg,
                        operand: Box::new(operand),
                    },
                    span,
                ))
            }
            TokenKind::Bang => {
                self.advance()?;
                let operand = self.parse_unary_expression()?;
                let span = start_span.merge(operand.span);
                Ok(Expr::new(
                    ExprKind::Unary {
                        op: UnaryOp::Not,
                        operand: Box::new(operand),
                    },
                    span,
                ))
            }
            TokenKind::Tilde => {
                self.advance()?;
                let operand = self.parse_unary_expression()?;
                let span = start_span.merge(operand.span);
                Ok(Expr::new(
                    ExprKind::Unary {
                        op: UnaryOp::BitNot,
                        operand: Box::new(operand),
                    },
                    span,
                ))
            }
            TokenKind::Sizeof => {
                self.advance()?;
                if self.check(&TokenKind::LParen) {
                    // Could be sizeof(type) or sizeof(expr)
                    self.advance()?;
                    // Try to parse as type first
                    if self.current.kind.can_start_declaration() {
                        let (_, ty) = self.parse_declaration_specifiers()?;
                        // Handle abstract declarator
                        let ty = if self.check(&TokenKind::Star) || self.check(&TokenKind::LBracket) {
                            let (_, ty) = self.parse_declarator(ty)?;
                            ty
                        } else {
                            ty
                        };
                        self.expect(TokenKind::RParen)?;
                        let span = start_span.merge(self.current.span);
                        return Ok(Expr::new(ExprKind::Sizeof(SizeofArg::Type(ty)), span));
                    } else {
                        let expr = self.parse_expression()?;
                        self.expect(TokenKind::RParen)?;
                        let span = start_span.merge(self.current.span);
                        return Ok(Expr::new(ExprKind::Sizeof(SizeofArg::Expr(Box::new(expr))), span));
                    }
                } else {
                    let operand = self.parse_unary_expression()?;
                    let span = start_span.merge(operand.span);
                    Ok(Expr::new(ExprKind::Sizeof(SizeofArg::Expr(Box::new(operand))), span))
                }
            }
            _ => self.parse_postfix_expression(),
        }
    }

    fn parse_postfix_expression(&mut self) -> CompileResult<Expr> {
        let mut expr = self.parse_primary_expression()?;

        loop {
            let start_span = expr.span;
            match &self.current.kind {
                TokenKind::LBracket => {
                    self.advance()?;
                    let index = self.parse_expression()?;
                    self.expect(TokenKind::RBracket)?;
                    let span = start_span.merge(self.current.span);
                    expr = Expr::new(
                        ExprKind::Index {
                            array: Box::new(expr),
                            index: Box::new(index),
                        },
                        span,
                    );
                }
                TokenKind::LParen => {
                    self.advance()?;
                    let args = self.parse_argument_list()?;
                    self.expect(TokenKind::RParen)?;
                    let span = start_span.merge(self.current.span);
                    expr = Expr::new(
                        ExprKind::Call {
                            callee: Box::new(expr),
                            args,
                        },
                        span,
                    );
                }
                TokenKind::Dot => {
                    self.advance()?;
                    let field = if let TokenKind::Identifier(name) = &self.current.kind {
                        let n = name.clone();
                        self.advance()?;
                        n
                    } else {
                        return Err(CompileError::parser("expected field name", self.current.span));
                    };
                    let span = start_span.merge(self.current.span);
                    expr = Expr::new(
                        ExprKind::Member {
                            object: Box::new(expr),
                            field,
                        },
                        span,
                    );
                }
                TokenKind::Arrow => {
                    self.advance()?;
                    let field = if let TokenKind::Identifier(name) = &self.current.kind {
                        let n = name.clone();
                        self.advance()?;
                        n
                    } else {
                        return Err(CompileError::parser("expected field name", self.current.span));
                    };
                    let span = start_span.merge(self.current.span);
                    expr = Expr::new(
                        ExprKind::PtrMember {
                            pointer: Box::new(expr),
                            field,
                        },
                        span,
                    );
                }
                TokenKind::PlusPlus => {
                    self.advance()?;
                    let span = start_span.merge(self.current.span);
                    expr = Expr::new(ExprKind::PostIncrement(Box::new(expr)), span);
                }
                TokenKind::MinusMinus => {
                    self.advance()?;
                    let span = start_span.merge(self.current.span);
                    expr = Expr::new(ExprKind::PostDecrement(Box::new(expr)), span);
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_argument_list(&mut self) -> CompileResult<Vec<Expr>> {
        let mut args = Vec::new();

        if self.check(&TokenKind::RParen) {
            return Ok(args);
        }

        loop {
            args.push(self.parse_assignment_expression()?);
            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }

        Ok(args)
    }

    fn parse_primary_expression(&mut self) -> CompileResult<Expr> {
        let span = self.current.span;

        match &self.current.kind {
            TokenKind::IntLiteral(s) => {
                let value = self.parse_int_literal(s)?;
                self.advance()?;
                Ok(Expr::new(ExprKind::IntLiteral(value), span))
            }
            TokenKind::HexLiteral(s) => {
                let value = self.parse_hex_literal(s)?;
                self.advance()?;
                Ok(Expr::new(ExprKind::IntLiteral(value), span))
            }
            TokenKind::OctalLiteral(s) => {
                let value = self.parse_octal_literal(s)?;
                self.advance()?;
                Ok(Expr::new(ExprKind::IntLiteral(value), span))
            }
            TokenKind::BinaryLiteral(s) => {
                let value = self.parse_binary_literal(s)?;
                self.advance()?;
                Ok(Expr::new(ExprKind::IntLiteral(value), span))
            }
            TokenKind::FloatLiteral(s) => {
                let s = s.clone();
                self.advance()?;
                let value: f64 = s.trim_end_matches(['f', 'F', 'l', 'L']).parse().unwrap_or(0.0);
                Ok(Expr::new(ExprKind::FloatLiteral(value), span))
            }
            TokenKind::CharLiteral(s) => {
                let s = s.clone();
                self.advance()?;
                let c = self.parse_char_literal(&s)?;
                Ok(Expr::new(ExprKind::CharLiteral(c), span))
            }
            TokenKind::StringLiteral(s) => {
                let s = s.clone();
                self.advance()?;
                let value = self.parse_string_literal(&s)?;
                Ok(Expr::new(ExprKind::StringLiteral(value), span))
            }
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance()?;
                Ok(Expr::new(ExprKind::Identifier(name), span))
            }
            TokenKind::LParen => {
                self.advance()?;
                let expr = self.parse_expression()?;
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            _ => Err(CompileError::parser(
                format!("unexpected token in expression: {}", self.current.kind),
                span,
            )),
        }
    }

    // =========================================================================
    // Literal parsing helpers
    // =========================================================================

    fn parse_int_literal(&self, s: &str) -> CompileResult<i64> {
        let s = s.trim_end_matches(['u', 'U', 'l', 'L']);
        s.parse().map_err(|_| {
            CompileError::parser(format!("invalid integer literal: {}", s), self.current.span)
        })
    }

    fn parse_hex_literal(&self, s: &str) -> CompileResult<i64> {
        let s = s.trim_start_matches("0x").trim_start_matches("0X");
        let s = s.trim_end_matches(['u', 'U', 'l', 'L']);
        i64::from_str_radix(s, 16).map_err(|_| {
            CompileError::parser(format!("invalid hex literal: {}", s), self.current.span)
        })
    }

    fn parse_octal_literal(&self, s: &str) -> CompileResult<i64> {
        let s = s.trim_start_matches('0');
        let s = s.trim_end_matches(['u', 'U', 'l', 'L']);
        if s.is_empty() {
            return Ok(0);
        }
        i64::from_str_radix(s, 8).map_err(|_| {
            CompileError::parser(format!("invalid octal literal: {}", s), self.current.span)
        })
    }

    fn parse_binary_literal(&self, s: &str) -> CompileResult<i64> {
        let s = s.trim_start_matches("0b").trim_start_matches("0B");
        let s = s.trim_end_matches(['u', 'U', 'l', 'L']);
        i64::from_str_radix(s, 2).map_err(|_| {
            CompileError::parser(format!("invalid binary literal: {}", s), self.current.span)
        })
    }

    fn parse_char_literal(&self, s: &str) -> CompileResult<char> {
        let inner = &s[1..s.len() - 1]; // Remove quotes
        if inner.starts_with('\\') {
            let mut chars = inner.chars().peekable();
            chars.next(); // consume backslash
            self.parse_escape_sequence(&mut chars)
                .map(|c| c as char)
        } else {
            Ok(inner.chars().next().unwrap_or('\0'))
        }
    }

    fn parse_string_literal(&self, s: &str) -> CompileResult<String> {
        let inner = &s[1..s.len() - 1]; // Remove quotes
        let mut result = String::new();
        let mut chars = inner.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\\' {
                let escaped = self.parse_escape_sequence(&mut chars)?;
                result.push(escaped as char);
            } else {
                result.push(c);
            }
        }

        Ok(result)
    }

    /// Parse an escape sequence after the backslash
    fn parse_escape_sequence(&self, chars: &mut std::iter::Peekable<std::str::Chars>) -> CompileResult<u8> {
        match chars.next() {
            Some('n') => Ok(b'\n'),
            Some('r') => Ok(b'\r'),
            Some('t') => Ok(b'\t'),
            Some('0') => {
                // Could be null or octal escape
                // Check if next char is a digit (octal)
                if chars.peek().map(|c| c.is_ascii_digit() && *c < '8').unwrap_or(false) {
                    // Octal escape: \0nn (up to 3 total octal digits including the leading 0)
                    let mut value: u32 = 0;
                    let mut count = 0;
                    while count < 2 { // Already consumed '0', so 2 more digits max
                        if let Some(&c) = chars.peek() {
                            if c >= '0' && c <= '7' {
                                chars.next();
                                value = value * 8 + (c as u32 - '0' as u32);
                                count += 1;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    Ok(value as u8)
                } else {
                    Ok(0) // Just \0 = null
                }
            }
            Some(c) if c >= '1' && c <= '7' => {
                // Octal escape: \nnn (1-3 octal digits)
                let mut value: u32 = c as u32 - '0' as u32;
                let mut count = 1;
                while count < 3 {
                    if let Some(&next) = chars.peek() {
                        if next >= '0' && next <= '7' {
                            chars.next();
                            value = value * 8 + (next as u32 - '0' as u32);
                            count += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                Ok(value as u8)
            }
            Some('x') => {
                // Hex escape: \xnn (any number of hex digits, but typically 1-2)
                let mut value: u32 = 0;
                let mut found_digit = false;
                while let Some(&c) = chars.peek() {
                    if let Some(digit) = c.to_digit(16) {
                        chars.next();
                        value = value * 16 + digit;
                        found_digit = true;
                    } else {
                        break;
                    }
                }
                if !found_digit {
                    // \x with no digits - just return 'x'
                    return Ok(b'x');
                }
                Ok(value as u8)
            }
            Some('a') => Ok(0x07), // Bell
            Some('b') => Ok(0x08), // Backspace
            Some('f') => Ok(0x0C), // Form feed
            Some('v') => Ok(0x0B), // Vertical tab
            Some('\\') => Ok(b'\\'),
            Some('\'') => Ok(b'\''),
            Some('"') => Ok(b'"'),
            Some('?') => Ok(b'?'),
            Some(c) => Ok(c as u8),
            None => Ok(0),
        }
    }

    // =========================================================================
    // Initializers
    // =========================================================================

    fn parse_initializer(&mut self) -> CompileResult<Initializer> {
        if self.check(&TokenKind::LBrace) {
            self.parse_initializer_list()
        } else {
            let expr = self.parse_assignment_expression()?;
            Ok(Initializer::Expr(expr))
        }
    }

    fn parse_initializer_list(&mut self) -> CompileResult<Initializer> {
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();

        if !self.check(&TokenKind::RBrace) {
            loop {
                // Check for designator
                if self.check(&TokenKind::Dot) || self.check(&TokenKind::LBracket) {
                    let designator = self.parse_designator()?;
                    self.expect(TokenKind::Eq)?;
                    let value = Box::new(self.parse_initializer()?);
                    items.push(Initializer::Designated { designator, value });
                } else {
                    items.push(self.parse_initializer()?);
                }

                if !self.match_token(&TokenKind::Comma)? {
                    break;
                }

                // Allow trailing comma
                if self.check(&TokenKind::RBrace) {
                    break;
                }
            }
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Initializer::List(items))
    }

    fn parse_designator(&mut self) -> CompileResult<Designator> {
        if self.match_token(&TokenKind::Dot)? {
            let name = if let TokenKind::Identifier(name) = &self.current.kind {
                let n = name.clone();
                self.advance()?;
                n
            } else {
                return Err(CompileError::parser("expected field name", self.current.span));
            };
            Ok(Designator::Field(name))
        } else if self.match_token(&TokenKind::LBracket)? {
            let index = self.parse_constant_expression()?;
            self.expect(TokenKind::RBracket)?;
            Ok(Designator::Index(Box::new(index)))
        } else {
            Err(CompileError::parser("expected designator", self.current.span))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_function() {
        let source = "int main() { return 0; }";
        let mut parser = Parser::new(source).unwrap();
        let tu = parser.parse().unwrap();

        assert_eq!(tu.declarations.len(), 1);
        if let DeclKind::Function(f) = &tu.declarations[0].kind {
            assert_eq!(f.name, "main");
            assert!(f.body.is_some());
        } else {
            panic!("expected function declaration");
        }
    }

    #[test]
    fn test_parse_variable_declaration() {
        let source = "int x = 42;";
        let mut parser = Parser::new(source).unwrap();
        let tu = parser.parse().unwrap();

        assert_eq!(tu.declarations.len(), 1);
        if let DeclKind::Variable(v) = &tu.declarations[0].kind {
            assert_eq!(v.name, "x");
            assert!(v.init.is_some());
        } else {
            panic!("expected variable declaration");
        }
    }

    #[test]
    fn test_parse_expressions() {
        let source = "int f() { return 1 + 2 * 3; }";
        let mut parser = Parser::new(source).unwrap();
        let tu = parser.parse().unwrap();

        assert_eq!(tu.declarations.len(), 1);
    }

    #[test]
    fn test_parse_if_statement() {
        let source = "void f() { if (x) y = 1; else y = 2; }";
        let mut parser = Parser::new(source).unwrap();
        let tu = parser.parse().unwrap();

        assert_eq!(tu.declarations.len(), 1);
    }

    #[test]
    fn test_parse_while_loop() {
        let source = "void f() { while (x < 10) x++; }";
        let mut parser = Parser::new(source).unwrap();
        let tu = parser.parse().unwrap();

        assert_eq!(tu.declarations.len(), 1);
    }

    #[test]
    fn test_parse_for_loop() {
        let source = "void f() { for (int i = 0; i < 10; i++) x++; }";
        let mut parser = Parser::new(source).unwrap();
        let tu = parser.parse().unwrap();

        assert_eq!(tu.declarations.len(), 1);
    }
}
