//! Lower Rust AST to MIR

use crate::common::CompileResult;
use crate::frontend::rust::ast::*;
use super::types::*;
use std::collections::HashMap;

/// Lowers Rust AST to MIR
pub struct MirLowerer {
    /// Current MIR body being built
    body: MirBody,
    /// Current block
    current_block: BlockId,
    /// Variable name to local ID mapping
    locals_map: HashMap<String, LocalId>,
    /// Break targets for loops
    break_targets: Vec<BlockId>,
    /// Continue targets for loops
    continue_targets: Vec<BlockId>,
}

impl MirLowerer {
    pub fn new(return_type: RustType) -> Self {
        let mut body = MirBody::new(return_type.clone());
        let entry = body.add_block();

        // Local 0 is always the return value
        body.add_local(return_type, Some("_return".to_string()));

        Self {
            body,
            current_block: entry,
            locals_map: HashMap::new(),
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
        }
    }

    /// Lower a function declaration to MIR
    pub fn lower_function(mut self, func: &FnDecl) -> CompileResult<MirBody> {
        // Add parameters as locals
        for param in &func.params {
            if let Some(name) = self.pattern_name(&param.pattern) {
                let local = self.body.add_local(param.ty.clone(), Some(name.clone()));
                self.locals_map.insert(name, local);
            }
        }

        // Lower body
        if let Some(body) = &func.body {
            let result = self.lower_block(body)?;

            // Store result in return local if there is a value
            if let Some(result) = result {
                self.emit_assign(
                    Place::local(LocalId(0)),
                    Rvalue::Use(result),
                );
            }

            self.terminate(MirTerminator::Return);
        }

        Ok(self.body)
    }

