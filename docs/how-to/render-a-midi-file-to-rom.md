# Render a MIDI file to a ROM

This guide turns a Standard MIDI File into a Mega Drive ROM that plays the song
through the YM2612 FM chip and the PSG. It assumes you have already built the
compiler.

## 1. Check the MIDI file

`smdc` accepts Standard MIDI File (SMF) formats 0 and 1 with ticks-per-quarter
timing. It rejects SMPTE timing and every other format. For the full supported
subset, see the [MIDI sound driver reference](../reference/midi-sound-driver.md).

## 2. Build the compiler

From the repository root:

```bash
cargo build
```

The binary is `target/debug/smdc`.

## 3. Render the ROM

```bash
./target/debug/smdc song.mid -o song.bin -t rom \
  --domestic-name "SONG NAME" \
  --overseas-name "SONG NAME"
```

The `.mid` or `.midi` extension selects the MIDI frontend automatically. Use
`--lang midi` if the file has a different extension.

The same command through Cargo:

```bash
cargo run -p smdc -- song.mid -o song.bin -t rom
```

Useful options:

| Option | Effect |
|---|---|
| `-t rom` | Emit a ROM instead of assembly |
| `-t asm` | Emit Motorola 68000 assembly instead of a ROM |
| `--emit-music-source song.c` | Also write the generated C driver and event data |
| `-v` | Print track, note, event, and frame counts |
| `--domestic-name`, `--overseas-name` | Set the titles in the ROM header |

To save every intermediate artifact:

```bash
./target/debug/smdc song.mid -o song.s -t asm --emit-music-source song.c
./target/debug/smdc song.mid -o song.bin -t rom
```

## 4. Run the ROM

Load `song.bin` in a Mega Drive/Genesis emulator. The generated program waits
for VBlank, dispatches events due on the current 60 Hz frame, and restarts the
song when it reaches the end. The length is printed by `-v` as a frame count.

## Troubleshooting

| Message or symptom | Meaning | What to do |
|---|---|---|
| `unsupported MIDI format N; only format 0 and 1 are supported` | The file is SMF format 2 or newer | Export it as SMF format 0 or 1 |
| `SMPTE time-division MIDI files are not supported` | The file uses SMPTE timing instead of ticks per quarter note | Re-export with PPQ timing |
| `MIDI ticks-per-quarter division is zero` | Invalid header | Re-export the file |
| `MIDI file does not contain note events that can be rendered` | No usable Note On/Off pairs | Check the file in a MIDI editor |
| `MIDI song is too long for the compact Mega Drive event stream at frame N` | The song exceeds the 16-bit frame range (about 18 minutes at 60 Hz) | Shorten or trim the song |
| `missing MIDI header chunk` / `missing MIDI track chunk` | Corrupt or non-SMF input | Re-export the file |
| `unsupported MIDI status byte 0x..` | A system message outside the supported subset | Re-export as a plain SMF 0/1 file |

## What happens under the hood

The host-side converter reads the MIDI into note events, chooses bass, rhythm,
and lead tracks, maps percussion to four PSG drum voices, and emits C arrays
plus a 60 Hz 68000 sequencer. That generated C is then compiled like any other
C source. See the [reference](../reference/midi-sound-driver.md) for the exact
mapping and the [design notes](../explanation/midi-sound-driver.md) for why the
driver runs on the 68000 instead of the Z80.
