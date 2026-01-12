//! Rust semantic analyzer

use crate::common::{CompileError, CompileResult};
use crate::frontend::rust::ast::*;
use super::scope::{RustScope, RustSymbol, RustSymbolKind};
use super::types::TypeChecker;

/// Rust semantic analyzer
pub struct RustAnalyzer {
    scope: RustScope,
    type_checker: TypeChecker,
    current_fn_return_type: Option<RustType>,
}

impl RustAnalyzer {
    pub fn new() -> Self {
        Self {
            scope: RustScope::new(),
            type_checker: TypeChecker::new(),
            current_fn_return_type: None,
        }
    }

    /// Analyze a complete module
    pub fn analyze(&mut self, module: &mut RustModule) -> CompileResult<()> {
        // First pass: collect type declarations
        for item in &module.items {
            self.collect_item_types(item)?;
        }

        // Second pass: analyze items
        for item in &mut module.items {
            self.analyze_item(item)?;
        }

        Ok(())
    }

    fn collect_item_types(&mut self, item: &Item) -> CompileResult<()> {
        match &item.kind {
            ItemKind::Struct(s) => {
                let ty = self.struct_to_type(s);
                let symbol = RustSymbol::new(
                    s.name.clone(),
                    RustSymbolKind::Struct(Box::new(s.clone())),
                    ty.clone(),
                );
                self.scope.define(symbol).map_err(|e| CompileError::semantic(e, item.span))?;
                self.scope.define_type(s.name.clone(), ty)
                    .map_err(|e| CompileError::semantic(e, item.span))?;
            }
            ItemKind::Enum(e) => {
                let ty = self.enum_to_type(e);
                let symbol = RustSymbol::new(
                    e.name.clone(),
                    RustSymbolKind::Enum(Box::new(e.clone())),
                    ty.clone(),
                );
                self.scope.define(symbol).map_err(|e| CompileError::semantic(e, item.span))?;
                self.scope.define_type(e.name.clone(), ty)
                    .map_err(|e| CompileError::semantic(e, item.span))?;

                // Register enum variants
                for (i, variant) in e.variants.iter().enumerate() {
                    let variant_ty = RustType::new(
                        RustTypeKind::Named(TypePath::simple(e.name.clone())),
                        variant.span,
                    );
                    let symbol = RustSymbol::new(
                        variant.name.clone(),
                        RustSymbolKind::EnumVariant {
                            enum_name: e.name.clone(),
                            variant_index: i,
                        },
                        variant_ty,
                    );
                    // Variants are in the same namespace as the enum
                    let _ = self.scope.define(symbol);
                }
            }
            ItemKind::TypeAlias(t) => {
                self.scope.define_type(t.name.clone(), t.ty.clone())
                    .map_err(|e| CompileError::semantic(e, item.span))?;
            }
            ItemKind::Fn(f) => {
                let fn_type = self.fn_to_type(f);
                let symbol = RustSymbol::new(
                    f.name.clone(),
                    RustSymbolKind::Function(Box::new(f.clone())),
                    fn_type,
                );
                self.scope.define(symbol).map_err(|e| CompileError::semantic(e, item.span))?;
            }
            ItemKind::Const(c) => {
                let symbol = RustSymbol::new(
                    c.name.clone(),
                    RustSymbolKind::Const,
                    c.ty.clone(),
                );
                self.scope.define(symbol).map_err(|e| CompileError::semantic(e, item.span))?;
            }
            ItemKind::Static(s) => {
                let symbol = RustSymbol::new(
                    s.name.clone(),
                    RustSymbolKind::Static { mutable: s.mutable },
                    s.ty.clone(),
                );
                self.scope.define(symbol).map_err(|e| CompileError::semantic(e, item.span))?;
            }
            _ => {}
        }
        Ok(())
    }

    fn struct_to_type(&self, s: &StructDecl) -> RustType {
        RustType::new(
            RustTypeKind::Named(TypePath::simple(s.name.clone())),
            s.span,
        )
    }

