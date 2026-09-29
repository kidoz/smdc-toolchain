//! Lowering of C semantics that the 68000 doesn't provide directly: access
//! widths of struct members, whole-struct copies, pointer scaling, signed
//! versus unsigned operations, integer conversions, static locals, and
//! initializer images. Each of these was once silently miscompiled; the
//! runtime behavior is covered by the emulator tests.

use smd_compiler::DiagnosticReporter;
use smd_compiler::backend::m68k::CodeGenerator;
use smd_compiler::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig};
use smd_compiler::ir::{BinOp, Inst, IrFunction, IrModule, Value};

fn compile_c(source: &str) -> IrModule {
    let mut reporter = DiagnosticReporter::new();
    let file_id = reporter.add_file("test.c", source);
    let context = CompileContext::new("test.c".to_string(), file_id, &reporter);

    CFrontend::new()
        .compile(source, &context, &FrontendConfig::default())
        .unwrap()
}

fn assembly(source: &str) -> String {
    CodeGenerator::new().generate(&compile_c(source)).unwrap()
}

fn function<'a>(module: &'a IrModule, name: &str) -> &'a IrFunction {
    module
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no function {name}"))
}

fn insts(func: &IrFunction) -> impl Iterator<Item = &Inst> {
    func.blocks.iter().flat_map(|b| &b.insts).map(|si| &si.inst)
}

fn store_sizes(func: &IrFunction) -> Vec<usize> {
    insts(func)
        .filter_map(|inst| match inst {
            Inst::Store { size, .. } => Some(*size),
            _ => None,
        })
        .collect()
}

fn binary_ops(func: &IrFunction) -> Vec<BinOp> {
    insts(func)
        .filter_map(|inst| match inst {
            Inst::Binary { op, .. } => Some(*op),
            _ => None,
        })
        .collect()
}

/// Binary operations with a constant right operand, as (op, constant)
fn const_binaries(func: &IrFunction) -> Vec<(BinOp, i64)> {
    insts(func)
        .filter_map(|inst| match inst {
            Inst::Binary {
                op,
                right: Value::IntConst(c),
                ..
            } => Some((*op, *c)),
            _ => None,
        })
        .collect()
}

#[test]
fn struct_member_stores_use_the_member_width() {
    let module = compile_c(
        "struct S { short a; char b; char c; int d; };\n\
         void f(struct S *p) { p->a = 1; p->b = 2; p->c = 3; p->d = 4; }",
    );

    assert_eq!(store_sizes(function(&module, "f")), vec![2, 1, 1, 4]);
}

#[test]
fn struct_layout_pads_members_and_size() {
    let module = compile_c(
        "struct T { char a; int b; short c; char d; };\n\
         struct T g;\n\
         int f(void) { return sizeof(struct T); }",
    );

    // a@0, b@2, c@6, d@8, padded to 10
    assert_eq!(module.globals[0].ty.size, 10);
    assert!(
        insts(function(&module, "f"))
            .any(|inst| matches!(inst, Inst::Return(Some(Value::IntConst(10)))))
    );
}

#[test]
fn struct_assignment_copies_every_byte() {
    let module = compile_c(
        "struct P { int x; int y; short z; };\n\
         void f(struct P *a, struct P *b) { *a = *b; }",
    );

    assert_eq!(store_sizes(function(&module, "f")), vec![4, 4, 2]);
}

#[test]
fn large_struct_copy_loops() {
    let module = compile_c(
        "struct Big { int v[20]; };\n\
         void f(struct Big *a, struct Big *b) { *a = *b; }",
    );
    let f = function(&module, "f");

    assert_eq!(store_sizes(f), vec![4]);
    assert!(insts(f).any(|inst| matches!(inst, Inst::CondJump { .. })));
}

#[test]
fn pointer_arithmetic_scales_by_element_size() {
    let module = compile_c(
        "int f(int *p, int n) { p = p + n; p++; p += 2; return *(p - 1); }\n\
         int g(short *a, short *b) { return a - b; }",
    );

    let f = const_binaries(function(&module, "f"));
    assert!(f.contains(&(BinOp::Mul, 4)), "p + n: {f:?}");
    assert!(f.contains(&(BinOp::Add, 4)), "p++: {f:?}");
    assert!(f.contains(&(BinOp::Add, 8)), "p += 2: {f:?}");
    assert!(f.contains(&(BinOp::Sub, 4)), "p - 1: {f:?}");
    assert!(const_binaries(function(&module, "g")).contains(&(BinOp::Sar, 1)));
}

