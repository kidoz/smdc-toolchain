//! Byte-exact round-trip gate for a foreign ROM (Tetris).
//!
//! ROM -> `--disasm --full` listing -> parse -> assemble must reproduce the
//! original image byte for byte. The ROM is copyrighted and lives outside
//! version control; the test skips gracefully when it is absent.

use smd_compiler::backend::m68k::{
    build_data_only_rebuild, build_hybrid_rom, compare_traces, decompile_rom, disassemble_full,
    parse_assembly, run_rom, Assembler, RunConfig,
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
    if std::env::var("SMDC_DUMP_REBUILD").is_ok() {
        let _ = std::fs::create_dir_all("tmp");
        let _ = std::fs::write("tmp/rebuilt.gen", &rebuilt);
        let _ = std::fs::write("tmp/rebuilt.s", &listing);
    }
    let b = run_rom(&rebuilt, &cfg);
    compare_traces(&a, &b).expect("byte-exact rebuild must behave identically");
}

/// Hybrid rebuild: original image as substrate, discovered functions
/// replaced by their compiled C. Milestone: the hybrid must behave
/// identically to the original — same work RAM state (stack residue
/// excluded) and the same VDP write sequence.
#[test]
fn tetris_hybrid_behaves_identically() {
    let Some(path) = tetris_rom_path() else {
        eprintln!("skipping: Tetris ROM not found (set SMDC_TETRIS_ROM to override)");
        return;
    };
    let Ok(original) = std::fs::read(&path) else {
        eprintln!("skipping: cannot read {}", path.display());
        return;
    };

    let hybrid = build_hybrid_rom(&original).expect("hybrid build failed");
    assert!(
        hybrid.image.len() > original.len(),
        "hybrid should append compiled code"
    );
    if std::env::var("SMDC_DUMP_HYBRID").is_ok() {
        let _ = std::fs::create_dir_all("tmp");
        let _ = std::fs::write("tmp/hybrid.gen", &hybrid.image);
        let _ = std::fs::write("tmp/hybrid.c", &hybrid.source);
        let mut map = String::new();
        let mut addrs: Vec<_> = hybrid.function_map.iter().collect();
        addrs.sort();
        for (orig, new) in addrs {
            map.push_str(&format!("{orig:06X} -> {new:06X}\n"));
        }
        let _ = std::fs::write("tmp/hybrid.map", map);
    }

    // A generous budget: the hybrid's compiled checksum loop alone takes
    // ~12M instructions (the original finishes its whole boot in ~0.5M).
    // Both ROMs run under the SAME config, keeping the comparison fair.
    let cfg = RunConfig {
        frames: 40,
        frame_instructions: 1_000_000,
    };
    let a = run_rom(&original, &cfg);
    assert!(
        a.error.is_none(),
        "original ROM failed to run: {} (after {} instructions)",
        a.error.unwrap_or_default(),
        a.instructions
    );

    let b = run_rom(&hybrid.image, &cfg);
    assert!(
        b.error.is_none(),
        "hybrid ROM failed to run: {} (after {} instructions)",
        b.error.unwrap_or_default(),
        b.instructions
    );

    match compare_traces(&a, &b) {
        Ok(()) => eprintln!("hybrid trace matches the original"),
        Err(divergence) => {
            // Census diagnostics: how far apart are the traces?
            let mut diffs: Vec<usize> = Vec::new();
            for (i, (x, y)) in a.ram.iter().zip(b.ram.iter()).enumerate() {
                if x != y && (0xFF0000 + i as u32) < a.stack_floor.min(b.stack_floor) {
                    diffs.push(i);
                }
            }
            panic!(
                "hybrid diverged from the original: {divergence}; {} semantic RAM bytes \
                 differ, first: {}; instructions {} vs {}",
                diffs.len(),
                diffs
                    .first()
                    .map(|i| format!("0x{:06X}", 0xFF0000 + i))
                    .unwrap_or_else(|| "?".into()),
                a.instructions,
                b.instructions
            );
        }
    }
}

/// Data-only C rebuild: all original code erased, only data survives at its
/// original addresses, executable code is 100% compiled from the decompiled
/// C. Milestone: behavioral equality with the original, as with the hybrid.
#[test]
fn tetris_data_only_rebuild_behaves_identically() {
    let Some(path) = tetris_rom_path() else {
        eprintln!("skipping: Tetris ROM not found (set SMDC_TETRIS_ROM to override)");
        return;
    };
    let Ok(original) = std::fs::read(&path) else {
        eprintln!("skipping: cannot read {}", path.display());
        return;
    };

    let rebuild = build_data_only_rebuild(&original).expect("data-only rebuild failed");

    // The stripped image must differ from the original inside the code
    // region (proof that erasure happened) but keep its size.
    assert_eq!(rebuild.image.len() >= original.len(), true);
    let erased = rebuild
        .image
        .iter()
        .zip(original.iter())
        .filter(|(r, o)| **r == 0xFF && **o != 0xFF)
        .count();
    assert!(erased > 0x400, "expected erasure of original code, got {erased} bytes");

    let cfg = RunConfig {
        frames: 40,
        frame_instructions: 1_000_000,
    };
    let a = run_rom(&original, &cfg);
    assert!(
        a.error.is_none(),
        "original ROM failed to run: {}",
        a.error.unwrap_or_default()
    );
    let b = run_rom(&rebuild.image, &cfg);
    assert!(
        b.error.is_none(),
        "data-only rebuild failed to run: {} (after {} instructions)",
        b.error.unwrap_or_default(),
        b.instructions
    );

    match compare_traces(&a, &b) {
        Ok(()) => eprintln!("data-only rebuild trace matches the original"),
        Err(divergence) => panic!(
            "data-only rebuild diverged from the original: {divergence}; instructions {} vs {}",
            a.instructions, b.instructions
        ),
    }
}
