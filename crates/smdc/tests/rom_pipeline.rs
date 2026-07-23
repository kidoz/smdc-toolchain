use smd_compiler::backend::rom::verify_checksum;
use smd_compiler::backend::{Backend, BackendConfig, OutputFormat, OutputKind, RomBackend};
use smd_compiler::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig, RustFrontend};
use smd_compiler::{DiagnosticReporter, ir::IrModule};

fn compile_c(source: &str) -> IrModule {
    compile(source, "test.c", &CFrontend::new())
}

fn compile_rust(source: &str) -> IrModule {
    compile(source, "test.rs", &RustFrontend::new())
}

fn compile(source: &str, filename: &str, frontend: &dyn Frontend) -> IrModule {
    let mut reporter = DiagnosticReporter::new();
    let file_id = reporter.add_file(filename, source);
    let context = CompileContext::new(filename.to_string(), file_id, &reporter);

    frontend
        .compile(source, &context, &FrontendConfig::default())
        .unwrap()
}

fn build_rom(module: &IrModule) -> Vec<u8> {
    let reporter = DiagnosticReporter::new();
    let context = CompileContext::new("test".to_string(), 0, &reporter);
    let config = BackendConfig {
        output_format: OutputFormat::Binary,
        ..BackendConfig::default()
    };
    let output = RomBackend::new()
        .generate(module, &context, &config)
        .unwrap();

    match output.data {
        OutputKind::Binary(rom) => rom,
        OutputKind::Text(_) => panic!("ROM backend returned text output"),
    }
}

fn assert_valid_rom(rom: &[u8]) {
    assert!(rom.len() >= 0x10000);
    assert_eq!(&rom[0x100..0x110], b"SEGA MEGA DRIVE ");
    assert_eq!(
        u32::from_be_bytes(rom[0..4].try_into().unwrap()),
        0x00FF_E000
    );
    assert_eq!(
        u32::from_be_bytes(rom[4..8].try_into().unwrap()),
        0x0000_0200
    );
    assert!(verify_checksum(rom));
}

#[test]
fn c_program_without_globals_builds_rom() {
    let module = compile_c("void main(void) {}");
    let rom = build_rom(&module);

    assert_valid_rom(&rom);
}

#[test]
fn rust_program_without_globals_builds_rom() {
    let module = compile_rust("fn main() {}");
    let rom = build_rom(&module);

    assert_valid_rom(&rom);
}

#[test]
fn initialized_local_array_builds_rom() {
    let module = compile_c(
        "void main(void) { int a[3] = {1, 2, 3}; char s[6] = \"hi\"; (void)a; (void)s; }",
    );
    let rom = build_rom(&module);

    assert_valid_rom(&rom);
}

#[test]
fn initialized_global_builds_rom() {
    let module = compile_c("int counter = 7; void main(void) {}");
    let rom = build_rom(&module);

    assert_valid_rom(&rom);
}

#[test]
fn c_control_flow_builds_rom() {
    let module = compile_c(
        "int sum_to(int n) {\n\
             int total = 0;\n\
             int i;\n\
             for (i = 1; i <= n; i++) {\n\
                 if (i % 2 == 0) total += i; else total += i * 2;\n\
             }\n\
             while (total > 100) total -= 10;\n\
             return total;\n\
         }\n\
         void main(void) { sum_to(10); }",
    );
    let rom = build_rom(&module);

    assert_valid_rom(&rom);
}

#[test]
fn c_structs_and_pointers_build_rom() {
    let module = compile_c(
        "struct Vec { int x; int y; };\n\
         static struct Vec pos;\n\
         void move_by(struct Vec *v, int dx, int dy) { v->x += dx; v->y += dy; }\n\
         void main(void) {\n\
             struct Vec local = {3, 4};\n\
             pos.x = local.x;\n\
             move_by(&pos, 1, 2);\n\
         }",
    );
    let rom = build_rom(&module);

    assert_valid_rom(&rom);
}

#[test]
fn c_sdk_call_builds_rom() {
    let module = compile_c(
        "void vdp_init(void);\n\
         void vdp_set_color(int index, int color);\n\
         void main(void) { vdp_init(); vdp_set_color(0, 0x0E00); }",
    );
    let rom = build_rom(&module);

    assert_valid_rom(&rom);
}

#[test]
fn c_enum_and_const_array_size_build_rom() {
    let module = compile_c(
        "enum Flags { A = 1 << 0, B = 1 << 1, AB = A | B };\n\
         void main(void) { int buf[4 * 4]; buf[15] = AB; (void)buf; }",
    );
    let rom = build_rom(&module);

    assert_valid_rom(&rom);
}

#[test]
fn rust_function_call_builds_rom() {
    let module = compile_rust(
        "fn add(a: i32, b: i32) -> i32 { a + b }\n\
         fn main() { let _x = add(2, 3); }",
    );
    let rom = build_rom(&module);

    assert_valid_rom(&rom);
}
