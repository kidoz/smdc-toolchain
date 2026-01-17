//! Convert MIR to the shared IR

use crate::ir::{IrFunction, Inst, Label, Temp, Value, BinOp as IrBinOp, UnOp as IrUnOp};
use crate::frontend::c::ast::CType;
use crate::frontend::rust::ast::{RustType, RustTypeKind};
use crate::common::Span;
use super::types::*;
use std::collections::HashMap;

/// Converts MIR to the shared IR
pub struct MirToIr<'a> {
    /// Mapping from MIR locals to IR temps
    local_to_temp: HashMap<LocalId, Temp>,
    /// Mapping from MIR blocks to IR labels
    block_to_label: HashMap<BlockId, Label>,
    /// Next temp ID
    next_temp: u32,
    /// Next label ID
    next_label: usize,
    /// Generated instructions
    instructions: Vec<Inst>,
    /// Current function name (for unique labels)
    func_name: String,
    /// Reference to the MIR body for type lookups
    mir_body: Option<&'a MirBody>,
}

impl<'a> MirToIr<'a> {
    pub fn new() -> Self {
        Self {
            local_to_temp: HashMap::new(),
            block_to_label: HashMap::new(),
            next_temp: 0,
            next_label: 0,
            instructions: Vec::new(),
            func_name: String::new(),
            mir_body: None,
        }
    }

    /// Convert a MIR body to IR
    pub fn convert(&mut self, name: String, mir: &'a MirBody) -> IrFunction {
        // Store function name for unique labels
        self.func_name = name.clone();
        // Store MIR body reference for type lookups
        self.mir_body = Some(mir);

        // Create temps for all locals
        for local in &mir.locals {
            let temp = self.new_temp();
            self.local_to_temp.insert(local.id, temp);
        }

        // Create labels for all blocks (include function name for uniqueness)
        for block in &mir.blocks {
            let label = Label(format!(".L{}_bb{}", self.func_name, block.id.0));
            self.block_to_label.insert(block.id, label.clone());
        }

        // Emit LoadParam instructions for function parameters
        // Parameters are locals 1..=arg_count (local 0 is return value)
        // The backend stores param ADDRESSES in temps, so we need to load
        // the actual value afterwards.
        for i in 0..mir.arg_count {
            let local_id = LocalId(i + 1); // +1 because local 0 is return value
            if let Some(&temp) = self.local_to_temp.get(&local_id) {
                let size = mir.locals.get(i + 1)
                    .map(|l| l.ty.size())
                    .unwrap_or(4);
                // Create a temp for the address
                let addr_temp = self.new_temp();
                self.emit(Inst::LoadParam {
                    dst: addr_temp,
                    index: i,
                    size,
                });
                // Load the actual value from the address into the final temp
                self.emit(Inst::Load {
                    dst: temp,
                    addr: Value::Temp(addr_temp),
                    size,
                    volatile: false,
                    signed: true,
                });
            }
        }

        // Convert each block
        for block in &mir.blocks {
            self.convert_block(block);
        }

        // Create a simple return type (i32 for now)
        let return_type = CType::int(Span::default());

        IrFunction {
            name,
            params: Vec::new(), // Would need to extract from MIR
            return_type,
            body: std::mem::take(&mut self.instructions),
            locals: Vec::new(),
        }
    }

    fn convert_block(&mut self, block: &MirBlock) {
        // Emit block label
        let label = self.get_block_label(&block.id);
        self.emit(Inst::Label(label));

        // Convert statements
        for stmt in &block.statements {
            self.convert_statement(stmt);
        }

        // Convert terminator
        if let Some(term) = &block.terminator {
            self.convert_terminator(term);
        }
    }

    fn convert_statement(&mut self, stmt: &MirStatement) {
        match stmt {
            MirStatement::Assign { dest, value } => {
                // Check if destination has a Deref projection (i.e., *ptr = value)
                if self.place_has_deref(dest) {
                    // Store through pointer
                    let addr_temp = self.get_local_temp(&dest.local);
                    let size = self.get_deref_size(dest);
                    let value_temp = self.new_temp();
                    self.convert_rvalue(value_temp, value);
                    self.emit(Inst::Store {
                        addr: Value::Temp(addr_temp),
                        src: Value::Temp(value_temp),
                        size,
                        volatile: false,
                    });
                } else {
                    let dest_temp = self.place_to_temp(dest);
                    self.convert_rvalue(dest_temp, value);
                }
            }
            MirStatement::Drop(_) => {
                // No-op for now (no destructors)
            }
            MirStatement::Nop => {}
        }
    }

