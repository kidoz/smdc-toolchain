//! IR builder - converts AST to IR

use crate::frontend::c::ast::*;
use crate::common::{CompileResult, CompileError};
use super::inst::*;
use std::collections::HashMap;

/// Builds IR from AST
pub struct IrBuilder {
    module: IrModule,
    current_func: Option<IrFunction>,
    temp_counter: u32,
    label_counter: u32,
    string_counter: u32,
    locals: HashMap<String, Temp>,
    break_label: Option<Label>,
    continue_label: Option<Label>,
}

impl IrBuilder {
    pub fn new() -> Self {
        Self {
            module: IrModule::new(),
            current_func: None,
            temp_counter: 0,
            label_counter: 0,
            string_counter: 0,
            locals: HashMap::new(),
            break_label: None,
            continue_label: None,
        }
    }

    /// Build IR module from translation unit
    pub fn build(&mut self, tu: &TranslationUnit) -> CompileResult<IrModule> {
        for decl in &tu.declarations {
            self.build_declaration(decl)?;
        }
        Ok(std::mem::take(&mut self.module))
    }

    fn new_temp(&mut self) -> Temp {
        let t = Temp(self.temp_counter);
        self.temp_counter += 1;
        t
    }

    fn new_label(&mut self, prefix: &str) -> Label {
        let l = Label(format!(".L{}_{}", prefix, self.label_counter));
        self.label_counter += 1;
        l
    }

    fn emit(&mut self, inst: Inst) {
        if let Some(func) = &mut self.current_func {
            func.body.push(inst);
        }
    }

    fn build_declaration(&mut self, decl: &Declaration) -> CompileResult<()> {
        match &decl.kind {
            DeclKind::Function(func) => self.build_function(func),
            DeclKind::Variable(var) => self.build_global_var(var),
            DeclKind::MultipleVariables(vars) => {
                for var in vars {
                    self.build_global_var(var)?;
                }
                Ok(())
            }
            _ => Ok(()), // Skip struct/union/enum/typedef for now
        }
    }

    fn build_global_var(&mut self, var: &VarDecl) -> CompileResult<()> {
        let init_bytes = if let Some(init) = &var.init {
            Some(self.evaluate_initializer(init, &var.ty)?)
        } else {
            None
        };

        let global = IrGlobal {
            name: var.name.clone(),
            ty: var.ty.clone(),
            init: init_bytes,
        };
        self.module.globals.push(global);
        Ok(())
    }

    /// Evaluate a constant initializer to bytes
    fn evaluate_initializer(&self, init: &Initializer, ty: &CType) -> CompileResult<Vec<u8>> {
        match init {
            Initializer::Expr(expr) => {
                self.evaluate_const_expr_to_bytes(expr, ty)
            }
            Initializer::List(items) => {
                self.evaluate_init_list_to_bytes(items, ty)
            }
            Initializer::Designated { .. } => {
                // For designated initializers, return zero-initialized for now
                Ok(vec![0u8; ty.size()])
            }
        }
    }

    /// Evaluate a constant expression to bytes
    fn evaluate_const_expr_to_bytes(&self, expr: &Expr, ty: &CType) -> CompileResult<Vec<u8>> {
        let size = ty.size();
        let value = self.evaluate_const_expr(expr)?;

        // Convert the value to bytes based on size (big-endian for M68k)
        Ok(match size {
            1 => vec![value as u8],
            2 => ((value as i16).to_be_bytes()).to_vec(),
            4 => ((value as i32).to_be_bytes()).to_vec(),
            _ => {
                // For larger types, pad with zeros
                let mut bytes = (value as i64).to_be_bytes().to_vec();
                while bytes.len() < size {
                    bytes.insert(0, 0);
                }
                bytes.truncate(size);
                bytes
            }
        })
    }

