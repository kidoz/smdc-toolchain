//! Lowering of C semantics that the 68000 doesn't provide directly: access
//! widths of struct members, whole-struct copies, pointer scaling, signed
//! versus unsigned operations, integer conversions, static locals, and
//! initializer images. Each of these was once silently miscompiled; the
//! runtime behavior is covered by the emulator tests.

use smd_compiler::DiagnosticReporter;
use smd_compiler::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig};
use smd_compiler::ir::{Inst, IrFunction, IrModule, Value};

fn compile_c(source: &str) -> IrModule {
    let mut reporter = DiagnosticReporter::new();
    let file_id = reporter.add_file("test.c", source);
    let context = CompileContext::new("test.c".to_string(), file_id, &reporter);

    CFrontend::new()
        .compile(source, &context, &FrontendConfig::default())
        .unwrap()
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
