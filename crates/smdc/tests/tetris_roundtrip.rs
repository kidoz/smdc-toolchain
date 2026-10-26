//! Byte-exact round-trip gate for a foreign ROM (Tetris).
//!
//! ROM -> `--disasm --full` listing -> parse -> assemble must reproduce the
//! original image byte for byte. The ROM is copyrighted and lives outside
//! version control; the test skips gracefully when it is absent.

use smd_compiler::backend::m68k::{
    compare_traces, decompile_rom, disassemble_full, parse_assembly, run_rom, Assembler, RunConfig,
};
use smd_compiler::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig};
use smd_compiler::DiagnosticReporter;
use std::collections::HashMap;
use std::path::PathBuf;

fn tetris_rom_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SMDC_TETRIS_ROM") {
        return Some(PathBuf::from(p));
    }
    // cargo test runs with cwd = crates/smdc; the CLI workflow runs from the
    // workspace root. Try both.
    [
        PathBuf::from("tmp/roms/Tetris (Japan).gen"),
        PathBuf::from("../../tmp/roms/Tetris (Japan).gen"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

/// Summarize the first `max` differing ranges for a readable failure.
fn diff_report(rebuilt: &[u8], original: &[u8], max: usize) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let mut ranges = 0usize;
    let mut start: Option<usize> = None;
    for i in 0..=original.len().max(rebuilt.len()) {
        let differs = rebuilt.get(i) != original.get(i);
        match (differs, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if ranges < max {
                    let end = i - 1;
                    let _ = write!(out, "  0x{s:04X}-0x{end:04X}: ",);
                    let lo = s.saturating_sub(2);
                    let hi = (end + 2).min(original.len().saturating_sub(1));
                    let _ = write!(
                        out,
                        "orig {:02X?} vs rebuilt {:02X?}\n",
                        &original[lo..=hi.min(original.len() - 1)],
                        &rebuilt[lo..=hi.min(rebuilt.len().saturating_sub(1))],
                    );
                }
                ranges += 1;
                start = None;
            }
            _ => {}
        }
        if i == original.len().max(rebuilt.len()) {
            break;
        }
    }
    let _ = writeln!(out, "  ({ranges} differing ranges total)");
    out
}

#[test]
fn tetris_round_trips_byte_exactly() {
    let Some(path) = tetris_rom_path() else {
        eprintln!(
            "skipping: Tetris ROM not found (set SMDC_TETRIS_ROM to override)"
        );
        return;
    };
    let Ok(original) = std::fs::read(&path) else {
        eprintln!("skipping: cannot read {}", path.display());
        return;
    };

    let listing = disassemble_full(&original, &HashMap::new());
    let items = parse_assembly(&listing).expect("listing must re-parse");
    let mut assembler = Assembler::new(0);
    let rebuilt = assembler
        .assemble_items(&items)
        .expect("listing must re-assemble");

    assert_eq!(
        rebuilt.len(),
        original.len(),
        "image size differs\n{}",
        diff_report(&rebuilt, &original, 8)
    );
    assert_eq!(
        rebuilt, original,
        "round trip is not byte-exact\n{}",
        diff_report(&rebuilt, &original, 8)
    );
}

/// The pseudo-C view of a foreign ROM must stay valid C: balanced and
/// compilable through the smdc C frontend.
#[test]
fn tetris_decompiles_to_compiling_pseudo_c() {
    let Some(path) = tetris_rom_path() else {
        eprintln!("skipping: Tetris ROM not found (set SMDC_TETRIS_ROM to override)");
        return;
    };
    let Ok(original) = std::fs::read(&path) else {
        eprintln!("skipping: cannot read {}", path.display());
        return;
    };

    let source = decompile_rom(&original, &HashMap::new(), None);

    assert_eq!(
        source.matches('{').count(),
        source.matches('}').count(),
        "unbalanced braces in decompiled source"
    );

    // Foreign-ROM recoveries: named hardware registers, typed RAM globals
    // (absolute-short addresses sign-extended into work RAM), interrupt
    // handlers without phantom arguments, register-convention signatures,
    // and return-value assignment at call sites.
    assert!(source.contains("#define VDP_CTRL"), "hardware registers not named");
    assert!(
        source.contains("#define ram_FFF6"),
        "RAM globals not named:\n{}",
        &source[..source.len().min(2000)]
    );
    assert!(
        source.contains("void F_00031A(void)"),
        "VBlank handler must not take arguments"
    );
    assert!(
        source.contains("int F_000854(int a0)")
            || source.contains("int F_0009C4(int d0)"),
        "register arguments not reconstructed"
    );
    assert!(
        source.contains("d0 = F_"),
        "return values not assigned at call sites"
    );

    let mut reporter = DiagnosticReporter::new();
    let file_id = reporter.add_file("tetris.c", &source);
    let context = CompileContext::new("tetris.c".to_string(), file_id, &reporter);
    let compiled = CFrontend::new().compile(&source, &context, &FrontendConfig::default());
    if let Err(e) = &compiled {
        // Show the offending construct for census-style iteration.
        let msg = format!("{e}");
        eprintln!("decompiled pseudo-C failed to compile: {msg}");
        for line in source.lines() {
            let l = line.trim();
            if !l.is_empty()
                && (msg.contains("line") || true)
                && (l.contains("(*)") || l.contains("*--"))
            {
                eprintln!("  suspect: {l}");
            }
        }
    }
    compiled.expect("decompiled pseudo-C must compile");
}

/// Both ROMs must behave identically on the deterministic interpreter:
/// equal final work RAM and equal VDP write sequences after N frames.
/// This is the oracle the C-recompile milestones will be measured with.
#[test]
fn tetris_byte_exact_rebuild_behaves_identically() {
    let Some(path) = tetris_rom_path() else {
        eprintln!("skipping: Tetris ROM not found (set SMDC_TETRIS_ROM to override)");
        return;
    };
    let Ok(original) = std::fs::read(&path) else {
        eprintln!("skipping: cannot read {}", path.display());
        return;
    };

    let cfg = RunConfig::default();
    let a = run_rom(&original, &cfg);
    assert!(
        a.error.is_none(),
        "original ROM failed to run: {} (after {} instructions)",
        a.error.unwrap_or_default(),
        a.instructions
    );

    // Determinism: a second run of the same ROM is identical.
    let a2 = run_rom(&original, &cfg);
    compare_traces(&a, &a2).expect("interpreter must be deterministic");

    let listing = disassemble_full(&original, &HashMap::new());
    let items = parse_assembly(&listing).expect("listing must re-parse");
    let mut assembler = Assembler::new(0);
    let rebuilt = assembler.assemble_items(&items).expect("reassembly failed");
    let b = run_rom(&rebuilt, &cfg);
    compare_traces(&a, &b).expect("byte-exact rebuild must behave identically");
}