    /// Evaluate a constant expression to an integer value
    fn evaluate_const_expr(&self, expr: &Expr) -> CompileResult<i64> {
        match &expr.kind {
            ExprKind::IntLiteral(n) => Ok(*n),
            ExprKind::CharLiteral(c) => Ok(*c as i64),
            ExprKind::Unary { op, operand } => {
                let val = self.evaluate_const_expr(operand)?;
                Ok(match op {
                    UnaryOp::Neg => -val,
                    UnaryOp::Not => if val == 0 { 1 } else { 0 },
                    UnaryOp::BitNot => !val,
                })
            }
            ExprKind::Binary { op, left, right } => {
                let l = self.evaluate_const_expr(left)?;
                let r = self.evaluate_const_expr(right)?;
                Ok(match op {
                    BinaryOp::Add => l.wrapping_add(r),
                    BinaryOp::Sub => l.wrapping_sub(r),
                    BinaryOp::Mul => l.wrapping_mul(r),
                    BinaryOp::Div => {
                        if r == 0 {
                            return Err(CompileError::codegen("division by zero in constant expression"));
                        }
                        l / r
                    }
                    BinaryOp::Mod => {
                        if r == 0 {
                            return Err(CompileError::codegen("modulo by zero in constant expression"));
                        }
                        l % r
                    }
                    BinaryOp::BitAnd => l & r,
                    BinaryOp::BitOr => l | r,
                    BinaryOp::BitXor => l ^ r,
                    BinaryOp::Shl => l.wrapping_shl(r as u32),
                    BinaryOp::Shr => l.wrapping_shr(r as u32),
                    BinaryOp::Eq => if l == r { 1 } else { 0 },
                    BinaryOp::Ne => if l != r { 1 } else { 0 },
                    BinaryOp::Lt => if l < r { 1 } else { 0 },
                    BinaryOp::Le => if l <= r { 1 } else { 0 },
                    BinaryOp::Gt => if l > r { 1 } else { 0 },
                    BinaryOp::Ge => if l >= r { 1 } else { 0 },
                    BinaryOp::LogAnd => if l != 0 && r != 0 { 1 } else { 0 },
                    BinaryOp::LogOr => if l != 0 || r != 0 { 1 } else { 0 },
                })
            }
            ExprKind::Ternary { condition, then_expr, else_expr } => {
                let cond = self.evaluate_const_expr(condition)?;
                if cond != 0 {
                    self.evaluate_const_expr(then_expr)
                } else {
                    self.evaluate_const_expr(else_expr)
                }
            }
            ExprKind::Cast { expr: inner, .. } => {
                // For now, just pass through (proper casts need type info)
                self.evaluate_const_expr(inner)
            }
            ExprKind::Sizeof(arg) => {
                let size = match arg {
                    SizeofArg::Type(ty) => ty.size(),
                    SizeofArg::Expr(e) => e.ty.as_ref().map(|t| t.size()).unwrap_or(4),
                };
                Ok(size as i64)
            }
            _ => Err(CompileError::codegen("non-constant expression in global initializer")),
        }
    }

    /// Evaluate an initializer list to bytes
    fn evaluate_init_list_to_bytes(&self, items: &[Initializer], ty: &CType) -> CompileResult<Vec<u8>> {
        match &ty.kind {
            TypeKind::Array { element, size } => {
                let elem_size = element.size();
                let count = size.unwrap_or(items.len());
                let mut bytes = Vec::with_capacity(elem_size * count);

                for (i, item) in items.iter().enumerate() {
                    if i >= count {
                        break;
                    }
                    let item_bytes = self.evaluate_initializer(item, element)?;
                    bytes.extend(item_bytes);
                }

                // Zero-fill remaining elements
                while bytes.len() < elem_size * count {
                    bytes.push(0);
                }

                Ok(bytes)
            }
            TypeKind::Struct { name: _, members } => {
                let mut bytes = vec![0u8; ty.size()];
                let mut offset = 0;

                for (i, (_name, member_ty)) in members.iter().enumerate() {
                    // Align offset
                    let align = member_ty.alignment();
                    offset = (offset + align - 1) & !(align - 1);

                    if let Some(item) = items.get(i) {
                        let item_bytes = self.evaluate_initializer(item, member_ty)?;
                        let end = (offset + item_bytes.len()).min(bytes.len());
                        bytes[offset..end].copy_from_slice(&item_bytes[..end - offset]);
                    }

                    offset += member_ty.size();
                }

                Ok(bytes)
            }
            _ => {
                // For scalar types with brace-init, use first element
                if let Some(first) = items.first() {
                    self.evaluate_initializer(first, ty)
                } else {
                    Ok(vec![0u8; ty.size()])
                }
            }
        }
    }

    /// Get the offset and type of a struct field for member access (obj.field)
    fn get_struct_field_offset(&self, object: &Expr, field: &str) -> CompileResult<(usize, CType)> {
        let obj_ty = object.ty.as_ref()
            .ok_or_else(|| CompileError::codegen("missing type for struct member access"))?;

        match &obj_ty.kind {
            TypeKind::Struct { members, .. } => {
                self.calculate_field_offset(members, field)
            }
            _ => Err(CompileError::codegen(format!(
                "member access on non-struct type: {:?}", obj_ty.kind
            ))),
        }
    }

    /// Get the offset and type of a struct field for pointer member access (ptr->field)
    fn get_ptr_struct_field_offset(&self, pointer: &Expr, field: &str) -> CompileResult<(usize, CType)> {
        let ptr_ty = pointer.ty.as_ref()
            .ok_or_else(|| CompileError::codegen("missing type for pointer member access"))?;

        match &ptr_ty.kind {
            TypeKind::Pointer(inner) => {
                match &inner.kind {
                    TypeKind::Struct { members, .. } => {
                        self.calculate_field_offset(members, field)
                    }
                    _ => Err(CompileError::codegen(format!(
                        "pointer member access on non-struct pointer: {:?}", inner.kind
                    ))),
                }
            }
            _ => Err(CompileError::codegen(format!(
                "-> operator on non-pointer type: {:?}", ptr_ty.kind
            ))),
        }
    }

    /// Calculate field offset within struct members
    fn calculate_field_offset(&self, members: &[(String, CType)], field: &str) -> CompileResult<(usize, CType)> {
        let mut offset = 0;

        for (name, member_ty) in members {
            // Align offset for this member
            let align = member_ty.alignment();
            offset = (offset + align - 1) & !(align - 1);

            if name == field {
                return Ok((offset, member_ty.clone()));
            }

            offset += member_ty.size();
        }

        Err(CompileError::codegen(format!("unknown struct field: {}", field)))
    }

