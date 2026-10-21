# MIDI sound driver reference

Reference for `smdc` input files ending in `.mid` or `.midi`. For a procedure,
see [Render a MIDI file to a ROM](../how-to/render-a-midi-file-to-rom.md). For
the reasoning behind the design, see
[Mega Drive MIDI sound driver design](../explanation/midi-sound-driver.md).

## Invocation

| Option | Behavior |
|---|---|
| `--lang midi` | Forces the MIDI frontend; otherwise `.mid`/`.midi` selects it |
| `-t rom` | Emits a ROM image |
| `-t asm` | Emits Motorola 68000 assembly |
| `--emit-music-source <path>` | Writes the generated C driver and event data |
| `-v`, `--verbose` | Prints track, note, FM channel, drum, event, and frame counts |
| `--domestic-name`, `--overseas-name` | Set the ROM header titles |

## Accepted MIDI input

| Feature | Support |
|---|---|
| SMF format 0 | yes |
| SMF format 1 | yes |
| SMF format 2 or later | rejected |
| Ticks-per-quarter-note timing | yes |
| SMPTE timing | rejected |
| Big-endian header and track lengths | yes |
| Variable-length delta times | yes |
| Running status | yes |
| Note On / Note Off | yes |
| Note On with velocity 0 | treated as Note Off |
| Tempo meta event (`FF 51 03`) | yes |
| End-of-track meta event (`FF 2F`) | yes |
| Other meta events | skipped |
| SysEx events (`F0`, `F7`) | skipped |
| Other channel messages | parsed and ignored |
| Unsupported system messages | rejected |

The default tempo is 500,000 microseconds per quarter note (120 BPM). If the
file has no tempo event at tick 0, that default is used until the first tempo
change.

Timing is converted from MIDI ticks to NTSC 60 Hz frames using the tempo map.
The conversion rounds to the nearest frame, so timing is quantized to 1/60 s.

## Arrangement

The converter does not map every MIDI track independently. It selects three
tracks from the note data (percussion notes are excluded from the selection):

| Part | Track selection | Voices | Note selection at each onset |
|---|---|---|---|
| Bass | Track with the lowest average note | FM voice 0 | Lowest note |
| Rhythm | Remaining track with the most notes | FM voices 1-2 | Lowest two notes |
| Lead | Remaining track with the highest maximum note | FM voice 3 | Highest note |

Each part is reduced to what the FM voices can play: bass and lead are
monophonic, and rhythm is limited to two simultaneous notes. If the file has
fewer tracks, some parts are simply absent.

Percussion is collected separately from MIDI channel 10 (zero-based channel 9)
across all tracks:

| Drum id | Role | MIDI notes |
|---|---|---|
| 0 | Kick | 35, 36 |
| 1 | Snare | 38, 40 |
| 2 | Tom | 41, 43, 45, 47, 48, 50 |
| 3 | Hi-hat / noise | 42, 44, 46, 49, 51, 52, 55, 57, 59 |

Other notes on channel 10 are ignored.

Pitch bend, modulation, program changes, and aftertouch are not applied: the
driver selects fixed FM patches per virtual channel.

## Generated C

The generated source contains three macros, eight event arrays, and the driver:

| Symbol | Meaning |
|---|---|
| `EVENT_COUNT` | Number of sequenced events |
| `SONG_FRAMES` | Frames before the song loops |
| `SONG_FM_CHANNELS` | FM channel slots maintained by the driver (6) |
| `event_frame_lo`, `event_frame_hi` | Event start frame, as a 16-bit little-endian pair |
| `event_duration_lo`, `event_duration_hi` | Event duration in frames, as a 16-bit little-endian pair |
| `event_vchannel` | `0` = melody, `1` = bass, `2` = drums |
| `event_voice` | FM voice `0`-`3` for melodic events; `255` for drums |
| `event_note` | MIDI note for melodic events; drum id `0`-`3` for drums |
| `event_velocity` | MIDI velocity, `1`-`127` |

The driver defines `music_init()` and `music_update()` and supplies its own
`main()`. At runtime `main()` initializes the VDP, YM2612, and PSG, then loops
on VBlank:

1. release expired FM and PSG tone notes
2. dispatch every event whose frame is less than or equal to the current frame
3. update the PSG drum envelopes
4. increment the frame counter and restart at `SONG_FRAMES`

A melodic event starts an FM note on its assigned voice and also mirrors the
note into the two-channel PSG tone pool. A drum event triggers one of the four PSG
drum voices: kick and tom use tone channel 2, snare and hi-hat use the noise
channel (channel 3). The driver also writes palette colors as a simple visual
indicator of note and drum activity.

`SONG_FRAMES` is derived from the last note end frame plus 60 frames (one
second at 60 Hz).

## Limits

| Limit | Value |
|---|---|
| Maximum event frame | 65,535 (16-bit), about 18 minutes at 60 Hz |
| Maximum event duration | 65,535 frames |
| Simultaneous FM notes per part | Bass 1, rhythm 2, lead 1 |
| FM voices addressed by the arrangement | 4 of the driver's 6 slots |
| PSG drum voices | 4 (kick, snare, tom, hi-hat/noise) |

## Error messages

| Message | Cause |
|---|---|
| `missing MIDI header chunk` | Input does not start with `MThd` |
| `MIDI header chunk is too short` | Malformed header |
| `unsupported MIDI format N; only format 0 and 1 are supported` | SMF format 2+ |
| `SMPTE time-division MIDI files are not supported` | SMPTE timing |
| `MIDI ticks-per-quarter division is zero` | Invalid header |
| `missing MIDI track chunk` | Expected `MTrk` chunk not found |
| `invalid MIDI variable-length quantity` | Malformed delta time or length |
| `MIDI running status used before status byte` | Malformed event stream |
| `unsupported MIDI status byte 0x..` | Unsupported system message |
| `MIDI file does not contain note events that can be rendered` | No usable Note On/Off pairs |
| `MIDI song is too long for the compact Mega Drive event stream at frame N` | Frame exceeds 16 bits |

## Source of truth

- `crates/smdc/src/music/midi.rs` — SMF parser
- `crates/smdc/src/music/mod.rs` — note collection, arrangement, C generation,
  and the driver template
- `crates/smdc/src/main.rs` — CLI entry point and MIDI input detection
