//! Semantic analyzer - type checking and validation

use crate::frontend::c::ast::*;
use crate::common::{CompileError, CompileResult};
use super::scope::{Scope, Symbol, SymbolKind, StructDef, UnionDef};

/// Semantic analyzer for type checking
pub struct SemanticAnalyzer {
    scope: Scope,
    current_function_return_type: Option<CType>,
    in_loop: bool,
    in_switch: bool,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            scope: Scope::new(),
            current_function_return_type: None,
            in_loop: false,
            in_switch: false,
        }
    }

    /// Analyze a translation unit
    pub fn analyze(&mut self, tu: &mut TranslationUnit) -> CompileResult<()> {
        for decl in &mut tu.declarations {
            self.analyze_declaration(decl)?;
        }
        Ok(())
    }

    fn analyze_declaration(&mut self, decl: &mut Declaration) -> CompileResult<()> {
        match &mut decl.kind {
            DeclKind::Variable(var) => self.analyze_var_decl(var),
            DeclKind::MultipleVariables(vars) => {
                for var in vars {
                    self.analyze_var_decl(var)?;
                }
                Ok(())
            }
            DeclKind::Function(func) => self.analyze_func_decl(func),
            DeclKind::Struct(s) => self.analyze_struct_decl(s),
            DeclKind::Union(u) => self.analyze_union_decl(u),
            DeclKind::Enum(e) => self.analyze_enum_decl(e),
            DeclKind::Typedef(t) => self.analyze_typedef_decl(t),
        }
    }

    fn analyze_var_decl(&mut self, var: &mut VarDecl) -> CompileResult<()> {
        // Resolve struct type if needed
        self.resolve_struct_type(&mut var.ty)?;

        // Define the variable in current scope
        let symbol = Symbol {
            name: var.name.clone(),
            kind: SymbolKind::Variable,
            ty: var.ty.clone(),
        };
        self.scope.define(symbol).map_err(|e| {
            CompileError::semantic(e, var.span)
        })?;

        // Analyze initializer if present
        if let Some(init) = &mut var.init {
            self.analyze_initializer(init, &var.ty)?;
        }

        Ok(())
    }

    /// Resolve struct/union types by looking up definitions and filling in members
    fn resolve_struct_type(&self, ty: &mut CType) -> CompileResult<()> {
        match &mut ty.kind {
            TypeKind::Struct { name: Some(struct_name), members } if members.is_empty() => {
                // Look up the struct definition
                if let Some(def) = self.scope.lookup_struct(struct_name) {
                    *members = def.members.clone();
                }
            }
            TypeKind::Union { name: Some(union_name), members } if members.is_empty() => {
                // Look up the union definition
                if let Some(def) = self.scope.lookup_union(union_name) {
                    *members = def.members.clone();
                }
            }
            TypeKind::Pointer(inner) => {
                self.resolve_struct_type(inner)?;
            }
            TypeKind::Array { element, .. } => {
                self.resolve_struct_type(element)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn analyze_func_decl(&mut self, func: &mut FuncDecl) -> CompileResult<()> {
        // Define the function in current scope
        let func_type = CType::new(
            TypeKind::Function {
                return_type: Box::new(func.return_type.clone()),
                params: func.params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect(),
                variadic: func.variadic,
            },
            func.span,
        );

        let symbol = Symbol {
            name: func.name.clone(),
            kind: SymbolKind::Function,
            ty: func_type,
        };

        // Allow redefinition for function declarations
        let _ = self.scope.define(symbol);

        // If there's a body, analyze it
        if let Some(body) = &mut func.body {
            // Enter new scope for function body
            self.scope.push_child();

            // Clear labels for new function scope
            self.scope.clear_labels();

            // Add parameters to scope
            for param in &mut func.params {
                // Resolve struct types in parameter types
                self.resolve_struct_type(&mut param.ty)?;

                if let Some(name) = &param.name {
                    let symbol = Symbol {
                        name: name.clone(),
                        kind: SymbolKind::Parameter,
                        ty: param.ty.clone(),
                    };
                    self.scope.define(symbol).map_err(|e| {
                        CompileError::semantic(e, param.span)
                    })?;
                }
            }

            // Set current function return type for return statement checking
            self.current_function_return_type = Some(func.return_type.clone());

            // Analyze body
            self.analyze_block(body)?;

            // Check that all referenced labels are defined
            if let Err(undefined) = self.scope.check_labels() {
                return Err(CompileError::semantic(
                    format!("undefined labels: {}", undefined.join(", ")),
                    func.span,
                ));
            }

            // Restore scope
            self.current_function_return_type = None;
            self.scope.pop_to_parent();
        }

        Ok(())
    }

    fn analyze_struct_decl(&mut self, s: &mut StructDecl) -> CompileResult<()> {
        // Register struct type if it has a name and members
        if let (Some(name), Some(members)) = (&s.name, &s.members) {
            let struct_members: Vec<(String, CType)> = members
                .iter()
                .map(|m| (m.name.clone(), m.ty.clone()))
                .collect();

            let def = StructDef {
                name: name.clone(),
                members: struct_members,
            };

            self.scope.define_struct(def).map_err(|e| {
                CompileError::semantic(e, s.span)
            })?;
        }
        Ok(())
    }

    fn analyze_union_decl(&mut self, u: &mut UnionDecl) -> CompileResult<()> {
        // Register union type if it has a name and members
        if let (Some(name), Some(members)) = (&u.name, &u.members) {
            let union_members: Vec<(String, CType)> = members
                .iter()
                .map(|m| (m.name.clone(), m.ty.clone()))
                .collect();

            let def = UnionDef {
                name: name.clone(),
                members: union_members,
            };

            self.scope.define_union(def).map_err(|e| {
                CompileError::semantic(e, u.span)
            })?;
        }
        Ok(())
    }

    fn analyze_enum_decl(&mut self, e: &mut EnumDecl) -> CompileResult<()> {
        // Register enum constants
        if let Some(variants) = &e.variants {
            for variant in variants {
                let value = variant.value.as_ref().map(|e| {
                    if let ExprKind::IntLiteral(v) = &e.kind {
                        *v
                    } else {
                        0
                    }
                }).unwrap_or(0);

                let symbol = Symbol {
                    name: variant.name.clone(),
                    kind: SymbolKind::EnumConstant(value),
                    ty: CType::int(variant.span),
                };
                self.scope.define(symbol).map_err(|err| {
                    CompileError::semantic(err, variant.span)
                })?;
            }
        }
        Ok(())
    }

    fn analyze_typedef_decl(&mut self, t: &mut TypedefDecl) -> CompileResult<()> {
        let symbol = Symbol {
            name: t.name.clone(),
            kind: SymbolKind::Typedef,
            ty: t.ty.clone(),
        };
        self.scope.define(symbol).map_err(|e| {
            CompileError::semantic(e, t.span)
        })?;
        Ok(())
    }

    fn analyze_block(&mut self, block: &mut Block) -> CompileResult<()> {
        for item in &mut block.items {
            match item {
                BlockItem::Statement(stmt) => self.analyze_stmt(stmt)?,
                BlockItem::Declaration(decl) => self.analyze_declaration(decl)?,
            }
        }
        Ok(())
    }

    fn analyze_stmt(&mut self, stmt: &mut Stmt) -> CompileResult<()> {
        match &mut stmt.kind {
            StmtKind::Expr(expr) => {
                self.analyze_expr(expr)?;
            }
            StmtKind::Empty => {}
            StmtKind::Block(block) => {
                self.scope.push_child();
                self.analyze_block(block)?;
                self.scope.pop_to_parent();
            }
            StmtKind::If { condition, then_branch, else_branch } => {
                self.analyze_expr(condition)?;
                self.analyze_stmt(then_branch)?;
                if let Some(else_branch) = else_branch {
                    self.analyze_stmt(else_branch)?;
                }
            }
            StmtKind::While { condition, body } => {
                self.analyze_expr(condition)?;
                let was_in_loop = self.in_loop;
                self.in_loop = true;
                self.analyze_stmt(body)?;
                self.in_loop = was_in_loop;
            }
            StmtKind::DoWhile { body, condition } => {
                let was_in_loop = self.in_loop;
                self.in_loop = true;
                self.analyze_stmt(body)?;
                self.in_loop = was_in_loop;
                self.analyze_expr(condition)?;
            }
            StmtKind::For { init, condition, update, body } => {
                self.scope.push_child();

                if let Some(init) = init {
                    match init {
                        ForInit::Expr(expr) => { self.analyze_expr(expr)?; }
                        ForInit::Declaration(decl) => { self.analyze_declaration(decl)?; }
                    }
                }
                if let Some(cond) = condition {
                    self.analyze_expr(cond)?;
                }
                if let Some(update) = update {
                    self.analyze_expr(update)?;
                }

                let was_in_loop = self.in_loop;
                self.in_loop = true;
                self.analyze_stmt(body)?;
                self.in_loop = was_in_loop;

                self.scope.pop_to_parent();
            }
            StmtKind::Switch { expr, body } => {
                self.analyze_expr(expr)?;
                let was_in_switch = self.in_switch;
                self.in_switch = true;
                self.analyze_stmt(body)?;
                self.in_switch = was_in_switch;
            }
            StmtKind::Case { value, stmt } => {
                if !self.in_switch {
                    return Err(CompileError::semantic("case outside switch", stmt.span));
                }
                self.analyze_expr(value)?;
                self.analyze_stmt(stmt)?;
            }
            StmtKind::Default(stmt) => {
                if !self.in_switch {
                    return Err(CompileError::semantic("default outside switch", stmt.span));
                }
                self.analyze_stmt(stmt)?;
            }
            StmtKind::Break => {
                if !self.in_loop && !self.in_switch {
                    return Err(CompileError::semantic("break outside loop or switch", stmt.span));
                }
            }
            StmtKind::Continue => {
                if !self.in_loop {
                    return Err(CompileError::semantic("continue outside loop", stmt.span));
                }
            }
            StmtKind::Return(expr) => {
                if let Some(expr) = expr {
                    self.analyze_expr(expr)?;
                }
                // TODO: Check return type matches function return type
            }
            StmtKind::Goto(label) => {
                // Reference the label (will be checked at end of function)
                self.scope.reference_label(label);
            }
            StmtKind::Label { name, stmt } => {
                // Define the label
                self.scope.define_label(name).map_err(|e| {
                    CompileError::semantic(e, stmt.span)
                })?;
                self.analyze_stmt(stmt)?;
            }
            StmtKind::Declaration(decl) => {
                self.analyze_declaration(decl)?;
            }
        }
        Ok(())
    }

    fn analyze_expr(&mut self, expr: &mut Expr) -> CompileResult<CType> {
        let ty = match &mut expr.kind {
            ExprKind::IntLiteral(_) => CType::int(expr.span),
            ExprKind::FloatLiteral(_) => CType::new(TypeKind::Double, expr.span),
            ExprKind::CharLiteral(_) => CType::char(expr.span),
            ExprKind::StringLiteral(_) => CType::pointer_to(CType::char(expr.span), expr.span),

            ExprKind::Identifier(name) => {
                if let Some(sym) = self.scope.lookup(name) {
                    sym.ty.clone()
                } else {
                    return Err(CompileError::semantic(
                        format!("undefined identifier '{}'", name),
                        expr.span,
                    ));
                }
            }

            ExprKind::Binary { left, right, .. } => {
                let left_ty = self.analyze_expr(left)?;
                let _right_ty = self.analyze_expr(right)?;
                // Simplified: just return left type
                // TODO: Proper type coercion
                left_ty
            }

            ExprKind::Unary { operand, .. } => {
                self.analyze_expr(operand)?
            }

            ExprKind::Assign { target, value, .. } => {
                let target_ty = self.analyze_expr(target)?;
                let _value_ty = self.analyze_expr(value)?;
                target_ty
            }

            ExprKind::Ternary { condition, then_expr, else_expr } => {
                self.analyze_expr(condition)?;
                let then_ty = self.analyze_expr(then_expr)?;
                let _else_ty = self.analyze_expr(else_expr)?;
                then_ty
            }

            ExprKind::Call { callee, args } => {
                let callee_ty = self.analyze_expr(callee)?;
                for arg in args {
                    self.analyze_expr(arg)?;
                }
                if let TypeKind::Function { return_type, .. } = callee_ty.kind {
                    *return_type
                } else {
                    return Err(CompileError::type_error(
                        "called object is not a function",
                        expr.span,
                    ));
                }
            }

            ExprKind::Index { array, index } => {
                let array_ty = self.analyze_expr(array)?;
                self.analyze_expr(index)?;
                match array_ty.kind {
                    TypeKind::Array { element, .. } => *element,
                    TypeKind::Pointer(inner) => *inner,
                    _ => return Err(CompileError::type_error(
                        "subscripted value is not an array or pointer",
                        expr.span,
                    )),
                }
            }

            ExprKind::Member { object, field } => {
                let obj_ty = self.analyze_expr(object)?;
                match &obj_ty.kind {
                    TypeKind::Struct { members, .. } | TypeKind::Union { members, .. } => {
                        for (name, ty) in members {
                            if name == field {
                                return Ok(ty.clone());
                            }
                        }
                        return Err(CompileError::type_error(
                            format!("no member named '{}' in struct", field),
                            expr.span,
                        ));
                    }
                    _ => return Err(CompileError::type_error(
                        "member access on non-struct type",
                        expr.span,
                    )),
                }
            }

            ExprKind::PtrMember { pointer, field } => {
                let ptr_ty = self.analyze_expr(pointer)?;
                if let TypeKind::Pointer(inner) = &ptr_ty.kind {
                    match &inner.kind {
                        TypeKind::Struct { members, .. } | TypeKind::Union { members, .. } => {
                            for (name, ty) in members {
                                if name == field {
                                    return Ok(ty.clone());
                                }
                            }
                            return Err(CompileError::type_error(
                                format!("no member named '{}' in struct", field),
                                expr.span,
                            ));
                        }
                        _ => return Err(CompileError::type_error(
                            "member access on non-struct type",
                            expr.span,
                        )),
                    }
                } else {
                    return Err(CompileError::type_error(
                        "arrow operator on non-pointer type",
                        expr.span,
                    ));
                }
            }

            ExprKind::Cast { ty, expr: inner } => {
                self.analyze_expr(inner)?;
                ty.clone()
            }

            ExprKind::Sizeof(arg) => {
                match arg {
                    SizeofArg::Expr(e) => { self.analyze_expr(e)?; }
                    SizeofArg::Type(_) => {}
                }
                // sizeof returns size_t, but we'll use unsigned long
                CType::new(TypeKind::Long { signed: false }, expr.span)
            }

            ExprKind::AddrOf(operand) => {
                let inner_ty = self.analyze_expr(operand)?;
                CType::pointer_to(inner_ty, expr.span)
            }

            ExprKind::Deref(operand) => {
                let ptr_ty = self.analyze_expr(operand)?;
                if let TypeKind::Pointer(inner) = ptr_ty.kind {
                    *inner
                } else {
                    return Err(CompileError::type_error(
                        "dereference of non-pointer type",
                        expr.span,
                    ));
                }
            }

            ExprKind::PreIncrement(operand) | ExprKind::PreDecrement(operand) |
            ExprKind::PostIncrement(operand) | ExprKind::PostDecrement(operand) => {
                self.analyze_expr(operand)?
            }

            ExprKind::Comma(exprs) => {
                let mut last_ty = CType::void(expr.span);
                for e in exprs {
                    last_ty = self.analyze_expr(e)?;
                }
                last_ty
            }

            ExprKind::CompoundLiteral { ty, initializers } => {
                for init in initializers {
                    self.analyze_initializer(init, ty)?;
                }
                ty.clone()
            }
        };

        expr.ty = Some(ty.clone());
        Ok(ty)
    }

    fn analyze_initializer(&mut self, init: &mut Initializer, _expected_ty: &CType) -> CompileResult<()> {
        match init {
            Initializer::Expr(expr) => {
                self.analyze_expr(expr)?;
            }
            Initializer::List(items) => {
                for item in items {
                    self.analyze_initializer(item, _expected_ty)?;
                }
            }
            Initializer::Designated { value, .. } => {
                self.analyze_initializer(value, _expected_ty)?;
            }
        }
        Ok(())
    }
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