    fn place_has_deref(&self, place: &Place) -> bool {
        place.projections.iter().any(|p| matches!(p, Projection::Deref))
    }

    /// Get the size of the pointee type for a dereferenced place
    fn get_deref_size(&self, place: &Place) -> usize {
        if let Some(mir) = self.mir_body {
            if let Some(local) = mir.locals.get(place.local.0) {
                // Get the pointer type and extract the pointee size
                if let RustTypeKind::Pointer { inner, .. } = &local.ty.kind {
                    return inner.size();
                }
            }
        }
        // Default to 4 bytes (i32) if we can't determine the type
        4
    }

    fn convert_rvalue(&mut self, dest: Temp, rvalue: &Rvalue) {
        match rvalue {
            Rvalue::Use(operand) => {
                let value = self.operand_to_value(operand);
                self.emit(Inst::Copy { dst: dest, src: value });
            }
            Rvalue::Ref { place, .. } => {
                // Take address of place
                let base_temp = self.place_to_temp(place);
                // For now, just copy the address (simplified)
                self.emit(Inst::Copy {
                    dst: dest,
                    src: Value::Temp(base_temp),
                });
            }
            Rvalue::BinaryOp { op, left, right } => {
                let left_val = self.operand_to_value(left);
                let right_val = self.operand_to_value(right);
                let ir_op = self.convert_bin_op(*op);
                self.emit(Inst::Binary {
                    dst: dest,
                    op: ir_op,
                    left: left_val,
                    right: right_val,
                });
            }
            Rvalue::UnaryOp { op, operand } => {
                let val = self.operand_to_value(operand);
                let ir_op = self.convert_unary_op(*op);
                self.emit(Inst::Unary {
                    dst: dest,
                    op: ir_op,
                    src: val,
                });
            }
            Rvalue::Cast { operand, .. } => {
                // Simplified: just copy
                let val = self.operand_to_value(operand);
                self.emit(Inst::Copy { dst: dest, src: val });
            }
            Rvalue::Aggregate { operands, .. } => {
                // For aggregates, we'd need to allocate space and store fields
                // Simplified: store in consecutive temps
                for (i, operand) in operands.iter().enumerate() {
                    let val = self.operand_to_value(operand);
                    if i == 0 {
                        self.emit(Inst::Copy { dst: dest, src: val });
                    } else {
                        let field_temp = self.new_temp();
                        self.emit(Inst::Copy { dst: field_temp, src: val });
                    }
                }
            }
            Rvalue::Len(_) => {
                // Would need runtime support
                self.emit(Inst::Copy {
                    dst: dest,
                    src: Value::IntConst(0),
                });
            }
        }
    }

    fn convert_terminator(&mut self, term: &MirTerminator) {
        match term {
            MirTerminator::Return => {
                // Return value is in local 0
                let ret_temp = self.get_local_temp(&LocalId(0));
                self.emit(Inst::Return(Some(Value::Temp(ret_temp))));
            }
            MirTerminator::Goto(target) => {
                let label = self.get_block_label(target);
                self.emit(Inst::Jump(label));
            }
            MirTerminator::If { condition, then_block, else_block } => {
                let cond = self.operand_to_value(condition);
                let then_label = self.get_block_label(then_block);
                let else_label = self.get_block_label(else_block);

                // Branch if condition is true
                self.emit(Inst::CondJump {
                    cond,
                    target: then_label,
                });
                self.emit(Inst::Jump(else_label));
            }
            MirTerminator::Switch { value, targets, default } => {
                let val = self.operand_to_value(value);
                let default_label = self.get_block_label(default);

                // Generate comparisons for each target
                for (const_val, target) in targets {
                    let target_label = self.get_block_label(target);
                    let cmp_temp = self.new_temp();

                    self.emit(Inst::Binary {
                        dst: cmp_temp,
                        op: IrBinOp::Eq,
                        left: val.clone(),
                        right: Value::IntConst(*const_val),
                    });
                    self.emit(Inst::CondJump {
                        cond: Value::Temp(cmp_temp),
                        target: target_label,
                    });
                }

                self.emit(Inst::Jump(default_label));
            }
            MirTerminator::Call { func, args, dest, target } => {
                let func_name = match func {
                    Operand::Constant(MirConstant::Function(name)) => name.clone(),
                    _ => "unknown".to_string(),
                };

                let arg_values: Vec<_> = args.iter()
                    .map(|a| self.operand_to_value(a))
                    .collect();

                let dest_temp = self.place_to_temp(dest);
                self.emit(Inst::Call {
                    dst: Some(dest_temp),
                    func: func_name,
                    args: arg_values,
                });

                let target_label = self.get_block_label(target);
                self.emit(Inst::Jump(target_label));
            }
            MirTerminator::Unreachable => {
                // Could emit a trap instruction
                self.emit(Inst::Return(None));
            }
        }
    }

