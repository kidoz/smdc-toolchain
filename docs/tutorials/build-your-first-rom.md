# Build your first Mega Drive ROM

In this tutorial you will compile a small C program into a Sega Mega Drive/Genesis
ROM and run it in an emulator. It takes about five minutes.

## Before you start

You need:

- a Rust toolchain with `cargo`
- a Mega Drive/Genesis emulator that can load `.bin` ROM images
- this repository checked out

## Step 1: Build the compiler

From the repository root:

```bash
cargo build
```

This produces the `smdc` compiler at `target/debug/smdc`.

## Step 2: Write a minimal program

Create a file named `first_rom.c`:

```c
/*
 * Minimal VDP test: fill the screen with red.
 */

int VDP_DATA_ADDR = 0xC00000;
int VDP_CTRL_ADDR = 0xC00004;

int main(void) {
    short *vdp_ctrl;
    short *vdp_data;

    vdp_ctrl = (short *)VDP_CTRL_ADDR;
    vdp_data = (short *)VDP_DATA_ADDR;

    // Enable the display (VDP register 1).
    *vdp_ctrl = 0x8144;

    // Set the background color to palette 0, color 1 (VDP register 7).
    *vdp_ctrl = 0x8701;

    // Point CRAM at color 1 and write red.
    *vdp_ctrl = 0xC002;
    *vdp_ctrl = 0x0000;
    *vdp_data = 0x000E;

    // Keep the program running.
    while (1) {
    }

    return 0;
}
```

The program talks to the Video Display Processor through its memory-mapped
registers: `$C00000` for data and `$C00004` for control. It enables the display,
selects color 1 of palette 0 as the background color, and writes red into that
palette entry.

The compiler accepts a C subset, so this example deliberately uses only simple
declarations, pointer casts, and a loop.

## Step 3: Compile the program to a ROM

```bash
./target/debug/smdc first_rom.c -o first_rom.bin -t rom
```

`-t rom` tells the backend to package the generated 68000 code as a cartridge
image. The result is a 64 KiB `first_rom.bin`.

Without `-t rom`, `smdc` emits Motorola 68000 assembly instead:

```bash
./target/debug/smdc first_rom.c -o first_rom.s
```

## Step 4: Run the ROM

Load `first_rom.bin` in your emulator as a Mega Drive/Genesis cartridge and
start it. You should see a solid red screen.

If the screen stays black, check that the emulator loaded the file as a
cartridge ROM rather than as a raw binary at an arbitrary address.

## What you learned

- `cargo build` produces the compiler at `target/debug/smdc`.
- `smdc <file>.c -o <file>.bin -t rom` compiles C and packages a ROM.
- A Mega Drive program can run without any SDK headers by writing to hardware
  registers directly.
- Omitting `-t rom` produces assembly instead of a ROM.

## Next steps

- Explore the SDK examples in `sdk/c/examples/`, such as `test_vdp.c` and
  `channel_test.c`.
- Render music with
  [Render a MIDI file to a ROM](../how-to/render-a-midi-file-to-rom.md).
- Look up compiler and driver behavior in the
  [reference](../reference/midi-sound-driver.md), or read the
  [MIDI driver design](../explanation/midi-sound-driver.md) for the reasoning
  behind the music pipeline.