    fn build_function(&mut self, func: &FuncDecl) -> CompileResult<()> {
        if func.body.is_none() {
            return Ok(()); // Skip declarations without body
        }

        let params: Vec<(String, CType)> = func
            .params
            .iter()
            .map(|p| (p.name.clone().unwrap_or_default(), p.ty.clone()))
            .collect();

        self.current_func = Some(IrFunction::new(
            func.name.clone(),
            params.clone(),
            func.return_type.clone(),
        ));
        self.locals.clear();
        self.temp_counter = 0;

        // Add parameters to locals and emit code to load them from stack
        // Parameters are passed on the stack at positive offsets from the frame pointer
        // In M68k cdecl with LINK A6: first param at 8(a6), second at 12(a6), etc.
        for (idx, (name, ty)) in params.iter().enumerate() {
            if !name.is_empty() {
                let temp = self.new_temp();
                self.locals.insert(name.clone(), temp);
                // Emit instruction to load parameter from its stack position
                // We use a special Param pseudo-value that the backend will translate
                // to the correct stack offset
                self.emit(Inst::LoadParam {
                    dst: temp,
                    index: idx,
                    size: ty.size(),
                });
            }
        }

        // Build function body
        if let Some(body) = &func.body {
            self.build_block(body)?;
        }

        // Add implicit return if needed
        if func.return_type.is_void() {
            self.emit(Inst::Return(None));
        }

        let built_func = self.current_func.take()
            .ok_or_else(|| CompileError::codegen("No current function to finalize"))?;
        self.module.functions.push(built_func);

        Ok(())
    }