    fn enum_to_type(&self, e: &EnumDecl) -> RustType {
        RustType::new(
            RustTypeKind::Named(TypePath::simple(e.name.clone())),
            e.span,
        )
    }

    fn fn_to_type(&self, f: &FnDecl) -> RustType {
        // For simplicity, we store the function as a named type
        // In a full implementation, we'd have a proper function type
        let return_type = f.return_type.clone().unwrap_or_else(|| RustType::unit(f.span));
        return_type
    }

    fn analyze_item(&mut self, item: &mut Item) -> CompileResult<()> {
        match &mut item.kind {
            ItemKind::Fn(f) => self.analyze_fn(f),
            ItemKind::Impl(i) => self.analyze_impl(i),
            ItemKind::Const(c) => self.analyze_const(c),
            ItemKind::Static(s) => self.analyze_static(s),
            ItemKind::Mod(m) => self.analyze_mod(m),
            // Struct, Enum, TypeAlias already handled in first pass
            _ => Ok(()),
        }
    }

    fn analyze_fn(&mut self, f: &mut FnDecl) -> CompileResult<()> {
        if let Some(body) = &mut f.body {
            self.scope.push_child();

            // Add parameters to scope
            for param in &f.params {
                let name = self.pattern_binding_name(&param.pattern);
                if let Some(name) = name {
                    let mutable = self.pattern_is_mutable(&param.pattern);
                    let symbol = RustSymbol::new(
                        name,
                        RustSymbolKind::Parameter,
                        param.ty.clone(),
                    ).with_mutability(mutable);
                    self.scope.define(symbol).map_err(|e| CompileError::semantic(e, param.span))?;
                }
            }

            // Set current function return type
            self.current_fn_return_type = f.return_type.clone();

            // Analyze body
            self.analyze_block(body)?;

            self.current_fn_return_type = None;
            self.scope.pop_to_parent();
        }
        Ok(())
    }

    fn analyze_impl(&mut self, i: &mut ImplDecl) -> CompileResult<()> {
        for item in &mut i.items {
            self.analyze_item(item)?;
        }
        Ok(())
    }

    fn analyze_const(&mut self, c: &mut ConstDecl) -> CompileResult<()> {
        let value_ty = self.analyze_expr(&mut c.value)?;
        if !self.type_checker.is_assignable(&c.ty, &value_ty) {
            return Err(CompileError::type_error(
                format!("mismatched types: expected {}, found {}", c.ty, value_ty),
                c.span,
            ));
        }
        Ok(())
    }

    fn analyze_static(&mut self, s: &mut StaticDecl) -> CompileResult<()> {
        let value_ty = self.analyze_expr(&mut s.value)?;
        if !self.type_checker.is_assignable(&s.ty, &value_ty) {
            return Err(CompileError::type_error(
                format!("mismatched types: expected {}, found {}", s.ty, value_ty),
                s.span,
            ));
        }
        Ok(())
    }

    fn analyze_mod(&mut self, m: &mut ModDecl) -> CompileResult<()> {
        if let Some(items) = &mut m.items {
            self.scope.push_child();
            // First pass: collect types
            for item in items.iter() {
                self.collect_item_types(item)?;
            }
            // Second pass: analyze
            for item in items.iter_mut() {
                self.analyze_item(item)?;
            }
            self.scope.pop_to_parent();
        }
        Ok(())
    }

    fn analyze_block(&mut self, block: &mut Block) -> CompileResult<Option<RustType>> {
        for stmt in &mut block.stmts {
            self.analyze_stmt(stmt)?;
        }

        if let Some(expr) = &mut block.expr {
            let ty = self.analyze_expr(expr)?;
            Ok(Some(ty))
        } else {
            Ok(None)
        }
    }

