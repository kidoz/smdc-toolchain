# Mega Drive MIDI sound driver design

This page explains the constraints and trade-offs behind the `smdc` MIDI music
path. For the supported input subset and generated symbols, see the
[MIDI sound driver reference](../reference/midi-sound-driver.md). For a
procedure, see [Render a MIDI file to a ROM](../how-to/render-a-midi-file-to-rom.md).

## The problem

A Standard MIDI File describes notes, tempo, and channels for an arbitrarily
large set of instruments. A Mega Drive has six YM2612 FM channels, four PSG
channels, and no General MIDI instrument set. The driver therefore has to:

1. decide which parts of the MIDI arrangement to keep
2. map those parts onto the available FM and PSG voices
3. schedule every note against the console's video frame clock
4. do all of this with the language and tooling the compiler already supports

## Hardware constraints

### YM2612 FM

- Register access is split into two banks: `$A04000`/`$A04001` for channels 1-3
  and `$A04002`/`$A04003` for channels 4-6.
- The status register exposes a busy bit, so register writes must be paced. The
  generated driver uses a short delay loop after each address and data write.
- The key-on register is `$28`. Starting a note means key-off, writing the high
  frequency byte to `$A4`, writing the low frequency byte to `$A0`, then
  key-on.
- Frequency is encoded as a block plus F-number. The driver uses a block-4
  twelve-note F-number table and derives the block from the MIDI note number,
  clamped to the valid range.

### PSG

- The PSG is written through `$C00011`.
- It provides three square-wave tone channels and one noise channel.
- Volume is attenuation: 15 is silence.
- There are no hardware envelopes. Drum decay is therefore implemented by
  changing the tone or noise volume on every video frame.

## Why a 68000-side sequencer

The driver is generated C that runs on the 68000 at 60 Hz. It is not a
Z80-resident sound driver. That choice keeps the music path inside the existing
compiler pipeline: the generated C is compiled by the same frontend and
backend as any other source file. It also avoids introducing a Z80 assembler or
a binary blob format just for audio.

The cost is that the sequencer shares the 68000 with game code and that all
timing is quantized to video frames. The benefit is a driver that can be
inspected, regenerated, and built with the rest of the toolchain.

## From MIDI to an event stream

The host-side converter parses the MIDI file, converts ticks to NTSC frames
using the tempo map, and reduces the arrangement:

- The track with the lowest average pitch becomes the bass.
- The remaining track with the most notes becomes the rhythm part.
- The remaining track with the highest maximum pitch becomes the lead.
- Notes on MIDI channel 10 are treated as percussion and mapped onto four PSG
  drum voices.

This heuristic exists because a MIDI file can contain far more tracks and
voices than the YM2612 can play. Rather than mixing everything down, the
converter keeps the parts that usually carry the song's identity: bass, a
rhythmic chord part, and a lead line. Each part is then reduced to the number
of voices available: monophonic for bass and lead, two notes at a time for
rhythm.

The result is a compact event stream. Each event records a frame, a duration, a
virtual channel (melody, bass, or drums), a voice, a note, and a velocity. The
converter emits these as C arrays followed by the driver, and the compiler
turns the whole thing into 68000 code. Sixteen-bit frame and duration values
are stored as separate low and high byte arrays.

At runtime the loop is deliberately small: wait for VBlank, release expired
notes, dispatch all events due on the current frame, update the PSG drum
envelopes, advance the frame counter, and restart when `SONG_FRAMES` is
reached.

## What the design trades away

- **Tempo is frame-quantized.** Everything runs at 60 Hz, so timing is rounded
  to the nearest frame rather than sample-accurate.
- **Expression is limited.** Pitch bend, modulation, program changes, and
  aftertouch are ignored; each virtual channel uses a fixed FM patch.
- **Parts are reduced.** Bass and lead are monophonic, and rhythm is limited to
  two simultaneous notes.
- **Percussion is approximate.** Four PSG voices stand in for a full drum kit.
- **Song length is bounded.** Event frames are 16-bit, which caps a song at
  about 18 minutes.

These are conscious trade-offs for a driver that fits the current compiler and
hardware model, not limitations of the YM2612 itself.

## Worked example: "Paranoid"

`Black Sabbath - Paranoid.mid` is an SMF format-1 file with 9 tracks and 192
ticks per quarter note. The current converter renders it as 2,322 sequencer
events over 10,154 frames (about 169 seconds).

Artifacts from this render live in `demo/`:

- `demo/paranoid_from_midi.c`
- `demo/paranoid_from_midi.s`
- `demo/paranoid_from_midi.bin`

To regenerate the source and ROM:

```bash
./target/debug/smdc "Black Sabbath - Paranoid.mid" \
  -o demo/paranoid_from_midi.s -t asm \
  --emit-music-source demo/paranoid_from_midi.c
./target/debug/smdc "Black Sabbath - Paranoid.mid" \
  -o demo/paranoid_from_midi.bin -t rom \
  --domestic-name "PARANOID SMDC" \
  --overseas-name "PARANOID SMDC"
```

The MIDI file itself is not part of the repository; bring your own copy to
reproduce this example.

## Sources

- MegaDrive Development Wiki, YM2612 ports and status:
  https://wiki.megadrive.org/index.php?title=YM2612
- MegaDrive Development Wiki, YM2612 register layout:
  https://wiki.megadrive.org/index.php?title=YM2612_Registers
- Plutiedev, YM2612 register reference and bank/operator details:
  https://www.plutiedev.com/ym2612-registers
- Plutiedev, common YM2612 note/volume operations:
  https://www.plutiedev.com/ym2612-operations
- Plutiedev, PSG channels, port, tone/noise/volume model:
  https://plutiedev.com/psg
- MIDI Association, Standard MIDI Files overview:
  https://midi.org/standard-midi-files
