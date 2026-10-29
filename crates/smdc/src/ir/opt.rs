//! Semantics-preserving IR optimizations shared by every backend consumer.
//!
//! The frontends lower expressions in -O0 style: every subexpression gets a
//! fresh temp, assignments produce `Copy` chains, and constant expressions
//! (masks, scaled offsets) survive as `Binary` nodes. This module runs
//! block-local passes that are safe without SSA construction:
//!
//! - **Copy propagation**: within a basic block, reads of a temp assigned by
//!   `Copy` use the copied value directly, collapsing `t2 = t1; use(t2)`
//!   chains into `use(t1)`.
//! - **Constant folding**: `Binary`/`Unary` over two/one constants folds to
//!   an `IntConst` operand (32-bit wrapping semantics, matching the target).
//! - **Dead copy elimination**: a `Copy` whose destination temp is never
//!   read anywhere in the function is dropped.
//!
//! Temps are single-assignment by construction in the builders, but the
//! passes re-verify that assumption (any reassignment of a temp kills its
//! mapping) so they stay correct if that ever changes.

use super::{BinOp, Inst, IrFunction, IrModule, UnOp, Value};

/// Run the optimization passes over every function in the module.
pub fn optimize_module(module: &mut IrModule) {
    for func in &mut module.functions {
        optimize_function(func);
    }
}

pub fn optimize_function(func: &mut IrFunction) {
    copy_propagate(func);
    fold_constants(func);
    eliminate_dead_copies(func);
}

/// Replace reads of copied temps with their source values within each block.
fn copy_propagate(func: &mut IrFunction) {
    for block in &mut func.blocks {
        let mut map: std::collections::HashMap<u32, Value> =
            std::collections::HashMap::new();
        for si in &mut block.insts {
            // Rewrite read operands first, then record this instruction's
            // own assignment.
            match &mut si.inst {
                Inst::Copy { src, .. } => {
                    subst(src, &map);
                }
                Inst::Unary { src, .. } => subst(src, &map),
                Inst::Binary { left, right, .. } => {
                    subst(left, &map);
                    subst(right, &map);
                }
                Inst::Load { addr, .. } => subst(addr, &map),
                Inst::Store { addr, src, .. } => {
                    subst(addr, &map);
                    subst(src, &map);
                }
                Inst::CondJump { cond, .. } | Inst::CondJumpFalse { cond, .. } => {
                    subst(cond, &map);
                }
                Inst::Call { args, .. } => {
                    for a in args {
                        subst(a, &map);
                    }
                }
                Inst::CallIndirect { target, args, .. } => {
                    subst(target, &map);
                    for a in args {
                        subst(a, &map);
                    }
                }
                Inst::Return(v) => {
                    if let Some(v) = v {
                        subst(v, &map);
                    }
                }
                Inst::Label(_) | Inst::Jump(_) | Inst::Alloca { .. }
                | Inst::AddrOf { .. } | Inst::LoadParam { .. } | Inst::Comment(_) => {}
            }
            // Record the assignment this instruction makes. A Copy maps its
            // destination to the (already substituted) source; any other
            // defining instruction invalidates the destination's mapping.
            match &si.inst {
                Inst::Copy { dst, src } => {
                    let src = src.clone();
                    map.insert(dst.0, src);
                }
                Inst::Unary { dst, .. }
                | Inst::Binary { dst, .. }
                | Inst::Load { dst, .. }
                | Inst::Alloca { dst, .. }
                | Inst::AddrOf { dst, .. }
                | Inst::LoadParam { dst, .. } => {
                    map.remove(&dst.0);
                }
                Inst::Call { dst: Some(dst), .. }
                | Inst::CallIndirect { dst: Some(dst), .. } => {
                    map.remove(&dst.0);
                }
                _ => {}
            }
        }
    }
}

fn subst(v: &mut Value, map: &std::collections::HashMap<u32, Value>) {
    if let Value::Temp(t) = v
        && let Some(replacement) = map.get(&t.0)
    {
        *v = replacement.clone();
    }
}

