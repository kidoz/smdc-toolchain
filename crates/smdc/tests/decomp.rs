//! ROM decompiler tests: compile C to a ROM, then decompile it back and
//! assert that functions, parameters, control flow and calls are recovered.

use smd_compiler::DiagnosticReporter;
use smd_compiler::backend::{BackendConfig, OutputFormat, RomBackend};
use smd_compiler::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig};
use smd_compiler::ir::IrModule;
use smd_compiler::backend::m68k::decompile_rom;

fn compile_c(source: &str) -> IrModule {
    let mut reporter = DiagnosticReporter::new();
    let file_id = reporter.add_file("test.c", source);
    let context = CompileContext::new("test.c".to_string(), file_id, &reporter);

    CFrontend::new()
        .compile(source, &context, &FrontendConfig::default())
        .unwrap()
}

/// Compile to a ROM with debug info; returns (rom bytes, symbols).
fn build_rom(source: &str) -> (Vec<u8>, std::collections::HashMap<String, u32>) {
    let module = compile_c(source);
    let reporter = DiagnosticReporter::new();
    let context = CompileContext::new("test".to_string(), 0, &reporter);
    let config = BackendConfig {
        output_format: OutputFormat::Binary,
        debug_info: true,
        ..BackendConfig::default()
    };
    let (rom, debug) = RomBackend::new().build_rom(&module, &config).unwrap();
    let symbols = debug.expect("debug artifacts").symbols;
    (rom, symbols)
}

fn decompile(source: &str) -> String {
    let (rom, symbols) = build_rom(source);
    decompile_rom(&rom, &symbols, None)
}

fn function_text<'a>(out: &'a str, name: &str) -> &'a str {
    // Anchor on the definition (signature followed by `{`), skipping the
    // forward-declaration prototypes emitted at the top of the file.
    let mut best: Option<usize> = None;
    for ty in ["int", "void"] {
        let sig = format!("{ty} {name}(");
        for (p, _) in out.match_indices(&sig) {
            let rest = &out[p..];
            let is_def = match (rest.find('{'), rest.find(';')) {
                (Some(b), Some(s)) => b < s,
                (Some(_), None) => true,
                (None, _) => false,
            };
            if is_def {
                best = Some(best.map_or(p, |b: usize| b.min(p)));
            }
        }
    }
    let sig = best.expect("function not found");
    let end = out[sig..]
        .find("\n}\n")
        .map(|i| sig + i + 3)
        .expect("function not terminated");
    &out[sig..end]
}

#[test]
fn functions_params_and_arithmetic_recovered() {
    let out = decompile("int add(int a, int b) { return a + b; } void main() { add(1, 2); }");
    let add = function_text(&out, "add");

    // Two parameters recovered from the cdecl frame (8(a6), 12(a6)).
    assert!(add.contains("int add(int p0, int p1)"), "{add}");
    // The addition survives as a C expression.
    assert!(add.contains('+'), "{add}");
    // Non-void return folded from `move.l X, d0; rts`.
    assert!(add.contains("return "), "{add}");
}

#[test]
fn function_names_come_from_symbols() {
    let out = decompile("int one(void) { return 1; } void main() { one(); }");
    assert!(out.contains(" one(void)"), "{out}");
    assert!(out.contains(" main("), "{out}");
}

#[test]
fn while_loop_is_structured() {
    let out = decompile(
        "void main() {\n\
             int n = 0;\n\
             while (n < 10) { n = n + 2; }\n\
         }\n",
    );
    // The back-edge branch + conditional exit must become a while loop.
    assert!(out.contains("while ("), "{out}");
}

#[test]
fn dbf_counter_loop_is_structured() {
    let out = decompile(
        "void main() {\n\
             int total = 0;\n\
             int i;\n\
             for (i = 0; i < 5; i = i + 1) { total = total + i; }\n\
         }\n",
    );
    let main = function_text(&out, "main");
    // `for` compiles to tst/bcc + dbf back edge: a loop must be recovered
    // (while or do-while), not left as raw gotos.
    assert!(
        main.contains("while (") || main.contains("do {"),
        "{main}"
    );
}