    fn lower_block(&mut self, block: &Block) -> CompileResult<Option<Operand>> {
        for stmt in &block.stmts {
            self.lower_stmt(stmt)?;
        }

        if let Some(expr) = &block.expr {
            Ok(Some(self.lower_expr(expr)?))
        } else {
            Ok(None)
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt) -> CompileResult<()> {
        match &stmt.kind {
            StmtKind::Let { pattern, ty, init } => {
                let local_ty = ty.clone().unwrap_or_else(|| {
                    init.as_ref()
                        .and_then(|e| e.ty.clone())
                        .unwrap_or_else(|| RustType::new(RustTypeKind::Infer, stmt.span))
                });

                if let Some(name) = self.pattern_name(pattern) {
                    let local = self.body.add_local(local_ty, Some(name.clone()));
                    self.locals_map.insert(name, local);

                    if let Some(init) = init {
                        let value = self.lower_expr(init)?;
                        self.emit_assign(Place::local(local), Rvalue::Use(value));
                    }
                }
            }
            StmtKind::Expr(expr) | StmtKind::ExprNoSemi(expr) => {
                self.lower_expr(expr)?;
            }
            StmtKind::Item(_) => {
                // Items in blocks are handled separately
            }
            StmtKind::Empty => {}
        }
        Ok(())
    }

    fn lower_expr(&mut self, expr: &Expr) -> CompileResult<Operand> {
        match &expr.kind {
            ExprKind::IntLiteral(value) => {
                Ok(Operand::Constant(MirConstant::Int(*value)))
            }
            ExprKind::FloatLiteral(value) => {
                Ok(Operand::Constant(MirConstant::Float(*value)))
            }
            ExprKind::BoolLiteral(value) => {
                Ok(Operand::Constant(MirConstant::Bool(*value)))
            }
            ExprKind::CharLiteral(value) => {
                Ok(Operand::Constant(MirConstant::Char(*value)))
            }
            ExprKind::StringLiteral(value) => {
                Ok(Operand::Constant(MirConstant::String(value.clone())))
            }
            ExprKind::ByteLiteral(value) => {
                Ok(Operand::Constant(MirConstant::Int(*value as i64)))
            }
            ExprKind::ByteStringLiteral(bytes) => {
                // Store as string for now
                let s = String::from_utf8_lossy(bytes).to_string();
                Ok(Operand::Constant(MirConstant::String(s)))
            }
            ExprKind::Identifier(name) => {
                if let Some(&local) = self.locals_map.get(name) {
                    Ok(Operand::Copy(Place::local(local)))
                } else {
                    // Might be a function or constant
                    Ok(Operand::Constant(MirConstant::Function(name.clone())))
                }
            }
            ExprKind::Path(path) => {
                Ok(Operand::Constant(MirConstant::Function(path.name().to_string())))
            }
            ExprKind::Binary { op, left, right } => {
                let left_op = self.lower_expr(left)?;
                let right_op = self.lower_expr(right)?;

                let mir_op = self.convert_bin_op(*op);
                let ty = expr.ty.clone().unwrap_or_else(|| RustType::i32(expr.span));
                let result = self.new_temp(ty);

                self.emit_assign(
                    Place::local(result),
                    Rvalue::BinaryOp {
                        op: mir_op,
                        left: left_op,
                        right: right_op,
                    },
                );

                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::Unary { op, operand } => {
                let operand_value = self.lower_expr(operand)?;

                match op {
                    UnaryOp::Deref => {
                        if let Operand::Copy(place) | Operand::Move(place) = operand_value {
                            Ok(Operand::Copy(place.deref()))
                        } else {
                            let ty = expr.ty.clone().unwrap_or_else(|| RustType::i32(expr.span));
                            let temp = self.new_temp(ty);
                            self.emit_assign(Place::local(temp), Rvalue::Use(operand_value));
                            Ok(Operand::Copy(Place::local(temp).deref()))
                        }
                    }
                    UnaryOp::Ref | UnaryOp::RefMut => {
                        if let Operand::Copy(place) | Operand::Move(place) = operand_value {
                            let ty = expr.ty.clone().unwrap_or_else(|| RustType::i32(expr.span));
                            let result = self.new_temp(ty);
                            self.emit_assign(
                                Place::local(result),
                                Rvalue::Ref {
                                    mutable: matches!(op, UnaryOp::RefMut),
                                    place,
                                },
                            );
                            Ok(Operand::Copy(Place::local(result)))
                        } else {
                            Ok(operand_value)
                        }
                    }
                    UnaryOp::Neg | UnaryOp::Not => {
                        let mir_op = match op {
                            UnaryOp::Neg => MirUnaryOp::Neg,
                            UnaryOp::Not => MirUnaryOp::Not,
                            _ => unreachable!(),
                        };
                        let ty = expr.ty.clone().unwrap_or_else(|| RustType::i32(expr.span));
                        let result = self.new_temp(ty);
                        self.emit_assign(
                            Place::local(result),
                            Rvalue::UnaryOp { op: mir_op, operand: operand_value },
                        );
                        Ok(Operand::Copy(Place::local(result)))
                    }
                }
            }
            ExprKind::Assign { target, op, value } => {
                let value_op = self.lower_expr(value)?;
                let target_place = self.lower_place(target)?;

                if let Some(bin_op) = op {
                    // Compound assignment: target op= value
                    let old_value = Operand::Copy(target_place.clone());
                    let mir_op = self.convert_bin_op(*bin_op);
                    let ty = expr.ty.clone().unwrap_or_else(|| RustType::i32(expr.span));
                    let temp = self.new_temp(ty);
                    self.emit_assign(
                        Place::local(temp),
                        Rvalue::BinaryOp {
                            op: mir_op,
                            left: old_value,
                            right: value_op,
                        },
                    );
                    self.emit_assign(target_place, Rvalue::Use(Operand::Copy(Place::local(temp))));
                } else {
                    self.emit_assign(target_place, Rvalue::Use(value_op));
                }

                Ok(Operand::Constant(MirConstant::Unit))
            }
            ExprKind::Call { callee, args } => {
                let func = self.lower_expr(callee)?;
                let args: Vec<_> = args.iter()
                    .map(|a| self.lower_expr(a))
                    .collect::<CompileResult<_>>()?;

                let ty = expr.ty.clone().unwrap_or_else(|| RustType::unit(expr.span));
                let result = self.new_temp(ty);
                let next_block = self.body.add_block();

                self.terminate(MirTerminator::Call {
                    func,
                    args,
                    dest: Place::local(result),
                    target: next_block,
                });

                self.current_block = next_block;
                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::MethodCall { receiver, method, args } => {
                // Desugar to regular call
                let receiver_op = self.lower_expr(receiver)?;
                let mut all_args = vec![receiver_op];
                for arg in args {
                    all_args.push(self.lower_expr(arg)?);
                }

                let ty = expr.ty.clone().unwrap_or_else(|| RustType::unit(expr.span));
                let result = self.new_temp(ty);
                let next_block = self.body.add_block();

                self.terminate(MirTerminator::Call {
                    func: Operand::Constant(MirConstant::Function(method.clone())),
                    args: all_args,
                    dest: Place::local(result),
                    target: next_block,
                });

                self.current_block = next_block;
                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::Field { object, field: _ } => {
                let obj = self.lower_place(object)?;
                // We'd need field index from type info
                Ok(Operand::Copy(obj.field(0))) // Simplified
            }
            ExprKind::TupleField { object, index } => {
                let obj = self.lower_place(object)?;
                Ok(Operand::Copy(obj.field(*index)))
            }
            ExprKind::Index { object, index } => {
                let obj = self.lower_place(object)?;
                let idx = self.lower_expr(index)?;
                Ok(Operand::Copy(obj.index(idx)))
            }
            ExprKind::Reference { mutable, operand } => {
                let place = self.lower_place(operand)?;
                let ty = expr.ty.clone().unwrap_or_else(|| RustType::i32(expr.span));
                let result = self.new_temp(ty);
                self.emit_assign(
                    Place::local(result),
                    Rvalue::Ref { mutable: *mutable, place },
                );
                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::Dereference(operand) => {
                let place = self.lower_place(operand)?;
                Ok(Operand::Copy(place.deref()))
            }
            ExprKind::Cast { expr: inner, ty } => {
                let operand = self.lower_expr(inner)?;
                let result = self.new_temp(ty.clone());
                self.emit_assign(
                    Place::local(result),
                    Rvalue::Cast { operand, ty: ty.clone() },
                );
                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::Block(block) => {
                let result = self.lower_block(block)?;
                Ok(result.unwrap_or(Operand::Constant(MirConstant::Unit)))
            }
            ExprKind::If { condition, then_block, else_block } => {
                let cond = self.lower_expr(condition)?;

                let then_bb = self.body.add_block();
                let else_bb = self.body.add_block();
                let merge_bb = self.body.add_block();

                let ty = expr.ty.clone().unwrap_or_else(|| RustType::unit(expr.span));
                let result = self.new_temp(ty);

                self.terminate(MirTerminator::If {
                    condition: cond,
                    then_block: then_bb,
                    else_block: else_bb,
                });

                // Then branch
                self.current_block = then_bb;
                let then_result = self.lower_block(then_block)?;
                if let Some(val) = then_result {
                    self.emit_assign(Place::local(result), Rvalue::Use(val));
                }
                self.terminate(MirTerminator::Goto(merge_bb));

                // Else branch
                self.current_block = else_bb;
                if let Some(else_expr) = else_block {
                    let else_result = self.lower_expr(else_expr)?;
                    self.emit_assign(Place::local(result), Rvalue::Use(else_result));
                } else {
                    self.emit_assign(
                        Place::local(result),
                        Rvalue::Use(Operand::Constant(MirConstant::Unit)),
                    );
                }
                self.terminate(MirTerminator::Goto(merge_bb));

                self.current_block = merge_bb;
                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::Loop { body, .. } => {
                let loop_bb = self.body.add_block();
                let exit_bb = self.body.add_block();

                let ty = expr.ty.clone().unwrap_or_else(|| RustType::never(expr.span));
                let result = self.new_temp(ty);

                self.break_targets.push(exit_bb);
                self.continue_targets.push(loop_bb);

                self.terminate(MirTerminator::Goto(loop_bb));

                self.current_block = loop_bb;
                self.lower_block(body)?;
                self.terminate(MirTerminator::Goto(loop_bb));

                self.break_targets.pop();
                self.continue_targets.pop();

                self.current_block = exit_bb;
                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::While { condition, body, .. } => {
                let cond_bb = self.body.add_block();
                let body_bb = self.body.add_block();
                let exit_bb = self.body.add_block();

                self.break_targets.push(exit_bb);
                self.continue_targets.push(cond_bb);

                self.terminate(MirTerminator::Goto(cond_bb));

                self.current_block = cond_bb;
                let cond = self.lower_expr(condition)?;
                self.terminate(MirTerminator::If {
                    condition: cond,
                    then_block: body_bb,
                    else_block: exit_bb,
                });

                self.current_block = body_bb;
                self.lower_block(body)?;
                self.terminate(MirTerminator::Goto(cond_bb));

                self.break_targets.pop();
                self.continue_targets.pop();

                self.current_block = exit_bb;
                Ok(Operand::Constant(MirConstant::Unit))
            }
            ExprKind::For { pattern, iter, body, .. } => {
                // Simplified: desugar to while loop over iterator
                let _iter_val = self.lower_expr(iter)?;

                let cond_bb = self.body.add_block();
                let body_bb = self.body.add_block();
                let exit_bb = self.body.add_block();

                self.break_targets.push(exit_bb);
                self.continue_targets.push(cond_bb);

                // Add loop variable
                if let Some(name) = self.pattern_name(pattern) {
                    let elem_ty = RustType::new(RustTypeKind::Infer, expr.span);
                    let local = self.body.add_local(elem_ty, Some(name.clone()));
                    self.locals_map.insert(name, local);
                }

                self.terminate(MirTerminator::Goto(cond_bb));

                self.current_block = cond_bb;
                // Would need actual iterator protocol here
                self.terminate(MirTerminator::Goto(body_bb));

                self.current_block = body_bb;
                self.lower_block(body)?;
                self.terminate(MirTerminator::Goto(cond_bb));

                self.break_targets.pop();
                self.continue_targets.pop();

                self.current_block = exit_bb;
                Ok(Operand::Constant(MirConstant::Unit))
            }
            ExprKind::Match { scrutinee, arms } => {
                let scrutinee_val = self.lower_expr(scrutinee)?;

                let ty = expr.ty.clone().unwrap_or_else(|| RustType::unit(expr.span));
                let result = self.new_temp(ty);
                let exit_bb = self.body.add_block();

                // Simplified: generate a switch for integer patterns
                let mut targets = Vec::new();
                let mut default_block = None;

                for arm in arms {
                    let arm_bb = self.body.add_block();

                    // Check pattern type
                    match &arm.pattern.kind {
                        PatternKind::Literal(lit_expr) => {
                            if let ExprKind::IntLiteral(value) = &(**lit_expr).kind {
                                targets.push((*value, arm_bb));
                            }
                        }
                        PatternKind::Wildcard => {
                            default_block = Some(arm_bb);
                        }
                        PatternKind::Binding { .. } => {
                            default_block = Some(arm_bb);
                        }
                        _ => {
                            // Other patterns need more complex handling
                            if default_block.is_none() {
                                default_block = Some(arm_bb);
                            }
                        }
                    }
                }

                let default = default_block.unwrap_or(exit_bb);
                self.terminate(MirTerminator::Switch {
                    value: scrutinee_val.clone(),
                    targets,
                    default,
                });

                // Generate arm bodies
                for (i, arm) in arms.iter().enumerate() {
                    let arm_bb = if i < self.body.blocks.len() {
                        BlockId(self.current_block.0 + 1 + i)
                    } else {
                        continue;
                    };

                    self.current_block = arm_bb;

                    // Bind pattern variables
                    if let PatternKind::Binding { name, .. } = &arm.pattern.kind {
                        if let Some(&local) = self.locals_map.get(name) {
                            self.emit_assign(
                                Place::local(local),
                                Rvalue::Use(scrutinee_val.clone()),
                            );
                        }
                    }

                    let arm_result = self.lower_expr(&arm.body)?;
                    self.emit_assign(Place::local(result), Rvalue::Use(arm_result));
                    self.terminate(MirTerminator::Goto(exit_bb));
                }

                self.current_block = exit_bb;
                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::Break { value, .. } => {
                if let Some(exit_bb) = self.break_targets.last().copied() {
                    if let Some(value) = value {
                        let _val = self.lower_expr(value)?;
                        // Store in result if available
                    }
                    self.terminate(MirTerminator::Goto(exit_bb));
                }
                Ok(Operand::Constant(MirConstant::Unit))
            }
            ExprKind::Continue { .. } => {
                if let Some(continue_bb) = self.continue_targets.last().copied() {
                    self.terminate(MirTerminator::Goto(continue_bb));
                }
                Ok(Operand::Constant(MirConstant::Unit))
            }
            ExprKind::Return(value) => {
                if let Some(value) = value {
                    let val = self.lower_expr(value)?;
                    self.emit_assign(Place::local(LocalId(0)), Rvalue::Use(val));
                }
                self.terminate(MirTerminator::Return);
                Ok(Operand::Constant(MirConstant::Unit))
            }
            ExprKind::Tuple(exprs) => {
                let operands: Vec<_> = exprs.iter()
                    .map(|e| self.lower_expr(e))
                    .collect::<CompileResult<_>>()?;

                let ty = expr.ty.clone().unwrap_or_else(|| RustType::unit(expr.span));
                let result = self.new_temp(ty);
                self.emit_assign(
                    Place::local(result),
                    Rvalue::Aggregate {
                        kind: AggregateKind::Tuple,
                        operands,
                    },
                );
                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::Array(exprs) => {
                let operands: Vec<_> = exprs.iter()
                    .map(|e| self.lower_expr(e))
                    .collect::<CompileResult<_>>()?;

                let ty = expr.ty.clone().unwrap_or_else(|| RustType::unit(expr.span));
                let result = self.new_temp(ty);
                self.emit_assign(
                    Place::local(result),
                    Rvalue::Aggregate {
                        kind: AggregateKind::Array,
                        operands,
                    },
                );
                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::Struct { path, fields, rest: _ } => {
                let mut operands = Vec::new();
                for field in fields {
                    if let Some(value) = &field.value {
                        operands.push(self.lower_expr(value)?);
                    } else {
                        // Shorthand: use variable with same name
                        if let Some(&local) = self.locals_map.get(&field.name) {
                            operands.push(Operand::Copy(Place::local(local)));
                        }
                    }
                }

                let ty = expr.ty.clone().unwrap_or_else(|| RustType::unit(expr.span));
                let result = self.new_temp(ty);
                self.emit_assign(
                    Place::local(result),
                    Rvalue::Aggregate {
                        kind: AggregateKind::Struct(path.name().to_string()),
                        operands,
                    },
                );
                Ok(Operand::Copy(Place::local(result)))
            }
            ExprKind::Unsafe(block) => {
                let result = self.lower_block(block)?;
                Ok(result.unwrap_or(Operand::Constant(MirConstant::Unit)))
            }
            ExprKind::Paren(inner) => {
                self.lower_expr(inner)
            }
            _ => {
                // For unhandled cases, return unit
                Ok(Operand::Constant(MirConstant::Unit))
            }
        }
    }

    fn lower_place(&mut self, expr: &Expr) -> CompileResult<Place> {
        match &expr.kind {
            ExprKind::Identifier(name) => {
                if let Some(&local) = self.locals_map.get(name) {
                    Ok(Place::local(local))
                } else {
                    // Create a temporary
                    let ty = expr.ty.clone().unwrap_or_else(|| RustType::i32(expr.span));
                    let local = self.new_temp(ty);
                    Ok(Place::local(local))
                }
            }
            ExprKind::Dereference(inner) => {
                let place = self.lower_place(inner)?;
                Ok(place.deref())
            }
            ExprKind::Field { object, .. } => {
                let place = self.lower_place(object)?;
                Ok(place.field(0)) // Simplified
            }
            ExprKind::TupleField { object, index } => {
                let place = self.lower_place(object)?;
                Ok(place.field(*index))
            }
            ExprKind::Index { object, index } => {
                let place = self.lower_place(object)?;
                let idx = self.lower_expr(index)?;
                Ok(place.index(idx))
            }
            _ => {
                // For expressions, create a temp and store the value
                let value = self.lower_expr(expr)?;
                let ty = expr.ty.clone().unwrap_or_else(|| RustType::i32(expr.span));
                let local = self.new_temp(ty);
                self.emit_assign(Place::local(local), Rvalue::Use(value));
                Ok(Place::local(local))
            }
        }
    }

    fn new_temp(&mut self, ty: RustType) -> LocalId {
        self.body.add_local(ty, None)
    }

    fn emit_assign(&mut self, dest: Place, value: Rvalue) {
        self.body.block_mut(self.current_block).push(MirStatement::Assign { dest, value });
    }

    fn terminate(&mut self, term: MirTerminator) {
        self.body.block_mut(self.current_block).terminate(term);
    }

    fn convert_bin_op(&self, op: BinOp) -> MirBinOp {
        match op {
            BinOp::Add => MirBinOp::Add,
            BinOp::Sub => MirBinOp::Sub,
            BinOp::Mul => MirBinOp::Mul,
            BinOp::Div => MirBinOp::Div,
            BinOp::Rem => MirBinOp::Rem,
            BinOp::BitAnd => MirBinOp::BitAnd,
            BinOp::BitOr => MirBinOp::BitOr,
            BinOp::BitXor => MirBinOp::BitXor,
            BinOp::Shl => MirBinOp::Shl,
            BinOp::Shr => MirBinOp::Shr,
            BinOp::Eq => MirBinOp::Eq,
            BinOp::Ne => MirBinOp::Ne,
            BinOp::Lt => MirBinOp::Lt,
            BinOp::Le => MirBinOp::Le,
            BinOp::Gt => MirBinOp::Gt,
            BinOp::Ge => MirBinOp::Ge,
            BinOp::And => MirBinOp::BitAnd, // Logical ops are already short-circuited
            BinOp::Or => MirBinOp::BitOr,
        }
    }

    fn pattern_name(&self, pattern: &Pattern) -> Option<String> {
        match &pattern.kind {
            PatternKind::Binding { name, .. } => Some(name.clone()),
            PatternKind::Paren(inner) => self.pattern_name(inner),
            _ => None,
        }
    }
}