#[test]
fn unsigned_operands_select_unsigned_operations() {
    let module = compile_c(
        "int f(unsigned int a, unsigned int b) { return (a < b) + (a > b) + a / b + a % b + (a >> 1); }\n\
         int g(int a, int b) { return (a < b) + a / b + a % b + (a >> 1); }\n\
         int h(unsigned char a, int b) { return (a < b) + a / b; }",
    );

    let f = binary_ops(function(&module, "f"));
    for op in [BinOp::ULt, BinOp::UGt, BinOp::UDiv, BinOp::UMod, BinOp::Shr] {
        assert!(f.contains(&op), "{op:?} missing from {f:?}");
    }
    let g = binary_ops(function(&module, "g"));
    for op in [BinOp::Lt, BinOp::Div, BinOp::Mod, BinOp::Sar] {
        assert!(g.contains(&op), "{op:?} missing from {g:?}");
    }
    // unsigned char promotes to (signed) int
    let h = binary_ops(function(&module, "h"));
    assert!(h.contains(&BinOp::Lt) && h.contains(&BinOp::Div), "{h:?}");
}

#[test]
fn narrowing_casts_truncate_and_extend() {
    let module = compile_c(
        "int f(int x) { return (unsigned char)x; }\n\
         int g(int x) { return (short)x; }\n\
         int h(unsigned char x) { return (int)x + (unsigned short)x; }\n\
         unsigned char r(int x) { return x; }",
    );

    assert_eq!(
        const_binaries(function(&module, "f")),
        vec![(BinOp::And, 0xFF)]
    );
    assert_eq!(
        const_binaries(function(&module, "g")),
        vec![(BinOp::Shl, 16), (BinOp::Sar, 16)]
    );
    // Widening an already-extended value needs no code
    assert!(!binary_ops(function(&module, "h")).contains(&BinOp::And));
    assert_eq!(
        const_binaries(function(&module, "r")),
        vec![(BinOp::And, 0xFF)]
    );
}

#[test]
fn static_local_is_a_private_global() {
    let module = compile_c("int f(void) { static int n = 5; n = n + 1; return n; }");

    let global = module
        .globals
        .iter()
        .find(|g| g.name.starts_with("__static_f_n"))
        .expect("static local not emitted as a global");
    assert_eq!(global.init.as_deref(), Some(&[0, 0, 0, 5][..]));
    assert!(!insts(function(&module, "f")).any(|inst| matches!(inst, Inst::Alloca { .. })));
}

#[test]
fn global_initializer_honors_designators_and_elided_braces() {
    let module = compile_c(
        "struct P { int x; short y; };\n\
         struct P p = { .y = 2, .x = 1 };\n\
         short m[2][2] = { 1, 2, [1] = { 3 } };\n\
         char s[] = \"hi\";",
    );
    let init = |name: &str| {
        module
            .globals
            .iter()
            .find(|g| g.name == name)
            .and_then(|g| g.init.clone())
            .unwrap()
    };

    assert_eq!(init("p"), vec![0, 0, 0, 1, 0, 2]);
    assert_eq!(init("m"), vec![0, 1, 0, 2, 0, 3, 0, 0]);
    assert_eq!(init("s"), b"hi\0".to_vec());
}

#[test]
fn multiply_and_divide_call_runtime_helpers_only_when_needed() {
    let asm = assembly(
        "int f(int a, int b) { return a * b; }\n\
         int g(int a, int b) { return a / b; }",
    );
    assert!(asm.contains("jsr     __mulsi3"));
    assert!(asm.contains("jsr     __divsi3"));
    // __divsi3 divides magnitudes with __udivsi3
    for helper in ["__mulsi3:", "__divsi3:", "__udivsi3:"] {
        assert!(asm.contains(helper), "{helper} not emitted");
    }
    assert!(!asm.contains("muls.w") && !asm.contains("divs.w"));

    // Constant multipliers and unsigned power-of-two divisors are inline
    let asm = assembly(
        "int f(int a) { return a * 8 + a * 10; }\n\
         unsigned int g(unsigned int a) { return a / 16 + a % 16; }",
    );
    assert!(!asm.contains("__mulsi3") && !asm.contains("__udivsi3"));
    assert!(asm.contains("lsl.l") && asm.contains("mulu.w") && asm.contains("lsr.l"));
}

#[test]
fn globals_after_byte_data_are_word_aligned() {
    let asm = assembly("char c = 1;\nint i = 2;\nvoid main(void) {}");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let label = lines.iter().position(|l| *l == "i:").unwrap();

    assert_eq!(lines[label - 1], ".align 2");
}
