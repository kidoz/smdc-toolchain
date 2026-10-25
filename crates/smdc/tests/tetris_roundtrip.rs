//! Byte-exact round-trip gate for a foreign ROM (Tetris).
//!
//! ROM -> `--disasm --full` listing -> parse -> assemble must reproduce the
//! original image byte for byte. The ROM is copyrighted and lives outside
//! version control; the test skips gracefully when it is absent.

use smd_compiler::backend::m68k::{disassemble_full, parse_assembly, Assembler};
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
