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
fn initialized_global_builds_rom() {
    let module = compile_c("int counter = 7; void main(void) {}");
    let rom = build_rom(&module);

    assert_valid_rom(&rom);
}