    /// Get the IR label for a MIR block, with fallback for missing blocks
    fn get_block_label(&self, block_id: &BlockId) -> Label {
        self.block_to_label
            .get(block_id)
            .cloned()
            .unwrap_or_else(|| Label(format!("bb_unknown_{}", block_id.0)))
    }

    /// Get the IR temp for a MIR local, with fallback for missing locals
    fn get_local_temp(&self, local_id: &LocalId) -> Temp {
        self.local_to_temp
            .get(local_id)
            .copied()
            .unwrap_or(Temp(0))
    }

    fn place_to_temp(&self, place: &Place) -> Temp {
        // For now, just return the base temp
        // A full implementation would handle projections
        self.get_local_temp(&place.local)
    }

    fn operand_to_value(&mut self, operand: &Operand) -> Value {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => {
                // Check if place has a Deref projection (i.e., reading *ptr)
                if self.place_has_deref(place) {
                    // Load through pointer
                    let addr_temp = self.get_local_temp(&place.local);
                    let size = self.get_deref_size(place);
                    let result_temp = self.new_temp();
                    self.emit(Inst::Load {
                        dst: result_temp,
                        addr: Value::Temp(addr_temp),
                        size,
                        volatile: false,
                        signed: true,
                    });
                    Value::Temp(result_temp)
                } else {
                    Value::Temp(self.place_to_temp(place))
                }
            }
            Operand::Constant(constant) => {
                match constant {
                    MirConstant::Int(v) => Value::IntConst(*v),
                    MirConstant::Float(v) => Value::IntConst(*v as i64),
                    MirConstant::Bool(v) => Value::IntConst(if *v { 1 } else { 0 }),
                    MirConstant::Char(v) => Value::IntConst(*v as i64),
                    MirConstant::String(s) => {
                        // Create a label for the string
                        Value::StringConst(Label(format!("str_{}", s.len())))
                    }
                    MirConstant::Unit => Value::IntConst(0),
                    MirConstant::Function(name) => Value::Name(name.clone()),
                    MirConstant::Static(name) => {
                        // Static variables need to be loaded from their address
                        let result_temp = self.new_temp();
                        self.emit(Inst::Load {
                            dst: result_temp,
                            addr: Value::Name(name.clone()),
                            size: 4, // Assume i32 for now
                            volatile: false,
                            signed: true,
                        });
                        Value::Temp(result_temp)
                    }
                }
            }
        }
    }

    fn convert_bin_op(&self, op: MirBinOp) -> IrBinOp {
        match op {
            MirBinOp::Add => IrBinOp::Add,
            MirBinOp::Sub => IrBinOp::Sub,
            MirBinOp::Mul => IrBinOp::Mul,
            MirBinOp::Div => IrBinOp::Div,
            MirBinOp::Rem => IrBinOp::Mod,
            MirBinOp::BitAnd => IrBinOp::And,
            MirBinOp::BitOr => IrBinOp::Or,
            MirBinOp::BitXor => IrBinOp::Xor,
            MirBinOp::Shl => IrBinOp::Shl,
            MirBinOp::Shr => IrBinOp::Shr,
            MirBinOp::Eq => IrBinOp::Eq,
            MirBinOp::Ne => IrBinOp::Ne,
            MirBinOp::Lt => IrBinOp::Lt,
            MirBinOp::Le => IrBinOp::Le,
            MirBinOp::Gt => IrBinOp::Gt,
            MirBinOp::Ge => IrBinOp::Ge,
        }
    }

    fn convert_unary_op(&self, op: MirUnaryOp) -> IrUnOp {
        match op {
            MirUnaryOp::Neg => IrUnOp::Neg,
            MirUnaryOp::Not => IrUnOp::Not,
        }
    }

    fn new_temp(&mut self) -> Temp {
        let id = self.next_temp;
        self.next_temp += 1;
        Temp(id)
    }

    fn new_label(&mut self, prefix: &str) -> Label {
        let id = self.next_label;
        self.next_label += 1;
        Label(format!(".L{}_{}{}", self.func_name, prefix, id))
    }

    fn emit(&mut self, inst: Inst) {
        self.instructions.push(inst);
    }
}

impl Default for MirToIr<'_> {
    fn default() -> Self {
        Self::new()
    }
}