#[test]
fn if_else_is_structured() {
    let out = decompile(
        "int pick(int x) {\n\
             int y = 0;\n\
             if (x) { y = 1; } else { y = 2; }\n\
             return y;\n\
         }\n\
         void main() { pick(3); }\n",
    );
    let pick = function_text(&out, "pick");
    // The then-arm bra + else fallthrough shape must render as if/else.
    assert!(pick.contains("} else {"), "{pick}");
}

#[test]
fn calls_and_stack_arguments_recovered() {
    let out = decompile("int one(void) { return 1; } void main() { one(); }");
    let main = function_text(&out, "main");
    // cdecl: args pushed with -(sp), call, then sp cleanup.
    assert!(main.contains("one();"), "{main}");
}

#[test]
fn startup_stub_is_decompiled() {
    let out = decompile("void main() {}");
    // The entry stub contains dbf RAM-clear loops and a copy loop.
    assert!(out.contains("do {"), "{out}");
    assert!(out.contains("while ("), "{out}");
}

#[test]
fn garbage_input_does_not_panic() {
    // Empty and tiny inputs.
    assert!(!decompile_rom(&[], &Default::default(), None).is_empty());
    assert!(!decompile_rom(&[0xFF; 4], &Default::default(), None).is_empty());
    // Header-only ROM (vectors point at 0x200 but there is no code).
    let mut rom = vec![0xFF; 0x200];
    rom[4..8].copy_from_slice(&0x200u32.to_be_bytes());
    assert!(!decompile_rom(&rom, &Default::default(), None).is_empty());
    // Undecodable garbage "vectors".
    let mut rom2 = vec![0x00; 0x400];
    rom2[4..8].copy_from_slice(&0x2AAu32.to_be_bytes());
    assert!(!decompile_rom(&rom2, &Default::default(), None).is_empty());
}

#[test]
fn braces_are_balanced() {
    let out = decompile(
        "void main() {\n\
             int i;\n\
             for (i = 0; i < 3; i = i + 1) {\n\
                 if (i) { i = i + 10; } else { i = i + 20; }\n\
             }\n\
         }\n",
    );
    assert_eq!(
        out.matches('{').count(),
        out.matches('}').count(),
        "unbalanced braces:\n{out}"
    );
}

/// Definition-of-done round trip: compile C -> ROM -> decompile -> the
/// decompiled pseudo-C must itself compile to a structurally valid ROM.
#[test]
fn roundtrip_recompiles_to_valid_rom() {
    use smd_compiler::backend::rom::verify_checksum;

    let (rom, symbols) = build_rom(
        "int add(int a, int b) { return a + b; }\n\
         void main() {\n\
             int i;\n\
             int total = 0;\n\
             for (i = 0; i < 4; i = i + 1) { total = total + add(i, 2); }\n\
             if (total) { total = total + 1; } else { total = 1; }\n\
         }\n",
    );
    let out = decompile_rom(&rom, &symbols, None);

    // Recompile the decompiled source.
    let mut reporter = DiagnosticReporter::new();
    let file_id = reporter.add_file("decompiled.c", &out);
    let context = CompileContext::new("decompiled.c".to_string(), file_id, &reporter);
    let module = CFrontend::new()
        .compile(&out, &context, &FrontendConfig::default())
        .expect("decompiled source must recompile");

    let reporter2 = DiagnosticReporter::new();
    let ctx2 = CompileContext::new("re".to_string(), 0, &reporter2);
    let (rebuilt, _) = RomBackend::new()
        .build_rom(
            &module,
            &BackendConfig {
                output_format: OutputFormat::Binary,
                ..BackendConfig::default()
            },
        )
        .unwrap();

    assert!(rebuilt.len() >= 0x10000);
    assert_eq!(&rebuilt[0x100..0x110], b"SEGA MEGA DRIVE ");
    assert!(verify_checksum(&rebuilt));
}
