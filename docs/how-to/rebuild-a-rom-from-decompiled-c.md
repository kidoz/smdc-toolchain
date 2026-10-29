# Rebuild a ROM from decompiled C

Recompile a foreign ROM's code from its decompiled C and verify that the
result behaves identically to the original. Two rebuild shapes exist:

- **Hybrid**: the original image stays as the substrate; every discovered
  function is replaced by its compiled C, appended after the original image.
  Original data, undiscovered code and the checksum range are untouched.
- **Data-only**: every flow-verified instruction byte of the original is
  erased, so only data (tables, tiles, strings, header) survives at its
  original addresses — all executed code comes from the compiled C. The boot
  checksum is recomputed so the ROM's self-check still passes.

Prerequisites: a built workspace (`cargo build`) and a ROM image. The
behavioral gates are wired to `tmp/roms/Tetris (Japan).gen`; set
`SMDC_TETRIS_ROM` to point them at any other ROM.

## 1. Run the behavioral gates

From the repository root, with a ROM in place:

```bash
cargo test -p smdc --test tetris_roundtrip
```

Six gates run when the ROM is present (they skip with a notice otherwise):

| Gate | Verifies |
|---|---|
| `tetris_round_trips_byte_exactly` | `.s` listing reassembles to the identical image |
| `tetris_decompiles_to_compiling_pseudo_c` | the lifted C compiles |
| `tetris_byte_exact_rebuild_behaves_identically` | byte-exact rebuild runs identically |
| `tetris_hybrid_behaves_identically` | hybrid behaves identically (idle) |
| `tetris_data_only_rebuild_behaves_identically` | data-only rebuild behaves identically (idle) |
| `tetris_data_only_rebuild_plays_identically` | data-only rebuild behaves identically under scripted controller input |

"Behaves identically" means: no interpreter error, equal final work RAM
(excluding each run's private stack window), and an identical VDP write
sequence.

## 2. Point the gates at your own ROM

```bash
SMDC_TETRIS_ROM=/path/to/rom.gen cargo test -p smdc --test tetris_roundtrip
```

A ROM the pipeline has not been tuned for may expose new lifter or
interpreter gaps — a failing gate is a census starting point, not a dead end
(see [the pipeline explanation](../explanation/decompile-recompile-pipeline.md)).

## 3. Dump the hybrid image and its map

```bash
SMDC_DUMP_HYBRID=1 cargo test -p smdc --test tetris_roundtrip tetris_hybrid_behaves_identically
```

This writes `tmp/hybrid.gen` (the hybrid image), `tmp/hybrid.c` (the
decompiled source it was compiled from) and `tmp/hybrid.map`
(`original address -> compiled address` for every replaced function).

## 4. Use the builders from your own code

The builders are a library API on the `smd_compiler` crate (there is no CLI
flag for them):

```rust
use smd_compiler::backend::m68k::{
    build_data_only_rebuild, build_hybrid_rom, compare_traces, run_rom, RunConfig,
};

let original = std::fs::read("game.gen")?;

// Hybrid: original image + compiled functions appended.
let hybrid = build_hybrid_rom(&original)?;
// Data-only: original code erased, compiled code only.
let rebuild = build_data_only_rebuild(&original)?;

let cfg = RunConfig {
    frames: 40,
    frame_instructions: 7_600_000,
    input: vec![],
};
let a = run_rom(&original, &cfg);
let b = run_rom(&rebuild.image, &cfg);
compare_traces(&a, &b)?; // Err describes the first divergence
```

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `skipping: Tetris ROM not found` | No ROM at the default path; set `SMDC_TETRIS_ROM`. |
| `no free work-RAM window for the compiled stack` | The probe run found no large untouched RAM region below the program's deepest stack use. A program that scribbles over all of work RAM leaves nowhere to put the compiled code's private stack. |
| `work RAM differs at 0xXXXXXX: ... stacks [...] excluded` | Genuine semantic divergence between the runs (the message lists the stack windows that were excluded from comparison). |
| `VDP write sequences differ at index N` | The runs performed different hardware-side operations. Index N of the longer run is where they first disagree. |
| A gate takes minutes | The behavioral gates interpret hundreds of millions of instructions per run by design; this is the cost of the equality guarantee. |

## Notes

- The compiled code runs on a private stack: the builder repoints the
  initial stack pointer (vector 0) at the top of the largest work-RAM window
  a probe run shows the original never touches. The compiled frames are far
  larger than the original's budget, and with the original stack pointer
  they grow straight through the program's data.
- The behavioral budget (7.6M instructions per simulated frame, VBlank at
  90% of it) is a test-pacing convention sized so the compiled setup
  finishes inside the first frame — it is not a real-time simulation of the
  hardware. Both runs use the same config.

## See also

- [Round-trip a ROM through assembly](roundtrip-a-rom.md) — the byte-exact
  `.s` path and the `--decompile` reading view.
- [Why the decompile/recompile pipeline is shaped this way](../explanation/decompile-recompile-pipeline.md)
  — the oracle design, the rebuild strategies and the census method used to
  close divergences.
