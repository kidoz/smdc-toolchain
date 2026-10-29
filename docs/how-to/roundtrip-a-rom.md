# Round-trip a ROM through assembly

This guide takes an existing Mega Drive ROM image, produces a re-assemblable
`.s` source, and rebuilds a byte-identical ROM from it. Use it to inspect or
patch foreign ROMs, or as the starting point for a full disassembly project.
For the C-like pseudo-source view instead, use `--decompile`.

## 1. Build the compiler

From the repository root:

```bash
cargo build
```

## 2. Disassemble the ROM with full coverage

```bash
./target/debug/smdc game.gen --disasm --full -o game.s
```

The listing contains:

- The vector table and ROM header (`0x000-0x1FF`) passed through as `dc.l`,
  so the rebuilt image keeps the original header and checksum.
- Code discovered by following every exception vector, calls, branches and
  PC-relative jump tables, with `F_`, `L_` and `DAT_` labels.
- Everything else as `dc.w`/`dc.b` data, which preserves the bytes exactly.

Symbols from a `game.sym` file (or `--sym`) are used for label names when
present.

## 3. Reassemble the source

```bash
./target/debug/smdc game.s -o game_rebuilt.gen
```

The `.s` (or `.asm`) extension selects the assembler automatically.

## 4. Verify the round trip

Compare the images:

```bash
cmp game.gen game_rebuilt.gen
```

An identical result is expected. `cargo test -p smdc --test tetris_roundtrip`
runs the same check automatically for `tmp/roms/Tetris (Japan).gen` when that
file exists (set `SMDC_TETRIS_ROM` to point at another ROM).

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `parse error: bad branch target: $XXXXXX` | A control transfer names an address that is not a label (odd, outside the image, or inside a resolved jump table). This indicates a decoder gap; the region falls back to `dc.w` data once fixed. |
| `assembly error: unresolved symbol: DAT_XXXXXX` | A label was referenced but never emitted — it landed inside an instruction. Report it as a bug; the disassembler removes such labels automatically. |
| `short branch ... out of range` | The reassembled layout drifted: some instruction encoded to a different length than the layout pass predicted. Run with `SMDC_VERIFY_LAYOUT=1` to print the offending instruction. |
| Rebuilt ROM differs in a data region | An instruction the sweep accepted from data re-encodes differently. Set `SMDC_VERIFY_LAYOUT=1`, check the printed items, and compare with the original bytes. |

## Notes

- Undecodable or ambiguous encodings deliberately become `dc.w` words: the
  rebuilt ROM stays byte-identical, at the cost of showing them as data.
- The listing grammar is exactly what the toolchain emits; see
  `crates/smdc/src/backend/m68k/asmparse.rs` for the accepted syntax.

## Reading the ROM as C

For a pseudo-C view instead of assembly:

```bash
./target/debug/smdc game.gen --decompile -o game.c
```

The decompiler reconstructs foreign calling conventions: functions without
an A6 frame get register arguments from a read-before-write analysis
(`int F_003076(int a0)`), call sites assign return values
(`d0 = F_000612();`), hardware registers render as `VDP_CTRL`-style defines,
and work-RAM globals become typed `ram_FFFFxx` defines with absolute-short
addresses sign-extended. Interrupt handlers entered from the vector table
take no arguments.

The lifted C is meant for reading, but it can also be *recompiled and
executed*: the hybrid and data-only rebuilders compile it back and verify
behavioral equality against the original under the deterministic
interpreter. See
[rebuild a ROM from decompiled C](rebuild-a-rom-from-decompiled-c.md).
