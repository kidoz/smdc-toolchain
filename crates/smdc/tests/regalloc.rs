//! Register-cache codegen invariants.
//!
//! The block-local D2-D7 cache in the M68k backend must uphold three invariants
//! that structural ROM tests cannot catch:
//! 1. Cached values are written back BEFORE a branch (the jump path exits at the
//!    branch — stores after it are dead on that path and the successor would
//!    read garbage from the stack).
//! 2. Cached values are written back at every block label (fall-through paths
//!    have no terminator; successors reached by jump must load from the stack,
//!    not from registers only the fall-through path populated).
//! 3. No store may appear between TST and its branch (MOVE clobbers Z/N flags
//!    on M68k, so an interleaved writeback would change what the branch tests).
//! 4. The frame must be sized for LoadParam destination temps (they previously
//!    were not counted, so functions with several unused params wrote below the
//!    frame into the MOVEM save area).

use smd_compiler::DiagnosticReporter;
use smd_compiler::backend::{Backend, BackendConfig, M68kBackend, OutputFormat, OutputKind};
use smd_compiler::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig};
use smd_compiler::ir::IrModule;

fn compile_c(source: &str) -> IrModule {
    let mut reporter = DiagnosticReporter::new();
    let file_id = reporter.add_file("test.c", source);
    let context = CompileContext::new("test.c".to_string(), file_id, &reporter);

    CFrontend::new()
        .compile(source, &context, &FrontendConfig::default())
        .unwrap()
}

fn build_asm(module: &IrModule) -> String {
    let reporter = DiagnosticReporter::new();
    let context = CompileContext::new("test".to_string(), 0, &reporter);
    let config = BackendConfig {
        output_format: OutputFormat::Assembly,
        ..BackendConfig::default()
    };
    let output = M68kBackend::new()
        .generate(module, &context, &config)
        .unwrap();

    match output.data {
        OutputKind::Text(asm) => asm,
        OutputKind::Binary(_) => panic!("assembly backend returned binary output"),
    }
}

#[test]
fn no_dead_stores_after_unconditional_branch() {
    // The then-branch ends with `bra endif`. Its cache writeback must happen
    // before the bra; anything between `bra` and the next label is dead on the
    // jump path and would signal a missing pre-branch flush.
    let module = compile_c(
        "int f(int x) { int y = x + 1; if (x) { y = 1; } else { y = y + 2; } return y; }",
    );
    let asm = build_asm(&module);

    let mut after_bra = false;
    for line in asm.lines() {
        let t = line.trim();
        if after_bra {
            assert!(
                t.starts_with('.') || t.is_empty(),
                "dead instruction after unconditional branch: {t:?}"
            );
            if t.starts_with('.') {
                after_bra = false;
            }
            continue;
        }
        if t.starts_with("bra") {
            after_bra = true;
        }
    }
}

#[test]
fn successor_blocks_load_temps_from_stack() {
    // After the endif label (reachable both by jump and by fall-through), the
    // first temp read must be a stack load. If the fall-through path's cache
    // leaked across the label, this would be a register-to-register move from
    // a D2-D7 register no jump path ever populated.
    let module = compile_c(
        "int f(int x) { int y = x + 1; if (x) { y = 1; } else { y = y + 2; } return y; }",
    );
    let asm = build_asm(&module);

    let endif = asm
        .lines()
        .position(|l| l.trim().starts_with(".Lendif"))
        .expect("no endif label in output");
    let after: Vec<&str> = asm
        .lines()
        .skip(endif + 1)
        .map(|l| l.trim())
        .take_while(|l| *l != "rts")
        .filter(|l| !l.is_empty())
        .collect();

    let first_load = after
        .iter()
        .find(|l| l.starts_with("move.l"))
        .expect("no load after endif");
    assert!(
        first_load.contains("(a6)") || first_load.contains("(a0)"),
        "first temp read after a join label must be a memory load, got: {first_load}"
    );
}

#[test]
fn tst_is_immediately_followed_by_branch() {
    // MOVE sets the Z/N flags on M68k. A cache writeback emitted between TST
    // and Bcc would make the branch test the flushed value instead of the
    // condition.
    let module = compile_c(
        "int f(int x) { int y = 0; while (y < x) { y = y + 1; } if (y) { y = 2; } return y; }",
    );
    let asm = build_asm(&module);

    let lines: Vec<&str> = asm.lines().map(|l| l.trim()).collect();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("tst") {
            let next = lines.get(i + 1).unwrap_or(&"<eof>");
            assert!(
                next.starts_with("beq")
                    || next.starts_with("bne")
                    || next.starts_with("bcc")
                    || next.starts_with("bcs")
                    || next.starts_with("bgt")
                    || next.starts_with("blt")
                    || next.starts_with("bge")
                    || next.starts_with("ble")
                    || next.starts_with("bhi")
                    || next.starts_with("bls")
                    || next.starts_with("bpl")
                    || next.starts_with("bmi"),
                "instruction between TST and branch clobbers flags: {line} / {next}"
            );
        }
    }
}

#[test]
fn frame_covers_unused_param_temps() {
    // 7 LoadParam destination temps: 7 * 4 bytes + 16 saved-reg headroom = 44.
    // Before the fix, LoadParam dsts were not counted in max_temp and the
    // frame stayed at 16 bytes while stores reached -28(a6) — 12 bytes into
    // the MOVEM save area.
    let module = compile_c("int f(int a, int b, int c, int d, int e, int g, int h) { return 5; }");
    let asm = build_asm(&module);

    let link = asm
        .lines()
        .find(|l| l.trim().starts_with("link"))
        .expect("no link instruction");
    assert!(
        link.contains("#-44"),
        "frame must cover all LoadParam temps (expected link #-44): {link}"
    );
}