/// Fold constant operations into `IntConst` operands.
fn fold_constants(func: &mut IrFunction) {
    for block in &mut func.blocks {
        for si in &mut block.insts {
            match &mut si.inst {
                Inst::Binary {
                    dst,
                    op,
                    left,
                    right,
                } => {
                    if let (Value::IntConst(a), Value::IntConst(b)) = (left, right)
                        && let Some(v) = fold_binary(*op, *a, *b)
                    {
                        si.inst = Inst::Copy {
                            dst: *dst,
                            src: Value::IntConst(v),
                        };
                    }
                }
                Inst::Unary { dst, op, src } => {
                    if let Value::IntConst(a) = src {
                        let a = *a;
                        let v = match op {
                            UnOp::Neg => a.wrapping_neg(),
                            UnOp::BitNot => !a,
                            UnOp::Not => (a == 0) as i64,
                        };
                        si.inst = Inst::Copy {
                            dst: *dst,
                            src: Value::IntConst(v),
                        };
                    }
                }
                _ => {}
            }
        }
    }
}

/// Fold `a op b` with the target's 32-bit wrapping semantics. Shift counts
/// are taken modulo 32. Returns `None` for operations without a foldable
/// form (division by zero stays for the backend to diagnose).
fn fold_binary(op: BinOp, a: i64, b: i64) -> Option<i64> {
    // Fold in 32-bit: the values are int-typed on the target, and shift
    // counts must wrap modulo 32, not 64.
    let a32 = a as i32;
    let b32 = b as i32;
    let r: i32 = match op {
        BinOp::Add => a32.wrapping_add(b32),
        BinOp::Sub => a32.wrapping_sub(b32),
        BinOp::Mul => a32.wrapping_mul(b32),
        BinOp::And => a32 & b32,
        BinOp::Or => a32 | b32,
        BinOp::Xor => a32 ^ b32,
        BinOp::Shl => a32.wrapping_shl(b32 as u32),
        BinOp::Shr => ((a32 as u32).wrapping_shr(b32 as u32)) as i32,
        BinOp::Sar => a32.wrapping_shr(b32 as u32),
        BinOp::Eq => (a32 == b32) as i32,
        BinOp::Ne => (a32 != b32) as i32,
        BinOp::Lt => (a32 < b32) as i32,
        BinOp::Le => (a32 <= b32) as i32,
        BinOp::Gt => (a32 > b32) as i32,
        BinOp::Ge => (a32 >= b32) as i32,
        BinOp::ULt => ((a32 as u32) < (b32 as u32)) as i32,
        BinOp::ULe => ((a32 as u32) <= (b32 as u32)) as i32,
        BinOp::UGt => ((a32 as u32) > (b32 as u32)) as i32,
        BinOp::UGe => ((a32 as u32) >= (b32 as u32)) as i32,
        BinOp::Div | BinOp::Mod | BinOp::UDiv | BinOp::UMod => return None,
    };
    Some(r as i32 as i64)
}

/// Drop `Copy` instructions whose destination is never read in the function.
fn eliminate_dead_copies(func: &mut IrFunction) {
    let read = collected_reads(func);
    for block in &mut func.blocks {
        block.insts.retain(|si| {
            if let Inst::Copy { dst, .. } = &si.inst {
                read.contains(&dst.0)
            } else {
                true
            }
        });
    }
}

/// Every temp id read by any instruction in the function.
fn collected_reads(func: &IrFunction) -> std::collections::HashSet<u32> {
    let mut read = std::collections::HashSet::new();
    fn note(v: &Value, read: &mut std::collections::HashSet<u32>) {
        if let Value::Temp(t) = v {
            read.insert(t.0);
        }
    }
    for block in &func.blocks {
        for si in &block.insts {
            match &si.inst {
                Inst::Copy { src, .. } => note(src, &mut read),
                Inst::Unary { src, .. } => note(src, &mut read),
                Inst::Binary { left, right, .. } => {
                    note(left, &mut read);
                    note(right, &mut read);
                }
                Inst::Load { addr, .. } => note(addr, &mut read),
                Inst::Store { addr, src, .. } => {
                    note(addr, &mut read);
                    note(src, &mut read);
                }
                Inst::CondJump { cond, .. } | Inst::CondJumpFalse { cond, .. } => {
                    note(cond, &mut read)
                }
                Inst::Call { args, .. } => {
                    for a in args {
                        note(a, &mut read);
                    }
                }
                Inst::CallIndirect { target, args, .. } => {
                    note(target, &mut read);
                    for a in args {
                        note(a, &mut read);
                    }
                }
                Inst::Return(Some(v)) => note(v, &mut read),
                _ => {}
            }
        }
    }
    read
}