    fn analyze_stmt(&mut self, stmt: &mut Stmt) -> CompileResult<()> {
        match &mut stmt.kind {
            StmtKind::Let { pattern, ty, init } => {
                let init_ty = if let Some(init) = init {
                    Some(self.analyze_expr(init)?)
                } else {
                    None
                };

                let var_ty = if let Some(ty) = ty {
                    if let Some(init_ty) = &init_ty {
                        if !self.type_checker.is_assignable(ty, init_ty) {
                            return Err(CompileError::type_error(
                                format!("mismatched types: expected {}, found {}", ty, init_ty),
                                stmt.span,
                            ));
                        }
                    }
                    ty.clone()
                } else if let Some(init_ty) = init_ty {
                    init_ty
                } else {
                    return Err(CompileError::semantic(
                        "type annotations needed",
                        stmt.span,
                    ));
                };

                let name = self.pattern_binding_name(pattern);
                if let Some(name) = name {
                    let mutable = self.pattern_is_mutable(pattern);
                    let symbol = RustSymbol::new(
                        name,
                        RustSymbolKind::Variable,
                        var_ty,
                    ).with_mutability(mutable);
                    self.scope.define(symbol).map_err(|e| CompileError::semantic(e, stmt.span))?;
                }
            }
            StmtKind::Expr(expr) | StmtKind::ExprNoSemi(expr) => {
                self.analyze_expr(expr)?;
            }
            StmtKind::Item(item) => {
                self.collect_item_types(item)?;
                self.analyze_item(item)?;
            }
            StmtKind::Empty => {}
        }
        Ok(())
    }

