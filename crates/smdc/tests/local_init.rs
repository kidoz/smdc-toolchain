//! Local variable initializer lowering: arrays, structs and string
//! initializers must produce runtime stores, not silently-uninitialized
//! stack memory.

use smd_compiler::DiagnosticReporter;
use smd_compiler::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig};
use smd_compiler::ir::{Inst, IrModule, Value};

fn try_compile_c(source: &str) -> smd_compiler::common::CompileResult<IrModule> {
    let mut reporter = DiagnosticReporter::new();
    let file_id = reporter.add_file("test.c", source);
    let context = CompileContext::new("test.c".to_string(), file_id, &reporter);

    CFrontend::new().compile(source, &context, &FrontendConfig::default())
}

fn compile_c(source: &str) -> IrModule {
    try_compile_c(source).unwrap()
}

/// All (size, constant) pairs stored by constant-value Store instructions in `main`.
fn const_stores(module: &IrModule) -> Vec<(usize, i64)> {
    let main = module
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("no main function");
    main.blocks
        .iter()
        .flat_map(|b| &b.insts)
        .filter_map(|si| match &si.inst {
            Inst::Store {
                src: Value::IntConst(v),
                size,
                ..
            } => Some((*size, *v)),
            _ => None,
        })
        .collect()
}

fn stored_bytes(stores: &[(usize, i64)]) -> usize {
    stores.iter().map(|(size, _)| size).sum()
}

#[test]
fn local_array_initializer_emits_constant_stores() {
    let module = compile_c("void main(void) { int a[3] = {1, 2, 3}; }");
    let stores = const_stores(&module);

    assert_eq!(stores, vec![(4, 1), (4, 2), (4, 3)]);
}

#[test]
fn local_array_zero_fills_omitted_elements() {
    let module = compile_c("void main(void) { int a[4] = {7}; }");
    let stores = const_stores(&module);

    // 16 bytes must be written: element 0 = 7, elements 1-3 zeroed
    assert_eq!(stored_bytes(&stores), 16);
    assert_eq!(stores[0], (4, 7));
    assert!(stores[1..].iter().all(|&(_, v)| v == 0));
}

#[test]
fn local_char_array_uses_byte_stores() {
    let module = compile_c("void main(void) { char a[3] = {1, 2, 3}; }");
    let stores = const_stores(&module);

    // Odd total at byte alignment: word store then byte store (big-endian packing)
    assert_eq!(stored_bytes(&stores), 3);
    let bytes: Vec<u8> = stores
        .iter()
        .flat_map(|&(size, v)| match size {
            1 => vec![v as u8],
            2 => (v as i16).to_be_bytes().to_vec(),
            4 => (v as i32).to_be_bytes().to_vec(),
            _ => panic!("unexpected store size {size}"),
        })
        .collect();
    assert_eq!(bytes, vec![1, 2, 3]);
}

#[test]
fn local_string_initializer_copies_bytes() {
    let module = compile_c("void main(void) { char s[6] = \"hi\"; }");
    let stores = const_stores(&module);

    // "hi" + NUL + zero fill = 6 bytes, copied by value (no string pointer store)
    assert_eq!(stored_bytes(&stores), 6);
    let main = &module.functions[0];
    let string_ptr_stores = main
        .blocks
        .iter()
        .flat_map(|b| &b.insts)
        .filter(|si| {
            matches!(
                si.inst,
                Inst::Store {
                    src: Value::StringConst(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(string_ptr_stores, 0);
}

#[test]
fn local_struct_initializer_emits_stores() {
    let module = compile_c(
        "struct Point { int x; int y; };\n\
         void main(void) { struct Point p = {3, 4}; }",
    );
    let stores = const_stores(&module);

    assert_eq!(stores, vec![(4, 3), (4, 4)]);
}

#[test]
fn local_array_with_runtime_values_stores_each_element() {
    let module = compile_c("void main(void) { int x = 5; int a[2] = {x, 9}; }");
    let main = &module.functions[0];

    let store_count = main
        .blocks
        .iter()
        .flat_map(|b| &b.insts)
        .filter(|si| matches!(si.inst, Inst::Store { .. }))
        .count();
    // x = 5, a[0] = x, a[1] = 9
    assert_eq!(store_count, 3);
}

#[test]
fn rejects_too_long_string_initializer() {
    let result = try_compile_c("void main(void) { char s[2] = \"abc\"; }");
    assert!(result.is_err());
}

#[test]
fn exact_fit_string_initializer_drops_nul() {
    let module = compile_c("void main(void) { char s[2] = \"ab\"; }");
    let stores = const_stores(&module);

    assert_eq!(stored_bytes(&stores), 2);
}

#[test]
fn scalar_initializer_still_works() {
    let module = compile_c("void main(void) { int x = 42; }");
    let stores = const_stores(&module);

    assert_eq!(stores, vec![(4, 42)]);
}