    fn build_block(&mut self, block: &Block) -> CompileResult<()> {
        for item in &block.items {
            match item {
                BlockItem::Statement(stmt) => self.build_stmt(stmt)?,
                BlockItem::Declaration(decl) => {
                    match &decl.kind {
                        DeclKind::Variable(var) => self.build_local_var(var)?,
                        DeclKind::MultipleVariables(vars) => {
                            for var in vars {
                                self.build_local_var(var)?;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn build_local_var(&mut self, var: &VarDecl) -> CompileResult<()> {
        let temp = self.new_temp();
        self.locals.insert(var.name.clone(), temp);

        // Allocate stack space
        let size = var.ty.size();
        let align = var.ty.alignment();
        self.emit(Inst::Alloca { dst: temp, size, align });

        // Handle initializer
        if let Some(init) = &var.init {
            if let Initializer::Expr(expr) = init {
                let value = self.build_expr(expr)?;
                self.emit(Inst::Store {
                    addr: Value::Temp(temp),
                    src: value,
                    size,
                    volatile: false,
                });
            }
        }

        Ok(())
    }

    fn build_stmt(&mut self, stmt: &Stmt) -> CompileResult<()> {
        match &stmt.kind {
            StmtKind::Expr(expr) => {
                self.build_expr(expr)?;
            }
            StmtKind::Empty => {}
            StmtKind::Block(block) => {
                self.build_block(block)?;
            }
            StmtKind::If { condition, then_branch, else_branch } => {
                self.build_if(condition, then_branch, else_branch.as_deref())?;
            }
            StmtKind::While { condition, body } => {
                self.build_while(condition, body)?;
            }
            StmtKind::DoWhile { body, condition } => {
                self.build_do_while(body, condition)?;
            }
            StmtKind::For { init, condition, update, body } => {
                self.build_for(init, condition, update, body)?;
            }
            StmtKind::Return(value) => {
                let val = if let Some(expr) = value {
                    Some(self.build_expr(expr)?)
                } else {
                    None
                };
                self.emit(Inst::Return(val));
            }
            StmtKind::Break => {
                if let Some(label) = &self.break_label {
                    self.emit(Inst::Jump(label.clone()));
                }
            }
            StmtKind::Continue => {
                if let Some(label) = &self.continue_label {
                    self.emit(Inst::Jump(label.clone()));
                }
            }
            StmtKind::Switch { expr, body } => {
                self.build_switch(expr, body)?;
            }
            StmtKind::Case { .. } | StmtKind::Default(_) => {
                // Handled inside switch
            }
            StmtKind::Goto(label) => {
                self.emit(Inst::Jump(Label(label.clone())));
            }
            StmtKind::Label { name, stmt } => {
                self.emit(Inst::Label(Label(name.clone())));
                self.build_stmt(stmt)?;
            }
            StmtKind::Declaration(decl) => {
                match &decl.kind {
                    DeclKind::Variable(var) => self.build_local_var(var)?,
                    DeclKind::MultipleVariables(vars) => {
                        for var in vars {
                            self.build_local_var(var)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn build_if(
        &mut self,
        condition: &Expr,
        then_branch: &Stmt,
        else_branch: Option<&Stmt>,
    ) -> CompileResult<()> {
        let cond = self.build_expr(condition)?;
        let else_label = self.new_label("else");
        let end_label = self.new_label("endif");

        self.emit(Inst::CondJumpFalse {
            cond,
            target: else_label.clone(),
        });

        self.build_stmt(then_branch)?;

        if else_branch.is_some() {
            self.emit(Inst::Jump(end_label.clone()));
        }

        self.emit(Inst::Label(else_label));

        if let Some(else_stmt) = else_branch {
            self.build_stmt(else_stmt)?;
            self.emit(Inst::Label(end_label));
        }

        Ok(())
    }

    fn build_while(&mut self, condition: &Expr, body: &Stmt) -> CompileResult<()> {
        let start_label = self.new_label("while");
        let end_label = self.new_label("endwhile");

        let old_break = self.break_label.replace(end_label.clone());
        let old_continue = self.continue_label.replace(start_label.clone());

        self.emit(Inst::Label(start_label.clone()));

        let cond = self.build_expr(condition)?;
        self.emit(Inst::CondJumpFalse {
            cond,
            target: end_label.clone(),
        });

        self.build_stmt(body)?;
        self.emit(Inst::Jump(start_label));
        self.emit(Inst::Label(end_label));

        self.break_label = old_break;
        self.continue_label = old_continue;

        Ok(())
    }

    fn build_do_while(&mut self, body: &Stmt, condition: &Expr) -> CompileResult<()> {
        let start_label = self.new_label("do");
        let cond_label = self.new_label("docond");
        let end_label = self.new_label("enddo");

        let old_break = self.break_label.replace(end_label.clone());
        let old_continue = self.continue_label.replace(cond_label.clone());

        self.emit(Inst::Label(start_label.clone()));
        self.build_stmt(body)?;

        self.emit(Inst::Label(cond_label));
        let cond = self.build_expr(condition)?;
        self.emit(Inst::CondJump {
            cond,
            target: start_label,
        });

        self.emit(Inst::Label(end_label));

        self.break_label = old_break;
        self.continue_label = old_continue;

        Ok(())
    }

    fn build_for(
        &mut self,
        init: &Option<ForInit>,
        condition: &Option<Expr>,
        update: &Option<Expr>,
        body: &Stmt,
    ) -> CompileResult<()> {
        // Init
        if let Some(init) = init {
            match init {
                ForInit::Expr(expr) => {
                    self.build_expr(expr)?;
                }
                ForInit::Declaration(decl) => {
                    match &decl.kind {
                        DeclKind::Variable(var) => self.build_local_var(var)?,
                        DeclKind::MultipleVariables(vars) => {
                            for var in vars {
                                self.build_local_var(var)?;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let start_label = self.new_label("for");
        let update_label = self.new_label("forupdate");
        let end_label = self.new_label("endfor");

        let old_break = self.break_label.replace(end_label.clone());
        let old_continue = self.continue_label.replace(update_label.clone());

        self.emit(Inst::Label(start_label.clone()));

        // Condition
        if let Some(cond_expr) = condition {
            let cond = self.build_expr(cond_expr)?;
            self.emit(Inst::CondJumpFalse {
                cond,
                target: end_label.clone(),
            });
        }

        // Body
        self.build_stmt(body)?;

        // Update
        self.emit(Inst::Label(update_label));
        if let Some(update_expr) = update {
            self.build_expr(update_expr)?;
        }

        self.emit(Inst::Jump(start_label));
        self.emit(Inst::Label(end_label));

        self.break_label = old_break;
        self.continue_label = old_continue;

        Ok(())
    }

    fn build_switch(&mut self, expr: &Expr, body: &Stmt) -> CompileResult<()> {
        // Evaluate switch expression
        let switch_val = self.build_expr(expr)?;
        let switch_temp = self.new_temp();
        self.emit(Inst::Copy { dst: switch_temp, src: switch_val });

        // Collect case labels and create jump targets
        let end_label = self.new_label("endswitch");
        let old_break = self.break_label.replace(end_label.clone());

        // First pass: collect all case values and create labels
        let mut cases: Vec<(i64, Label)> = Vec::new();
        let mut default_label: Option<Label> = None;
        self.collect_switch_cases(body, &mut cases, &mut default_label);

        // Generate comparison and jumps for each case
        for (value, label) in &cases {
            let cmp_temp = self.new_temp();
            self.emit(Inst::Binary {
                dst: cmp_temp,
                op: BinOp::Eq,
                left: Value::Temp(switch_temp),
                right: Value::IntConst(*value),
            });
            self.emit(Inst::CondJump {
                cond: Value::Temp(cmp_temp),
                target: label.clone(),
            });
        }

        // Jump to default or end
        if let Some(def_label) = &default_label {
            self.emit(Inst::Jump(def_label.clone()));
        } else {
            self.emit(Inst::Jump(end_label.clone()));
        }

        // Second pass: emit the switch body with labels
        self.emit_switch_body(body, &cases, &default_label)?;

        self.emit(Inst::Label(end_label));
        self.break_label = old_break;

        Ok(())
    }

    /// Collect all case values and create labels for them
    fn collect_switch_cases(
        &mut self,
        stmt: &Stmt,
        cases: &mut Vec<(i64, Label)>,
        default_label: &mut Option<Label>,
    ) {
        match &stmt.kind {
            StmtKind::Case { value, stmt: inner } => {
                if let Ok(val) = self.evaluate_const_expr(value) {
                    let label = self.new_label("case");
                    cases.push((val, label));
                }
                self.collect_switch_cases(inner, cases, default_label);
            }
            StmtKind::Default(inner) => {
                if default_label.is_none() {
                    *default_label = Some(self.new_label("default"));
                }
                self.collect_switch_cases(inner, cases, default_label);
            }
            StmtKind::Block(block) => {
                for item in &block.items {
                    if let BlockItem::Statement(s) = item {
                        self.collect_switch_cases(s, cases, default_label);
                    }
                }
            }
            _ => {}
        }
    }

    /// Emit switch body with case labels
    fn emit_switch_body(
        &mut self,
        stmt: &Stmt,
        cases: &[(i64, Label)],
        default_label: &Option<Label>,
    ) -> CompileResult<()> {
        match &stmt.kind {
            StmtKind::Case { value, stmt: inner } => {
                // Find the label for this case
                if let Ok(val) = self.evaluate_const_expr(value) {
                    if let Some((_, label)) = cases.iter().find(|(v, _)| *v == val) {
                        self.emit(Inst::Label(label.clone()));
                    }
                }
                self.emit_switch_body(inner, cases, default_label)?;
            }
            StmtKind::Default(inner) => {
                if let Some(def_label) = default_label {
                    self.emit(Inst::Label(def_label.clone()));
                }
                self.emit_switch_body(inner, cases, default_label)?;
            }
            StmtKind::Block(block) => {
                for item in &block.items {
                    match item {
                        BlockItem::Statement(s) => {
                            self.emit_switch_body(s, cases, default_label)?;
                        }
                        BlockItem::Declaration(decl) => {
                            match &decl.kind {
                                DeclKind::Variable(var) => self.build_local_var(var)?,
                                DeclKind::MultipleVariables(vars) => {
                                    for var in vars {
                                        self.build_local_var(var)?;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {
                self.build_stmt(stmt)?;
            }
        }
        Ok(())
    }

    fn build_expr(&mut self, expr: &Expr) -> CompileResult<Value> {
        match &expr.kind {
            ExprKind::IntLiteral(n) => Ok(Value::IntConst(*n)),

            ExprKind::FloatLiteral(_) => {
                // TODO: Handle floats
                Ok(Value::IntConst(0))
            }

            ExprKind::CharLiteral(c) => Ok(Value::IntConst(*c as i64)),

            ExprKind::StringLiteral(s) => {
                let label = Label(format!(".Lstr{}", self.string_counter));
                self.string_counter += 1;
                self.module.strings.push((label.clone(), s.clone()));
                Ok(Value::StringConst(label))
            }

            ExprKind::Identifier(name) => {
                if let Some(&temp) = self.locals.get(name) {
                    // Load from local variable
                    let dst = self.new_temp();
                    let size = expr.ty.as_ref().map(|t| t.size()).unwrap_or(4);
                    let signed = expr.ty.as_ref().map(|t| t.is_signed()).unwrap_or(false);
                    self.emit(Inst::Load {
                        dst,
                        addr: Value::Temp(temp),
                        size,
                        volatile: false,
                        signed,
                    });
                    Ok(Value::Temp(dst))
                } else {
                    // Global variable or function
                    // Arrays decay to pointers (their address) in expressions
                    if let Some(ty) = &expr.ty {
                        if ty.is_array() {
                            let dst = self.new_temp();
                            self.emit(Inst::AddrOf {
                                dst,
                                name: name.clone(),
                            });
                            return Ok(Value::Temp(dst));
                        }
                        // For non-array globals, get address then load with correct size
                        // (function references stay as Value::Name for call targets)
                        if !matches!(ty.kind, TypeKind::Function { .. }) {
                            let addr_temp = self.new_temp();
                            self.emit(Inst::AddrOf {
                                dst: addr_temp,
                                name: name.clone(),
                            });
                            let dst = self.new_temp();
                            self.emit(Inst::Load {
                                dst,
                                addr: Value::Temp(addr_temp),
                                size: ty.size(),
                                volatile: false,
                                signed: ty.is_signed(),
                            });
                            return Ok(Value::Temp(dst));
                        }
                    }
                    Ok(Value::Name(name.clone()))
                }
            }

            ExprKind::Binary { op, left, right } => {
                let l = self.build_expr(left)?;
                let r = self.build_expr(right)?;
                let dst = self.new_temp();

                // Check if either operand is unsigned (for div/mod)
                let is_unsigned = left.ty.as_ref().map(|t| !t.is_signed()).unwrap_or(false)
                    || right.ty.as_ref().map(|t| !t.is_signed()).unwrap_or(false);

                let ir_op = match op {
                    BinaryOp::Add => BinOp::Add,
                    BinaryOp::Sub => BinOp::Sub,
                    BinaryOp::Mul => BinOp::Mul,
                    BinaryOp::Div => if is_unsigned { BinOp::UDiv } else { BinOp::Div },
                    BinaryOp::Mod => if is_unsigned { BinOp::UMod } else { BinOp::Mod },
                    BinaryOp::BitAnd => BinOp::And,
                    BinaryOp::BitOr => BinOp::Or,
                    BinaryOp::BitXor => BinOp::Xor,
                    BinaryOp::Shl => BinOp::Shl,
                    BinaryOp::Shr => BinOp::Shr,
                    BinaryOp::Eq => BinOp::Eq,
                    BinaryOp::Ne => BinOp::Ne,
                    BinaryOp::Lt => BinOp::Lt,
                    BinaryOp::Le => BinOp::Le,
                    BinaryOp::Gt => BinOp::Gt,
                    BinaryOp::Ge => BinOp::Ge,
                    BinaryOp::LogAnd | BinaryOp::LogOr => {
                        // Short-circuit evaluation
                        return self.build_logical_expr(op, left, right);
                    }
                };

                self.emit(Inst::Binary {
                    dst,
                    op: ir_op,
                    left: l,
                    right: r,
                });

                Ok(Value::Temp(dst))
            }

            ExprKind::Unary { op, operand } => {
                let src = self.build_expr(operand)?;
                let dst = self.new_temp();

                let ir_op = match op {
                    UnaryOp::Neg => UnOp::Neg,
                    UnaryOp::Not => UnOp::Not,
                    UnaryOp::BitNot => UnOp::BitNot,
                };

                self.emit(Inst::Unary { dst, op: ir_op, src });
                Ok(Value::Temp(dst))
            }

            ExprKind::Assign { op, target, value } => {
                let val = self.build_expr(value)?;

                // Get the address of the target
                let addr = self.build_lvalue(target)?;

                // Handle compound assignment
                let final_val = if let Some(bin_op) = op.to_binary_op() {
                    let old_val = self.new_temp();
                    let size = target.ty.as_ref().map(|t| t.size()).unwrap_or(4);
                    let signed = target.ty.as_ref().map(|t| t.is_signed()).unwrap_or(false);
                    self.emit(Inst::Load {
                        dst: old_val,
                        addr: addr.clone(),
                        size,
                        volatile: false,
                        signed,
                    });

                    // Check if unsigned for div/mod operations
                    let is_unsigned = !signed;

                    let ir_op = match bin_op {
                        BinaryOp::Add => BinOp::Add,
                        BinaryOp::Sub => BinOp::Sub,
                        BinaryOp::Mul => BinOp::Mul,
                        BinaryOp::Div => if is_unsigned { BinOp::UDiv } else { BinOp::Div },
                        BinaryOp::Mod => if is_unsigned { BinOp::UMod } else { BinOp::Mod },
                        BinaryOp::BitAnd => BinOp::And,
                        BinaryOp::BitOr => BinOp::Or,
                        BinaryOp::BitXor => BinOp::Xor,
                        BinaryOp::Shl => BinOp::Shl,
                        BinaryOp::Shr => BinOp::Shr,
                        _ => unreachable!(),
                    };

                    let result = self.new_temp();
                    self.emit(Inst::Binary {
                        dst: result,
                        op: ir_op,
                        left: Value::Temp(old_val),
                        right: val,
                    });
                    Value::Temp(result)
                } else {
                    val
                };

                let size = target.ty.as_ref().map(|t| t.size()).unwrap_or(4);
                self.emit(Inst::Store {
                    addr,
                    src: final_val.clone(),
                    size,
                    volatile: false,
                });

                Ok(final_val)
            }

            ExprKind::Call { callee, args } => {
                let mut ir_args = Vec::new();
                for arg in args {
                    ir_args.push(self.build_expr(arg)?);
                }

                let func_name = if let ExprKind::Identifier(name) = &callee.kind {
                    name.clone()
                } else {
                    return Err(CompileError::codegen("indirect calls not supported yet"));
                };

                let dst = self.new_temp();
                self.emit(Inst::Call {
                    dst: Some(dst),
                    func: func_name,
                    args: ir_args,
                });

                Ok(Value::Temp(dst))
            }

            ExprKind::Index { array, index } => {
                let base = self.build_expr(array)?;
                let idx = self.build_expr(index)?;

                // Calculate offset: base + index * element_size
                let (elem_size, elem_signed) = array
                    .ty
                    .as_ref()
                    .and_then(|t| match &t.kind {
                        TypeKind::Array { element, .. } => Some((element.size(), element.is_signed())),
                        TypeKind::Pointer(inner) => Some((inner.size(), inner.is_signed())),
                        _ => None,
                    })
                    .unwrap_or((4, false));

                let offset = self.new_temp();
                self.emit(Inst::Binary {
                    dst: offset,
                    op: BinOp::Mul,
                    left: idx,
                    right: Value::IntConst(elem_size as i64),
                });

                let addr = self.new_temp();
                self.emit(Inst::Binary {
                    dst: addr,
                    op: BinOp::Add,
                    left: base,
                    right: Value::Temp(offset),
                });

                let dst = self.new_temp();
                self.emit(Inst::Load {
                    dst,
                    addr: Value::Temp(addr),
                    size: elem_size,
                    volatile: false,
                    signed: elem_signed,
                });

                Ok(Value::Temp(dst))
            }

            ExprKind::AddrOf(operand) => {
                self.build_lvalue(operand)
            }

            ExprKind::Deref(operand) => {
                let addr = self.build_expr(operand)?;
                let dst = self.new_temp();
                let size = expr.ty.as_ref().map(|t| t.size()).unwrap_or(4);
                let signed = expr.ty.as_ref().map(|t| t.is_signed()).unwrap_or(false);
                // Check if pointer type is volatile
                let is_volatile = operand.ty.as_ref()
                    .map(|t| t.qualifiers.is_volatile)
                    .unwrap_or(false);
                self.emit(Inst::Load { dst, addr, size, volatile: is_volatile, signed });
                Ok(Value::Temp(dst))
            }

            ExprKind::PreIncrement(operand) | ExprKind::PreDecrement(operand) => {
                let addr = self.build_lvalue(operand)?;
                let size = operand.ty.as_ref().map(|t| t.size()).unwrap_or(4);
                let signed = operand.ty.as_ref().map(|t| t.is_signed()).unwrap_or(false);
                let is_volatile = operand.ty.as_ref()
                    .map(|t| t.qualifiers.is_volatile)
                    .unwrap_or(false);

                let old = self.new_temp();
                self.emit(Inst::Load {
                    dst: old,
                    addr: addr.clone(),
                    size,
                    volatile: is_volatile,
                    signed,
                });

                let op = if matches!(expr.kind, ExprKind::PreIncrement(_)) {
                    BinOp::Add
                } else {
                    BinOp::Sub
                };

                let new_val = self.new_temp();
                self.emit(Inst::Binary {
                    dst: new_val,
                    op,
                    left: Value::Temp(old),
                    right: Value::IntConst(1),
                });

                self.emit(Inst::Store {
                    addr,
                    src: Value::Temp(new_val),
                    size,
                    volatile: is_volatile,
                });

                Ok(Value::Temp(new_val))
            }

            ExprKind::PostIncrement(operand) | ExprKind::PostDecrement(operand) => {
                let addr = self.build_lvalue(operand)?;
                let size = operand.ty.as_ref().map(|t| t.size()).unwrap_or(4);
                let signed = operand.ty.as_ref().map(|t| t.is_signed()).unwrap_or(false);
                let is_volatile = operand.ty.as_ref()
                    .map(|t| t.qualifiers.is_volatile)
                    .unwrap_or(false);

                let old = self.new_temp();
                self.emit(Inst::Load {
                    dst: old,
                    addr: addr.clone(),
                    size,
                    volatile: is_volatile,
                    signed,
                });

                let op = if matches!(expr.kind, ExprKind::PostIncrement(_)) {
                    BinOp::Add
                } else {
                    BinOp::Sub
                };

                let new_val = self.new_temp();
                self.emit(Inst::Binary {
                    dst: new_val,
                    op,
                    left: Value::Temp(old),
                    right: Value::IntConst(1),
                });

                self.emit(Inst::Store {
                    addr,
                    src: Value::Temp(new_val),
                    size,
                    volatile: is_volatile,
                });

                Ok(Value::Temp(old)) // Return old value
            }

            ExprKind::Ternary { condition, then_expr, else_expr } => {
                let cond = self.build_expr(condition)?;
                let else_label = self.new_label("ternelse");
                let end_label = self.new_label("ternend");
                let result = self.new_temp();

                self.emit(Inst::CondJumpFalse {
                    cond,
                    target: else_label.clone(),
                });

                let then_val = self.build_expr(then_expr)?;
                self.emit(Inst::Copy {
                    dst: result,
                    src: then_val,
                });
                self.emit(Inst::Jump(end_label.clone()));

                self.emit(Inst::Label(else_label));
                let else_val = self.build_expr(else_expr)?;
                self.emit(Inst::Copy {
                    dst: result,
                    src: else_val,
                });

                self.emit(Inst::Label(end_label));
                Ok(Value::Temp(result))
            }

            ExprKind::Cast { expr: inner, .. } => {
                // For now, just pass through (proper casts need type info)
                self.build_expr(inner)
            }

            ExprKind::Sizeof(arg) => {
                let size = match arg {
                    SizeofArg::Type(ty) => ty.size(),
                    SizeofArg::Expr(e) => e.ty.as_ref().map(|t| t.size()).unwrap_or(4),
                };
                Ok(Value::IntConst(size as i64))
            }

            ExprKind::Member { object, field } => {
                // Get the address of the struct object
                let base_addr = self.build_lvalue(object)?;

                // Get struct type and find the field offset
                let (offset, field_ty) = self.get_struct_field_offset(object, field)?;

                // Calculate field address
                let field_addr = self.new_temp();
                self.emit(Inst::Binary {
                    dst: field_addr,
                    op: BinOp::Add,
                    left: base_addr,
                    right: Value::IntConst(offset as i64),
                });

                // Load the field value
                let dst = self.new_temp();
                self.emit(Inst::Load {
                    dst,
                    addr: Value::Temp(field_addr),
                    size: field_ty.size(),
                    volatile: false,
                    signed: field_ty.is_signed(),
                });
                Ok(Value::Temp(dst))
            }

            ExprKind::PtrMember { pointer, field } => {
                // The pointer value is the address of the struct
                let base_addr = self.build_expr(pointer)?;

                // Get the pointed-to struct type and find the field offset
                let (offset, field_ty) = self.get_ptr_struct_field_offset(pointer, field)?;

                // Calculate field address
                let field_addr = self.new_temp();
                self.emit(Inst::Binary {
                    dst: field_addr,
                    op: BinOp::Add,
                    left: base_addr,
                    right: Value::IntConst(offset as i64),
                });

                // Load the field value
                let dst = self.new_temp();
                self.emit(Inst::Load {
                    dst,
                    addr: Value::Temp(field_addr),
                    size: field_ty.size(),
                    volatile: false,
                    signed: field_ty.is_signed(),
                });
                Ok(Value::Temp(dst))
            }

            ExprKind::Comma(exprs) => {
                let mut last = Value::IntConst(0);
                for e in exprs {
                    last = self.build_expr(e)?;
                }
                Ok(last)
            }

            ExprKind::CompoundLiteral { .. } => {
                // TODO: Compound literals
                Ok(Value::IntConst(0))
            }
        }
    }

    fn build_lvalue(&mut self, expr: &Expr) -> CompileResult<Value> {
        match &expr.kind {
            ExprKind::Identifier(name) => {
                if let Some(&temp) = self.locals.get(name) {
                    Ok(Value::Temp(temp))
                } else {
                    let dst = self.new_temp();
                    self.emit(Inst::AddrOf {
                        dst,
                        name: name.clone(),
                    });
                    Ok(Value::Temp(dst))
                }
            }
            ExprKind::Deref(inner) => {
                self.build_expr(inner)
            }
            ExprKind::Index { array, index } => {
                let base = self.build_expr(array)?;
                let idx = self.build_expr(index)?;

                let elem_size = array
                    .ty
                    .as_ref()
                    .and_then(|t| match &t.kind {
                        TypeKind::Array { element, .. } => Some(element.size()),
                        TypeKind::Pointer(inner) => Some(inner.size()),
                        _ => None,
                    })
                    .unwrap_or(4);

                let offset = self.new_temp();
                self.emit(Inst::Binary {
                    dst: offset,
                    op: BinOp::Mul,
                    left: idx,
                    right: Value::IntConst(elem_size as i64),
                });

                let addr = self.new_temp();
                self.emit(Inst::Binary {
                    dst: addr,
                    op: BinOp::Add,
                    left: base,
                    right: Value::Temp(offset),
                });

                Ok(Value::Temp(addr))
            }
            ExprKind::Member { object, field } => {
                // Get the address of the struct object
                let base_addr = self.build_lvalue(object)?;

                // Get struct field offset
                let (offset, _field_ty) = self.get_struct_field_offset(object, field)?;

                // Calculate field address
                let field_addr = self.new_temp();
                self.emit(Inst::Binary {
                    dst: field_addr,
                    op: BinOp::Add,
                    left: base_addr,
                    right: Value::IntConst(offset as i64),
                });

                Ok(Value::Temp(field_addr))
            }
            ExprKind::PtrMember { pointer, field } => {
                // The pointer value is the address of the struct
                let base_addr = self.build_expr(pointer)?;

                // Get struct field offset from pointer type
                let (offset, _field_ty) = self.get_ptr_struct_field_offset(pointer, field)?;

                // Calculate field address
                let field_addr = self.new_temp();
                self.emit(Inst::Binary {
                    dst: field_addr,
                    op: BinOp::Add,
                    left: base_addr,
                    right: Value::IntConst(offset as i64),
                });

                Ok(Value::Temp(field_addr))
            }
            _ => Err(CompileError::codegen("invalid lvalue")),
        }
    }

    fn build_logical_expr(
        &mut self,
        op: &BinaryOp,
        left: &Expr,
        right: &Expr,
    ) -> CompileResult<Value> {
        let result = self.new_temp();
        let short_circuit = self.new_label("shortcircuit");
        let end_label = self.new_label("logend");

        let l = self.build_expr(left)?;

        match op {
            BinaryOp::LogAnd => {
                // If left is false, result is 0
                self.emit(Inst::CondJumpFalse {
                    cond: l,
                    target: short_circuit.clone(),
                });
                let r = self.build_expr(right)?;
                // Result is right != 0
                let cmp = self.new_temp();
                self.emit(Inst::Binary {
                    dst: cmp,
                    op: BinOp::Ne,
                    left: r,
                    right: Value::IntConst(0),
                });
                self.emit(Inst::Copy {
                    dst: result,
                    src: Value::Temp(cmp),
                });
                self.emit(Inst::Jump(end_label.clone()));

                self.emit(Inst::Label(short_circuit));
                self.emit(Inst::Copy {
                    dst: result,
                    src: Value::IntConst(0),
                });
            }
            BinaryOp::LogOr => {
                // If left is true, result is 1
                self.emit(Inst::CondJump {
                    cond: l,
                    target: short_circuit.clone(),
                });
                let r = self.build_expr(right)?;
                let cmp = self.new_temp();
                self.emit(Inst::Binary {
                    dst: cmp,
                    op: BinOp::Ne,
                    left: r,
                    right: Value::IntConst(0),
                });
                self.emit(Inst::Copy {
                    dst: result,
                    src: Value::Temp(cmp),
                });
                self.emit(Inst::Jump(end_label.clone()));

                self.emit(Inst::Label(short_circuit));
                self.emit(Inst::Copy {
                    dst: result,
                    src: Value::IntConst(1),
                });
            }
            _ => unreachable!(),
        }

        self.emit(Inst::Label(end_label));
        Ok(Value::Temp(result))
    }
}

impl Default for IrBuilder {
    fn default() -> Self {
        Self::new()
    }
}