    fn analyze_expr(&mut self, expr: &mut Expr) -> CompileResult<RustType> {
        let ty = match &mut expr.kind {
            ExprKind::IntLiteral(value) => {
                self.type_checker.infer_int_literal_type(*value, None, expr.span)
            }
            ExprKind::FloatLiteral(_) => {
                self.type_checker.infer_float_literal_type(None, expr.span)
            }
            ExprKind::BoolLiteral(_) => RustType::bool(expr.span),
            ExprKind::CharLiteral(_) => {
                RustType::new(RustTypeKind::Primitive(PrimitiveType::Char), expr.span)
            }
            ExprKind::StringLiteral(_) => {
                // &'static str
                RustType::reference(
                    RustType::new(RustTypeKind::Named(TypePath::simple("str".to_string())), expr.span),
                    false,
                    expr.span,
                )
            }
            ExprKind::ByteLiteral(_) => {
                RustType::new(RustTypeKind::Primitive(PrimitiveType::U8), expr.span)
            }
            ExprKind::ByteStringLiteral(bytes) => {
                // &'static [u8; N]
                RustType::reference(
                    RustType::new(
                        RustTypeKind::Array {
                            element: Box::new(RustType::new(
                                RustTypeKind::Primitive(PrimitiveType::U8),
                                expr.span,
                            )),
                            size: bytes.len(),
                        },
                        expr.span,
                    ),
                    false,
                    expr.span,
                )
            }
            ExprKind::Identifier(name) => {
                if let Some(sym) = self.scope.lookup(name) {
                    if sym.moved && !sym.ty.is_copy() {
                        return Err(CompileError::semantic(
                            format!("use of moved value '{}'", name),
                            expr.span,
                        ));
                    }
                    sym.ty.clone()
                } else {
                    return Err(CompileError::semantic(
                        format!("undefined identifier '{}'", name),
                        expr.span,
                    ));
                }
            }
            ExprKind::Path(path) => {
                // Look up the path
                if let Some(sym) = self.scope.lookup(path.name()) {
                    sym.ty.clone()
                } else if let Some(ty) = self.scope.lookup_type(path.name()) {
                    ty.clone()
                } else {
                    return Err(CompileError::semantic(
                        format!("undefined path '{}'", path),
                        expr.span,
                    ));
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left_ty = self.analyze_expr(left)?;
                let right_ty = self.analyze_expr(right)?;
                self.type_checker.binary_result_type(*op, &left_ty, &right_ty, expr.span)?
            }
            ExprKind::Unary { op, operand } => {
                let operand_ty = self.analyze_expr(operand)?;
                self.type_checker.unary_result_type(*op, &operand_ty, expr.span)?
            }
            ExprKind::Assign { target, value, .. } => {
                let target_ty = self.analyze_expr(target)?;
                let value_ty = self.analyze_expr(value)?;

                if !self.type_checker.is_assignable(&target_ty, &value_ty) {
                    return Err(CompileError::type_error(
                        format!("mismatched types: expected {}, found {}", target_ty, value_ty),
                        expr.span,
                    ));
                }

                RustType::unit(expr.span)
            }
            ExprKind::Call { callee, args } => {
                let callee_ty = self.analyze_expr(callee)?;
                for arg in args {
                    self.analyze_expr(arg)?;
                }
                // Simplified: return the callee type (would need function type info)
                callee_ty
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                let receiver_ty = self.analyze_expr(receiver)?;
                for arg in args {
                    self.analyze_expr(arg)?;
                }
                // Simplified: return the receiver type
                receiver_ty
            }
            ExprKind::Field { object, field: _ } => {
                let _obj_ty = self.analyze_expr(object)?;
                // Look up field type - simplified
                RustType::new(RustTypeKind::Infer, expr.span)
            }
            ExprKind::TupleField { object, index } => {
                let obj_ty = self.analyze_expr(object)?;
                match &obj_ty.kind {
                    RustTypeKind::Tuple(types) => {
                        if *index < types.len() {
                            types[*index].clone()
                        } else {
                            return Err(CompileError::type_error(
                                format!("tuple index {} out of bounds", index),
                                expr.span,
                            ));
                        }
                    }
                    _ => {
                        return Err(CompileError::type_error(
                            "tuple field access on non-tuple",
                            expr.span,
                        ));
                    }
                }
            }
            ExprKind::Index { object, index } => {
                let obj_ty = self.analyze_expr(object)?;
                let index_ty = self.analyze_expr(index)?;

                if !index_ty.is_integer() {
                    return Err(CompileError::type_error(
                        "array index must be an integer",
                        expr.span,
                    ));
                }

                self.type_checker.element_type(&obj_ty).ok_or_else(|| {
                    CompileError::type_error("cannot index into this type", expr.span)
                })?
            }
            ExprKind::Reference { mutable, operand } => {
                let operand_ty = self.analyze_expr(operand)?;
                RustType::reference(operand_ty, *mutable, expr.span)
            }
            ExprKind::Dereference(operand) => {
                let operand_ty = self.analyze_expr(operand)?;
                match &operand_ty.kind {
                    RustTypeKind::Reference { inner, .. } |
                    RustTypeKind::Pointer { inner, .. } => (**inner).clone(),
                    _ => {
                        return Err(CompileError::type_error(
                            "cannot dereference non-pointer type",
                            expr.span,
                        ));
                    }
                }
            }
            ExprKind::Cast { expr: inner, ty } => {
                self.analyze_expr(inner)?;
                ty.clone()
            }
            ExprKind::Block(block) => {
                self.scope.push_child();
                let ty = self.analyze_block(block)?;
                self.scope.pop_to_parent();
                ty.unwrap_or_else(|| RustType::unit(expr.span))
            }
            ExprKind::If { condition, then_block, else_block } => {
                let cond_ty = self.analyze_expr(condition)?;
                if !self.type_checker.is_bool(&cond_ty) {
                    return Err(CompileError::type_error(
                        "if condition must be a bool",
                        condition.span,
                    ));
                }

                self.scope.push_child();
                let then_ty = self.analyze_block(then_block)?;
                self.scope.pop_to_parent();

                if let Some(else_expr) = else_block {
                    let else_ty = self.analyze_expr(else_expr)?;
                    if let Some(then_ty) = then_ty {
                        if !self.type_checker.types_match(&then_ty, &else_ty) {
                            return Err(CompileError::type_error(
                                format!("if/else branches have incompatible types: {} vs {}", then_ty, else_ty),
                                expr.span,
                            ));
                        }
                        then_ty
                    } else {
                        else_ty
                    }
                } else {
                    RustType::unit(expr.span)
                }
            }
            ExprKind::Loop { body, .. } => {
                self.scope.push_child();
                self.scope.enter_loop();
                self.analyze_block(body)?;
                self.scope.exit_loop();
                self.scope.pop_to_parent();
                // Loop without break value has type !
                RustType::never(expr.span)
            }
            ExprKind::While { condition, body, .. } => {
                let cond_ty = self.analyze_expr(condition)?;
                if !self.type_checker.is_bool(&cond_ty) {
                    return Err(CompileError::type_error(
                        "while condition must be a bool",
                        condition.span,
                    ));
                }

                self.scope.push_child();
                self.scope.enter_loop();
                self.analyze_block(body)?;
                self.scope.exit_loop();
                self.scope.pop_to_parent();
                RustType::unit(expr.span)
            }
            ExprKind::For { pattern, iter, body, .. } => {
                let iter_ty = self.analyze_expr(iter)?;
                // Simplified: assume iterator yields the element type
                let elem_ty = self.type_checker.element_type(&iter_ty)
                    .unwrap_or_else(|| RustType::new(RustTypeKind::Infer, expr.span));

                self.scope.push_child();
                self.scope.enter_loop();

                // Add pattern binding
                if let Some(name) = self.pattern_binding_name(pattern) {
                    let symbol = RustSymbol::new(
                        name,
                        RustSymbolKind::Variable,
                        elem_ty,
                    );
                    let _ = self.scope.define(symbol);
                }

                self.analyze_block(body)?;
                self.scope.exit_loop();
                self.scope.pop_to_parent();
                RustType::unit(expr.span)
            }
            ExprKind::Match { scrutinee, arms } => {
                let _scrutinee_ty = self.analyze_expr(scrutinee)?;
                let mut arm_types = Vec::new();

                for arm in arms {
                    self.scope.push_child();
                    // TODO: properly type-check patterns
                    if let Some(guard) = &mut arm.guard.clone() {
                        self.analyze_expr(guard)?;
                    }
                    let arm_ty = self.analyze_expr(&mut arm.body.clone())?;
                    arm_types.push(arm_ty);
                    self.scope.pop_to_parent();
                }

                // All arms must have compatible types
                if let Some(first) = arm_types.first() {
                    for ty in &arm_types[1..] {
                        if !self.type_checker.types_match(first, ty) {
                            return Err(CompileError::type_error(
                                "match arms have incompatible types",
                                expr.span,
                            ));
                        }
                    }
                    first.clone()
                } else {
                    RustType::never(expr.span)
                }
            }
            ExprKind::Break { value, .. } => {
                if !self.scope.in_loop() {
                    return Err(CompileError::semantic(
                        "break outside of loop",
                        expr.span,
                    ));
                }
                if let Some(value) = value {
                    self.analyze_expr(value)?;
                }
                RustType::never(expr.span)
            }
            ExprKind::Continue { .. } => {
                if !self.scope.in_loop() {
                    return Err(CompileError::semantic(
                        "continue outside of loop",
                        expr.span,
                    ));
                }
                RustType::never(expr.span)
            }
            ExprKind::Return(value) => {
                if let Some(value) = value {
                    let value_ty = self.analyze_expr(value)?;
                    if let Some(expected) = &self.current_fn_return_type {
                        if !self.type_checker.is_assignable(expected, &value_ty) {
                            return Err(CompileError::type_error(
                                format!("mismatched return type: expected {}, found {}", expected, value_ty),
                                expr.span,
                            ));
                        }
                    }
                }
                RustType::never(expr.span)
            }
            ExprKind::Range { start, end, .. } => {
                if let Some(start) = start {
                    self.analyze_expr(start)?;
                }
                if let Some(end) = end {
                    self.analyze_expr(end)?;
                }
                // Range type - simplified
                RustType::new(RustTypeKind::Named(TypePath::simple("Range".to_string())), expr.span)
            }
            ExprKind::Tuple(exprs) => {
                let types: Vec<_> = exprs.iter_mut()
                    .map(|e| self.analyze_expr(e))
                    .collect::<CompileResult<_>>()?;
                RustType::new(RustTypeKind::Tuple(types), expr.span)
            }
            ExprKind::Array(exprs) => {
                if exprs.is_empty() {
                    return Ok(RustType::new(
                        RustTypeKind::Array {
                            element: Box::new(RustType::new(RustTypeKind::Infer, expr.span)),
                            size: 0,
                        },
                        expr.span,
                    ));
                }

                let first_ty = self.analyze_expr(&mut exprs[0])?;
                for e in &mut exprs[1..] {
                    let ty = self.analyze_expr(e)?;
                    if !self.type_checker.types_match(&first_ty, &ty) {
                        return Err(CompileError::type_error(
                            "array elements must have the same type",
                            e.span,
                        ));
                    }
                }

                RustType::new(
                    RustTypeKind::Array {
                        element: Box::new(first_ty),
                        size: exprs.len(),
                    },
                    expr.span,
                )
            }
            ExprKind::ArrayRepeat { value, count } => {
                let elem_ty = self.analyze_expr(value)?;
                let count_ty = self.analyze_expr(count)?;

                if !count_ty.is_integer() {
                    return Err(CompileError::type_error(
                        "array repeat count must be an integer",
                        count.span,
                    ));
                }

                // We'd need const evaluation to get the actual count
                RustType::new(
                    RustTypeKind::Array {
                        element: Box::new(elem_ty),
                        size: 0, // Unknown at this point
                    },
                    expr.span,
                )
            }
            ExprKind::Struct { path, fields, rest } => {
                // Look up struct type
                let struct_ty = if let Some(ty) = self.scope.lookup_type(path.name()) {
                    ty.clone()
                } else {
                    return Err(CompileError::type_error(
                        format!("undefined struct '{}'", path),
                        expr.span,
                    ));
                };

                for field in fields {
                    if let Some(value) = &mut field.value.clone() {
                        self.analyze_expr(value)?;
                    }
                }

                if let Some(rest) = rest {
                    self.analyze_expr(&mut *rest.clone())?;
                }

                struct_ty
            }
            ExprKind::Closure { params, return_type: _, body } => {
                self.scope.push_child();

                for (pattern, ty) in params {
                    if let Some(name) = self.pattern_binding_name(pattern) {
                        let param_ty = ty.clone().unwrap_or_else(|| RustType::new(RustTypeKind::Infer, expr.span));
                        let symbol = RustSymbol::new(
                            name,
                            RustSymbolKind::Parameter,
                            param_ty,
                        );
                        let _ = self.scope.define(symbol);
                    }
                }

                let body_ty = self.analyze_expr(body)?;
                self.scope.pop_to_parent();

                // Return the body type for now (simplified)
                body_ty
            }
            ExprKind::Unsafe(block) => {
                self.scope.push_child();
                self.scope.enter_unsafe();
                let ty = self.analyze_block(block)?;
                self.scope.exit_unsafe();
                self.scope.pop_to_parent();
                ty.unwrap_or_else(|| RustType::unit(expr.span))
            }
            ExprKind::Paren(inner) => {
                self.analyze_expr(inner)?
            }
        };

        expr.ty = Some(ty.clone());
        Ok(ty)
    }

    fn pattern_binding_name(&self, pattern: &Pattern) -> Option<String> {
        match &pattern.kind {
            PatternKind::Binding { name, .. } => Some(name.clone()),
            PatternKind::Paren(inner) => self.pattern_binding_name(inner),
            _ => None,
        }
    }

    fn pattern_is_mutable(&self, pattern: &Pattern) -> bool {
        match &pattern.kind {
            PatternKind::Binding { mutable, .. } => *mutable,
            PatternKind::Paren(inner) => self.pattern_is_mutable(inner),
            _ => false,
        }
    }
}

impl Default for RustAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
