//! Host-side music tooling for Mega Drive audio demos.

mod midi;

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::common::{CompileError, CompileResult};
use midi::{MidiFile, MidiMessage, TempoEvent};

const NTSC_FRAMES_PER_SECOND: u64 = 60;
const DEFAULT_TEMPO_US_PER_QUARTER: u32 = 500_000;
const MIDI_DRUM_CHANNEL: u8 = 9;
const MAX_FM_CHANNELS: usize = 6;

type ActiveNoteMap = HashMap<(u8, u8), Vec<(u64, u8)>>;

#[derive(Debug, Clone)]
pub struct GeneratedMusic {
    pub c_source: String,
    pub stats: MusicStats,
}

#[derive(Debug, Clone)]
pub struct MusicStats {
    pub midi_tracks: usize,
    pub note_events: usize,
    pub rendered_events: usize,
    pub song_frames: u32,
    pub fm_channels: usize,
    pub drum_events: usize,
}

#[derive(Debug, Clone)]
struct NoteEvent {
    start_tick: u64,
    end_tick: u64,
    track_index: usize,
    channel: u8,
    note: u8,
    velocity: u8,
}

#[derive(Debug, Clone)]
struct SequencedEvent {
    frame: u32,
    duration: u16,
    vchannel: VirtualChannel,
    voice: u8,
    note: u8,
    velocity: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum VirtualChannel {
    Melody,
    Bass,
    Drums,
}

impl VirtualChannel {
    const fn as_u8(self) -> u8 {
        match self {
            Self::Melody => 0,
            Self::Bass => 1,
            Self::Drums => 2,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ArrangementTracks {
    bass: Option<usize>,
    rhythm: Option<usize>,
    lead: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct TrackProfile {
    track_index: usize,
    note_count: usize,
    first_tick: u64,
    min_note: u8,
    max_note: u8,
    note_sum: u32,
}

impl TrackProfile {
    fn average_note(self) -> u32 {
        self.note_sum / self.note_count as u32
    }
}

#[derive(Debug, Clone, Copy)]
struct ManualHit {
    step: u32,
    note: u8,
    duration: u16,
    voice: u8,
    vchannel: VirtualChannel,
    velocity: u8,
}

pub fn generate_from_midi(bytes: &[u8], title: &str) -> CompileResult<GeneratedMusic> {
    let midi = MidiFile::parse(bytes).map_err(|err| CompileError::codegen(err.to_string()))?;
    let notes = collect_notes(&midi);
    if notes.is_empty() {
        return Err(CompileError::codegen(
            "MIDI file does not contain note events that can be rendered",
        ));
    }

    let song_frames = notes
        .iter()
        .map(|note| tick_to_frame(&midi, note.end_tick))
        .max()
        .map_or(1, |frame| {
            frame.saturating_add(NTSC_FRAMES_PER_SECOND as u32)
        });
    if let Some(frame) = notes
        .iter()
        .map(|note| tick_to_frame(&midi, note.end_tick))
        .find(|frame| *frame > u32::from(u16::MAX))
    {
        return Err(CompileError::codegen(format!(
            "MIDI song is too long for the compact Mega Drive event stream at frame {frame}"
        )));
    }
    let mut sequenced_events = build_arranged_note_events(&midi, &notes, song_frames);
    sequenced_events.sort_by_key(sequenced_event_sort_key);
    sequenced_events.dedup_by(|a, b| {
        a.frame == b.frame
            && a.duration == b.duration
            && a.vchannel == b.vchannel
            && a.voice == b.voice
            && a.note == b.note
            && a.velocity == b.velocity
    });
    let drum_events = notes
        .iter()
        .filter(|note| note.channel == MIDI_DRUM_CHANNEL && map_drum_note(note.note).is_some())
        .count();
    let c_source = render_c_source(title, &sequenced_events, song_frames);

    Ok(GeneratedMusic {
        c_source,
        stats: MusicStats {
            midi_tracks: midi.tracks.len(),
            note_events: notes.len(),
            rendered_events: sequenced_events.len(),
            song_frames,
            fm_channels: MAX_FM_CHANNELS,
            drum_events,
        },
    })
}

pub fn generate_manual_paranoid_cover(title: &str) -> GeneratedMusic {
    const STEP_FRAMES: u32 = 6;
    const BAR_STEPS: u32 = 16;
    const BAR_FRAMES: u32 = STEP_FRAMES * BAR_STEPS;
    const BAR_COUNT: u32 = 32;
    const SONG_FRAMES: u32 = BAR_FRAMES * BAR_COUNT;

    let mut events = Vec::new();

    for bar in 0..BAR_COUNT {
        let section_bar = bar % 8;
        push_manual_drums(&mut events, bar, BAR_FRAMES, STEP_FRAMES);
        push_manual_bass(&mut events, bar, BAR_FRAMES, STEP_FRAMES, section_bar);
        push_manual_rhythm(&mut events, bar, BAR_FRAMES, STEP_FRAMES, section_bar);
        if (8..24).contains(&bar) {
            push_manual_lead(&mut events, bar, BAR_FRAMES, STEP_FRAMES, section_bar);
        }
    }

    events.sort_by_key(sequenced_event_sort_key);
    let drum_events = events
        .iter()
        .filter(|event| event.vchannel == VirtualChannel::Drums)
        .count();
    let c_source = render_c_source(title, &events, SONG_FRAMES);

    GeneratedMusic {
        c_source,
        stats: MusicStats {
            midi_tracks: 0,
            note_events: events.len(),
            rendered_events: events.len(),
            song_frames: SONG_FRAMES,
            fm_channels: MAX_FM_CHANNELS,
            drum_events,
        },
    }
}

fn push_manual_bass(
    events: &mut Vec<SequencedEvent>,
    bar: u32,
    bar_frames: u32,
    step_frames: u32,
    section_bar: u32,
) {
    let turnaround = section_bar == 3 || section_bar == 7;
    let pattern = if turnaround {
        [
            ManualHit::bass(0, 40, 2),
            ManualHit::bass(2, 40, 2),
            ManualHit::bass(4, 43, 1),
            ManualHit::bass(5, 45, 1),
            ManualHit::bass(6, 40, 2),
            ManualHit::bass(8, 38, 2),
            ManualHit::bass(10, 35, 1),
            ManualHit::bass(11, 38, 1),
            ManualHit::bass(12, 40, 1),
            ManualHit::bass(13, 43, 1),
            ManualHit::bass(14, 40, 2),
        ]
    } else {
        [
            ManualHit::bass(0, 40, 2),
            ManualHit::bass(2, 40, 2),
            ManualHit::bass(4, 43, 1),
            ManualHit::bass(5, 45, 1),
            ManualHit::bass(6, 40, 2),
            ManualHit::bass(8, 40, 2),
            ManualHit::bass(10, 38, 1),
            ManualHit::bass(11, 40, 1),
            ManualHit::bass(12, 43, 1),
            ManualHit::bass(13, 45, 1),
            ManualHit::bass(14, 40, 2),
        ]
    };
    push_manual_hits(events, bar, bar_frames, step_frames, &pattern);
}

fn push_manual_rhythm(
    events: &mut Vec<SequencedEvent>,
    bar: u32,
    bar_frames: u32,
    step_frames: u32,
    section_bar: u32,
) {
    let turnaround = section_bar == 3 || section_bar == 7;
    let roots = if turnaround {
        [(0, 52, 2), (4, 55, 1), (5, 57, 1), (8, 50, 2), (12, 52, 2)]
    } else {
        [(0, 52, 2), (4, 55, 1), (5, 57, 1), (8, 52, 2), (12, 55, 1)]
    };

    for (step, root, duration) in roots {
        push_manual_hit(
            events,
            bar,
            bar_frames,
            step_frames,
            ManualHit::guitar(1, step, root, duration, 118),
        );
        push_manual_hit(
            events,
            bar,
            bar_frames,
            step_frames,
            ManualHit::guitar(2, step, root + 7, duration, 112),
        );
    }
}

fn push_manual_lead(
    events: &mut Vec<SequencedEvent>,
    bar: u32,
    bar_frames: u32,
    step_frames: u32,
    section_bar: u32,
) {
    let phrase = match section_bar {
        0 | 4 => [
            (0, 64),
            (2, 67),
            (4, 69),
            (6, 71),
            (8, 69),
            (10, 67),
            (12, 64),
            (14, 62),
        ],
        1 | 5 => [
            (0, 64),
            (2, 64),
            (4, 67),
            (6, 69),
            (8, 72),
            (10, 71),
            (12, 69),
            (14, 67),
        ],
        2 | 6 => [
            (0, 76),
            (2, 74),
            (4, 72),
            (6, 71),
            (8, 69),
            (10, 67),
            (12, 64),
            (14, 67),
        ],
        _ => [
            (0, 69),
            (2, 67),
            (4, 64),
            (6, 62),
            (8, 64),
            (10, 67),
            (12, 64),
            (14, 59),
        ],
    };

    for (step, note) in phrase {
        push_manual_hit(
            events,
            bar,
            bar_frames,
            step_frames,
            ManualHit::lead(step, note, 2),
        );
    }
}

fn push_manual_drums(
    events: &mut Vec<SequencedEvent>,
    bar: u32,
    bar_frames: u32,
    step_frames: u32,
) {
    for step in [0, 2, 4, 6, 8, 10, 12, 14] {
        push_manual_hit(
            events,
            bar,
            bar_frames,
            step_frames,
            ManualHit::drum(step, 3, 4, 76),
        );
    }
    for step in [0, 6, 8, 14] {
        push_manual_hit(
            events,
            bar,
            bar_frames,
            step_frames,
            ManualHit::drum(step, 0, 8, 124),
        );
    }
    for step in [4, 12] {
        push_manual_hit(
            events,
            bar,
            bar_frames,
            step_frames,
            ManualHit::drum(step, 1, 7, 118),
        );
    }
}

fn push_manual_hits(
    events: &mut Vec<SequencedEvent>,
    bar: u32,
    bar_frames: u32,
    step_frames: u32,
    hits: &[ManualHit],
) {
    for hit in hits {
        push_manual_hit(events, bar, bar_frames, step_frames, *hit);
    }
}

fn push_manual_hit(
    events: &mut Vec<SequencedEvent>,
    bar: u32,
    bar_frames: u32,
    step_frames: u32,
    hit: ManualHit,
) {
    events.push(SequencedEvent {
        frame: (bar * bar_frames) + (hit.step * step_frames),
        duration: hit.duration,
        vchannel: hit.vchannel,
        voice: hit.voice,
        note: hit.note,
        velocity: hit.velocity,
    });
}

impl ManualHit {
    const fn bass(step: u32, note: u8, step_duration: u16) -> Self {
        Self {
            step,
            note,
            duration: step_duration * 6,
            voice: 0,
            vchannel: VirtualChannel::Bass,
            velocity: 116,
        }
    }

    const fn guitar(voice: u8, step: u32, note: u8, step_duration: u16, velocity: u8) -> Self {
        Self {
            step,
            note,
            duration: step_duration * 6,
            voice,
            vchannel: VirtualChannel::Melody,
            velocity,
        }
    }

    const fn lead(step: u32, note: u8, step_duration: u16) -> Self {
        Self {
            step,
            note,
            duration: step_duration * 6,
            voice: 3,
            vchannel: VirtualChannel::Melody,
            velocity: 104,
        }
    }

    const fn drum(step: u32, note: u8, duration: u16, velocity: u8) -> Self {
        Self {
            step,
            note,
            duration,
            voice: 255,
            vchannel: VirtualChannel::Drums,
            velocity,
        }
    }
}

fn collect_notes(midi: &MidiFile) -> Vec<NoteEvent> {
    let mut notes = Vec::new();

    for (track_index, track) in midi.tracks.iter().enumerate() {
        let mut active: ActiveNoteMap = HashMap::new();

        for event in &track.events {
            match event.message {
                MidiMessage::NoteOn {
                    channel,
                    note,
                    velocity,
                } if velocity > 0 => {
                    active
                        .entry((channel, note))
                        .or_default()
                        .push((event.tick, velocity));
                }
                MidiMessage::NoteOff { channel, note }
                | MidiMessage::NoteOn { channel, note, .. } => {
                    if let Some(stack) = active.get_mut(&(channel, note))
                        && let Some((start_tick, velocity)) = stack.pop()
                        && event.tick > start_tick
                    {
                        notes.push(NoteEvent {
                            start_tick,
                            end_tick: event.tick,
                            track_index,
                            channel,
                            note,
                            velocity,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    notes.sort_by_key(|note| (note.start_tick, note.track_index, note.channel, note.note));
    notes
}

fn tick_to_frame(midi: &MidiFile, tick: u64) -> u32 {
    let micros = tick_to_micros(tick, midi.division, &midi.tempos);
    ((micros * NTSC_FRAMES_PER_SECOND + 500_000) / 1_000_000) as u32
}

fn tick_to_micros(tick: u64, division: u16, tempos: &[TempoEvent]) -> u64 {
    let ticks_per_quarter = u64::from(division);
    let mut current_tick = 0;
    let mut current_tempo = DEFAULT_TEMPO_US_PER_QUARTER;
    let mut micros = 0u64;

    for tempo in tempos {
        if tempo.tick > tick {
            break;
        }
        let delta = tempo.tick.saturating_sub(current_tick);
        micros += delta * u64::from(current_tempo) / ticks_per_quarter;
        current_tick = tempo.tick;
        current_tempo = tempo.micros_per_quarter;
    }

    let delta = tick.saturating_sub(current_tick);
    micros + delta * u64::from(current_tempo) / ticks_per_quarter
}

fn map_drum_note(note: u8) -> Option<u8> {
    match note {
        35 | 36 => Some(0),
        38 | 40 => Some(1),
        41 | 43 | 45 | 47 | 48 | 50 => Some(2),
        42 | 44 | 46 | 49 | 51 | 52 | 55 | 57 | 59 => Some(3),
        _ => None,
    }
}

fn build_arranged_note_events(
    midi: &MidiFile,
    notes: &[NoteEvent],
    song_frames: u32,
) -> Vec<SequencedEvent> {
    let tracks = choose_arrangement_tracks(notes);
    let mut events = Vec::new();

    if let Some(track_index) = tracks.bass {
        push_monophonic_track_events(
            &mut events,
            midi,
            notes,
            track_index,
            VirtualChannel::Bass,
            0,
            SelectNote::Lowest,
        );
    }
    if let Some(track_index) = tracks.rhythm {
        push_polyphonic_track_events(&mut events, midi, notes, track_index, 1, 2);
    }
    if let Some(track_index) = tracks.lead {
        push_monophonic_track_events(
            &mut events,
            midi,
            notes,
            track_index,
            VirtualChannel::Melody,
            3,
            SelectNote::Highest,
        );
    }

    push_drum_events(&mut events, midi, notes);

    for event in &mut events {
        if event.frame.saturating_add(u32::from(event.duration)) > song_frames {
            event.duration = song_frames.saturating_sub(event.frame).max(1) as u16;
        }
    }

    events
}

#[derive(Debug, Clone, Copy)]
enum SelectNote {
    Lowest,
    Highest,
}

fn choose_arrangement_tracks(notes: &[NoteEvent]) -> ArrangementTracks {
    let profiles = track_profiles(notes);
    let bass = profiles
        .iter()
        .min_by_key(|profile| (profile.average_note(), profile.first_tick))
        .map(|profile| profile.track_index);

    let rhythm = profiles
        .iter()
        .filter(|profile| Some(profile.track_index) != bass)
        .min_by_key(|profile| (usize::MAX - profile.note_count, profile.first_tick))
        .map(|profile| profile.track_index);

    let lead = profiles
        .iter()
        .filter(|profile| Some(profile.track_index) != bass && Some(profile.track_index) != rhythm)
        .min_by_key(|profile| (u8::MAX - profile.max_note, profile.note_count))
        .map(|profile| profile.track_index);

    ArrangementTracks { bass, rhythm, lead }
}

fn track_profiles(notes: &[NoteEvent]) -> Vec<TrackProfile> {
    let mut profiles: Vec<TrackProfile> = Vec::new();

    for note in notes {
        if note.channel == MIDI_DRUM_CHANNEL {
            continue;
        }

        if let Some(profile) = profiles
            .iter_mut()
            .find(|profile| profile.track_index == note.track_index)
        {
            profile.note_count += 1;
            profile.first_tick = profile.first_tick.min(note.start_tick);
            profile.min_note = profile.min_note.min(note.note);
            profile.max_note = profile.max_note.max(note.note);
            profile.note_sum += u32::from(note.note);
        } else {
            profiles.push(TrackProfile {
                track_index: note.track_index,
                note_count: 1,
                first_tick: note.start_tick,
                min_note: note.note,
                max_note: note.note,
                note_sum: u32::from(note.note),
            });
        }
    }

    profiles
}

fn push_monophonic_track_events(
    events: &mut Vec<SequencedEvent>,
    midi: &MidiFile,
    notes: &[NoteEvent],
    track_index: usize,
    vchannel: VirtualChannel,
    voice: u8,
    select: SelectNote,
) {
    let mut index = 0usize;
    while index < notes.len() {
        let start_tick = notes[index].start_tick;
        let mut selected: Option<&NoteEvent> = None;

        while index < notes.len() && notes[index].start_tick == start_tick {
            let note = &notes[index];
            if note.track_index == track_index && note.channel != MIDI_DRUM_CHANNEL {
                selected = match (selected, select) {
                    (None, _) => Some(note),
                    (Some(current), SelectNote::Lowest) if note.note < current.note => Some(note),
                    (Some(current), SelectNote::Highest) if note.note > current.note => Some(note),
                    (other, _) => other,
                };
            }
            index += 1;
        }

        if let Some(note) = selected {
            push_note_event(events, midi, note, vchannel, voice);
        }
    }
}

fn push_polyphonic_track_events(
    events: &mut Vec<SequencedEvent>,
    midi: &MidiFile,
    notes: &[NoteEvent],
    track_index: usize,
    voice_base: u8,
    voice_count: usize,
) {
    let mut index = 0usize;
    while index < notes.len() {
        let start_tick = notes[index].start_tick;
        let mut group = Vec::new();

        while index < notes.len() && notes[index].start_tick == start_tick {
            let note = &notes[index];
            if note.track_index == track_index && note.channel != MIDI_DRUM_CHANNEL {
                group.push(note);
            }
            index += 1;
        }

        group.sort_by_key(|note| note.note);
        group.truncate(voice_count);
        for (voice_offset, note) in group.into_iter().enumerate() {
            push_note_event(
                events,
                midi,
                note,
                VirtualChannel::Melody,
                voice_base + voice_offset as u8,
            );
        }
    }
}

fn push_drum_events(events: &mut Vec<SequencedEvent>, midi: &MidiFile, notes: &[NoteEvent]) {
    for note in notes {
        if note.channel == MIDI_DRUM_CHANNEL
            && let Some(drum) = map_drum_note(note.note)
        {
            events.push(SequencedEvent {
                frame: tick_to_frame(midi, note.start_tick),
                duration: drum_duration(drum),
                vchannel: VirtualChannel::Drums,
                voice: 255,
                note: drum,
                velocity: note.velocity,
            });
        }
    }
}

fn push_note_event(
    events: &mut Vec<SequencedEvent>,
    midi: &MidiFile,
    note: &NoteEvent,
    vchannel: VirtualChannel,
    voice: u8,
) {
    let start_frame = tick_to_frame(midi, note.start_tick);
    let end_frame = tick_to_frame(midi, note.end_tick);
    let duration = end_frame.saturating_sub(start_frame).max(1);
    events.push(SequencedEvent {
        frame: start_frame,
        duration: duration.min(u32::from(u16::MAX)) as u16,
        vchannel,
        voice,
        note: note.note,
        velocity: note.velocity,
    });
}

fn sequenced_event_sort_key(event: &SequencedEvent) -> (u32, u8, u8, u8) {
    (event.frame, event.vchannel.as_u8(), event.voice, event.note)
}

const fn drum_duration(drum: u8) -> u16 {
    match drum {
        0 => 8,
        1 | 2 => 7,
        _ => 4,
    }
}

fn render_c_source(title: &str, sequenced_events: &[SequencedEvent], song_frames: u32) -> String {
    let safe_title = sanitize_c_comment(title);
    let mut source = String::new();

    source.push_str("/* Generated by smdc music compiler.\n");
    source.push_str(" * Source: ");
    source.push_str(&safe_title);
    source.push('\n');
    source.push_str(" * Driver: 60 Hz YM2612 + PSG virtual-channel sequencer.\n");
    source.push_str(" */\n\n");

    let _ = writeln!(source, "#define EVENT_COUNT {}", sequenced_events.len());
    let _ = writeln!(source, "#define SONG_FRAMES {}", song_frames.max(1));
    let _ = writeln!(source, "#define SONG_FM_CHANNELS {MAX_FM_CHANNELS}\n");
    push_byte_array(
        &mut source,
        "event_frame_lo",
        "EVENT_COUNT",
        sequenced_events
            .iter()
            .map(|event| (event.frame & 0xFF) as u8),
    );
    push_byte_array(
        &mut source,
        "event_frame_hi",
        "EVENT_COUNT",
        sequenced_events
            .iter()
            .map(|event| ((event.frame >> 8) & 0xFF) as u8),
    );
    push_byte_array(
        &mut source,
        "event_duration_lo",
        "EVENT_COUNT",
        sequenced_events
            .iter()
            .map(|event| (event.duration & 0xFF) as u8),
    );
    push_byte_array(
        &mut source,
        "event_duration_hi",
        "EVENT_COUNT",
        sequenced_events
            .iter()
            .map(|event| ((event.duration >> 8) & 0xFF) as u8),
    );
    push_byte_array(
        &mut source,
        "event_vchannel",
        "EVENT_COUNT",
        sequenced_events.iter().map(|event| event.vchannel.as_u8()),
    );
    push_byte_array(
        &mut source,
        "event_voice",
        "EVENT_COUNT",
        sequenced_events.iter().map(|event| event.voice),
    );
    push_byte_array(
        &mut source,
        "event_note",
        "EVENT_COUNT",
        sequenced_events.iter().map(|event| event.note),
    );
    push_byte_array(
        &mut source,
        "event_velocity",
        "EVENT_COUNT",
        sequenced_events.iter().map(|event| event.velocity),
    );
    source.push_str(DRIVER_C);
    source
}

fn push_byte_array<I>(source: &mut String, name: &str, count_macro: &str, values: I)
where
    I: IntoIterator<Item = u8>,
{
    source.push_str("unsigned char ");
    source.push_str(name);
    source.push('[');
    source.push_str(count_macro);
    source.push_str("] = {\n");

    let mut col = 0usize;
    for value in values {
        if col == 0 {
            source.push_str("    ");
        }
        source.push_str(&u32::from(value).to_string());
        source.push_str(", ");
        col += 1;
        if col == 12 {
            source.push('\n');
            col = 0;
        }
    }
    if col != 0 {
        source.push('\n');
    }
    source.push_str("};\n\n");
}

fn sanitize_c_comment(value: &str) -> String {
    value.replace("*/", "* /")
}

const DRIVER_C: &str = r"
#define VDP_DATA   0xC00000
#define VDP_CTRL   0xC00004
#define PSG_PORT   0xC00011
#define YM_ADDR0   0xA04000
#define YM_DATA0   0xA04001
#define YM_ADDR1   0xA04002
#define YM_DATA1   0xA04003

int note_fnum[12] = {644, 682, 723, 766, 811, 859, 910, 964, 1021, 1081, 1146, 1214};
int op_offsets[4] = {0, 8, 4, 12};
int psg_note_div[12] = {428, 404, 381, 360, 340, 321, 303, 286, 270, 255, 241, 227};
int note_colors[12] = {0x00E, 0x04E, 0x0E0, 0x0E8, 0xE00, 0xE08, 0x0EE, 0x08E, 0xE0E, 0xE08, 0xEE0, 0xEEE};
int event_index = 0;
int song_frame = 0;
int fm_vchannel[6] = {255, 255, 255, 255, 255, 255};
int fm_patch[6] = {255, 255, 255, 255, 255, 255};
int fm_end_frame[6] = {0, 0, 0, 0, 0, 0};
int fm_note[6] = {0, 0, 0, 0, 0, 0};
int tone_vchannel[2] = {255, 255};
int tone_priority[2] = {0, 0};
int tone_end_frame[2] = {0, 0};
int tone_note[2] = {0, 0};
int drum_kick = 0;
int drum_snare = 0;
int drum_tom = 0;
int drum_hat = 0;

void write_vdp_ctrl(int value) {
    short *ptr;
    ptr = (short *)VDP_CTRL;
    *ptr = (short)value;
}

void write_vdp_data(int value) {
    short *ptr;
    ptr = (short *)VDP_DATA;
    *ptr = (short)value;
}

int read_vdp_status(void) {
    short *ptr;
    ptr = (short *)VDP_CTRL;
    return *ptr;
}

void vdp_set_register(int reg, int value) {
    write_vdp_ctrl(0x8000 | (reg << 8) | value);
}

void vdp_init(void) {
    vdp_set_register(0, 0x04);
    vdp_set_register(1, 0x44);
    vdp_set_register(7, 0x01);
    vdp_set_register(12, 0x81);
}

void vdp_wait_vblank(void) {
    int status;
    status = read_vdp_status();
    while (status & 0x08) { status = read_vdp_status(); }
    status = read_vdp_status();
    while ((status & 0x08) == 0) { status = read_vdp_status(); }
}

void vdp_set_color(int index, int color) {
    write_vdp_ctrl(0xC000 | ((index & 63) << 1));
    write_vdp_ctrl(0);
    write_vdp_data(color);
}

void psg_write(int value) {
    char *ptr;
    ptr = (char *)PSG_PORT;
    *ptr = (char)value;
}

void ym_delay(void) {
    int i;
    i = 0;
    while (i < 96) { i = i + 1; }
}

void ym_write0(int reg, int val) {
    char *a;
    char *d;
    a = (char *)YM_ADDR0;
    d = (char *)YM_DATA0;
    *a = (char)reg;
    ym_delay();
    *d = (char)val;
    ym_delay();
}

void ym_write1(int reg, int val) {
    char *a;
    char *d;
    a = (char *)YM_ADDR1;
    d = (char *)YM_DATA1;
    *a = (char)reg;
    ym_delay();
    *d = (char)val;
    ym_delay();
}

void ym_write_ch(int ch, int reg, int val) {
    if (ch < 3) {
        ym_write0(reg + ch, val);
    } else {
        ym_write1(reg + ch - 3, val);
    }
}

void ym_write_op(int ch, int op, int reg, int val) {
    int offset;
    offset = op_offsets[op];
    if (ch < 3) {
        ym_write0(reg + offset + ch, val);
    } else {
        ym_write1(reg + offset + ch - 3, val);
    }
}

void ym_key_on(int ch) {
    int slot;
    if (ch < 3) {
        slot = ch;
    } else {
        slot = (ch - 3) | 4;
    }
    ym_write0(0x28, 0xF0 | slot);
}

void ym_key_off(int ch) {
    int slot;
    if (ch < 3) {
        slot = ch;
    } else {
        slot = (ch - 3) | 4;
    }
    ym_write0(0x28, slot);
}

void ym_set_freq(int ch, int note) {
    int block;
    int note_index;
    int hi;
    block = (note / 12) - 1;
    if (block < 1) { block = 1; }
    if (block > 7) { block = 7; }
    note_index = note % 12;
    hi = ((block & 7) << 3) | ((note_fnum[note_index] >> 8) & 7);
    ym_write_ch(ch, 0xA4, hi);
    ym_write_ch(ch, 0xA0, note_fnum[note_index] & 255);
}

void ym_set_pan(int ch, int pan) {
    ym_write_ch(ch, 0xB4, pan);
}

void ym_set_algo(int ch, int algo, int feedback) {
    ym_write_ch(ch, 0xB0, ((feedback & 7) << 3) | (algo & 7));
}

void ym_write_operator_adsr(int ch, int op, int mul, int tl, int ar, int dr, int sr, int rr) {
    ym_write_op(ch, op, 0x30, mul);
    ym_write_op(ch, op, 0x40, tl);
    ym_write_op(ch, op, 0x50, ar);
    ym_write_op(ch, op, 0x60, dr);
    ym_write_op(ch, op, 0x70, sr);
    ym_write_op(ch, op, 0x80, rr);
    ym_write_op(ch, op, 0x90, 0);
}

void ym_patch_guitar(int ch, int pan) {
    ym_set_algo(ch, 5, 7);
    ym_set_pan(ch, pan);
    ym_write_operator_adsr(ch, 0, 0x71, 0x1A, 0x1F, 0x0D, 0x02, 0x2A);
    ym_write_operator_adsr(ch, 1, 0x01, 0x12, 0x1F, 0x0A, 0x02, 0x2A);
    ym_write_operator_adsr(ch, 2, 0x32, 0x1C, 0x1F, 0x0A, 0x02, 0x2A);
    ym_write_operator_adsr(ch, 3, 0x01, 0x14, 0x1F, 0x0A, 0x02, 0x2A);
}

void ym_patch_bass(int ch, int pan) {
    ym_set_algo(ch, 0, 5);
    ym_set_pan(ch, pan);
    ym_write_operator_adsr(ch, 0, 0x01, 0x20, 0x1F, 0x08, 0x04, 0x1A);
    ym_write_operator_adsr(ch, 1, 0x02, 0x28, 0x1F, 0x0A, 0x05, 0x18);
    ym_write_operator_adsr(ch, 2, 0x01, 0x7F, 0x1F, 0x00, 0x00, 0x0F);
    ym_write_operator_adsr(ch, 3, 0x01, 0x08, 0x1F, 0x06, 0x03, 0x1C);
}

void ym_patch_lead(int ch, int pan) {
    ym_set_algo(ch, 4, 4);
    ym_set_pan(ch, pan);
    ym_write_operator_adsr(ch, 0, 0x31, 0x24, 0x1F, 0x08, 0x03, 0x27);
    ym_write_operator_adsr(ch, 1, 0x01, 0x10, 0x1F, 0x08, 0x03, 0x27);
    ym_write_operator_adsr(ch, 2, 0x02, 0x20, 0x1F, 0x08, 0x03, 0x27);
    ym_write_operator_adsr(ch, 3, 0x01, 0x12, 0x1F, 0x08, 0x03, 0x27);
}

int fm_algo_for_vchannel(int vchannel) {
    if (vchannel == 1) {
        return 0;
    }
    return 5;
}

int fm_base_tl_for_vchannel(int vchannel) {
    if (vchannel == 1) {
        return 8;
    }
    return 18;
}

void ym_set_carrier_volume(int ch, int algo, int base_tl, int velocity) {
    int attenuation;
    int tl;
    attenuation = (127 - velocity) >> 2;
    tl = base_tl + attenuation;
    if (tl > 127) { tl = 127; }
    if (tl < 0) { tl = 0; }
    if (algo == 7) { ym_write_op(ch, 0, 0x40, tl); }
    if (algo >= 4) { ym_write_op(ch, 1, 0x40, tl); }
    if (algo >= 5) { ym_write_op(ch, 2, 0x40, tl); }
    ym_write_op(ch, 3, 0x40, tl);
}

int pan_for_channel(int ch) {
    if (ch == 1) { return 0x80; }
    if (ch == 2) { return 0x40; }
    if (ch == 4) { return 0x80; }
    if (ch == 5) { return 0x40; }
    return 0xC0;
}

void fm_apply_patch(int ch, int vchannel) {
    if (fm_patch[ch] == vchannel) {
        return;
    }
    ym_key_off(ch);
    if (vchannel == 1) {
        ym_patch_bass(ch, pan_for_channel(ch));
    } else if (ch == 2 || ch == 5) {
        ym_patch_lead(ch, pan_for_channel(ch));
    } else {
        ym_patch_guitar(ch, pan_for_channel(ch));
    }
    fm_patch[ch] = vchannel;
}

void ym_init(void) {
    int ch;
    ym_write0(0x22, 0x00);
    ym_write0(0x27, 0x00);
    ym_write0(0x2B, 0x00);
    ch = 0;
    while (ch < 6) {
        ym_key_off(ch);
        fm_vchannel[ch] = 255;
        fm_patch[ch] = 255;
        fm_end_frame[ch] = 0;
        fm_note[ch] = 0;
        ch = ch + 1;
    }
}

void psg_set_volume(int ch, int vol) {
    psg_write(0x90 | (ch << 5) | (vol & 15));
}

void psg_set_tone(int ch, int divider) {
    psg_write(0x80 | ((ch & 3) << 5) | (divider & 15));
    psg_write((divider >> 4) & 63);
}

void psg_set_noise(int mode) {
    psg_write(0xE0 | (mode & 7));
}

void psg_init(void) {
    psg_set_volume(0, 15);
    psg_set_volume(1, 15);
    psg_set_volume(2, 15);
    psg_set_volume(3, 15);
    psg_set_noise(4);
}

int note_to_divider(int note) {
    int octave;
    int divider;
    int note_index;
    while (note < 36) { note = note + 12; }
    while (note > 84) { note = note - 12; }
    note_index = note % 12;
    divider = psg_note_div[note_index];
    octave = (note / 12) - 1;
    while (octave > 4) {
        divider = divider >> 1;
        octave = octave - 1;
    }
    while (octave < 4) {
        divider = divider << 1;
        octave = octave + 1;
    }
    if (divider < 1) { divider = 1; }
    if (divider > 1023) { divider = 1023; }
    return divider;
}

int vchannel_priority(int vchannel) {
    if (vchannel == 1) {
        return 70;
    }
    return 60;
}

int vchannel_color(int vchannel) {
    if (vchannel == 1) {
        return 2;
    }
    return 1;
}

void psg_note_on(int ch, int vchannel, int note, int velocity, int color_index) {
    int volume;
    volume = 14 - (velocity >> 4);
    if (vchannel == 0) { volume = volume + 2; }
    if (vchannel == 1) { volume = volume - 4; }
    if (volume < 1) { volume = 1; }
    if (volume > 13) { volume = 13; }
    psg_set_tone(ch, note_to_divider(note));
    psg_set_volume(ch, volume);
    vdp_set_color(color_index, note_colors[note % 12]);
}

void psg_note_off(int ch, int color_index) {
    psg_set_volume(ch, 15);
    vdp_set_color(color_index, 0x222);
}

void tone_release(int ch) {
    if (tone_vchannel[ch] != 255) {
        psg_note_off(ch, vchannel_color(tone_vchannel[ch]));
    } else {
        psg_set_volume(ch, 15);
    }
    tone_vchannel[ch] = 255;
    tone_priority[ch] = 0;
    tone_end_frame[ch] = 0;
    tone_note[ch] = 0;
}

void tone_release_expired(void) {
    if ((tone_vchannel[0] != 255) && (tone_end_frame[0] <= song_frame)) {
        tone_release(0);
    }
    if ((tone_vchannel[1] != 255) && (tone_end_frame[1] <= song_frame)) {
        tone_release(1);
    }
}

int tone_allocate(int vchannel, int priority) {
    int lowest;
    if (tone_vchannel[0] == vchannel) {
        return 0;
    }
    if (tone_vchannel[1] == vchannel) {
        return 1;
    }
    if (tone_vchannel[0] == 255) {
        return 0;
    }
    if (tone_vchannel[1] == 255) {
        return 1;
    }
    lowest = 0;
    if (tone_priority[1] < tone_priority[0]) {
        lowest = 1;
    }
    if (priority >= tone_priority[lowest]) {
        tone_release(lowest);
        return lowest;
    }
    return -1;
}

void tone_note_on(int vchannel, int note, int velocity, int duration) {
    int ch;
    int priority;
    if (velocity < 104) {
        return;
    }
    priority = vchannel_priority(vchannel);
    ch = tone_allocate(vchannel, priority);
    if (ch < 0) {
        return;
    }
    tone_vchannel[ch] = vchannel;
    tone_priority[ch] = priority;
    tone_end_frame[ch] = song_frame + duration;
    tone_note[ch] = note;
    psg_note_on(ch, vchannel, note, velocity, vchannel_color(vchannel));
}

void psg_drum_trigger(int drum, int velocity, int duration) {
    if (drum == 0) {
        drum_kick = duration;
        vdp_set_color(3, 0xE00);
    } else if (drum == 1) {
        drum_snare = duration;
        vdp_set_color(4, 0xEEE);
    } else if (drum == 2) {
        drum_tom = duration;
        vdp_set_color(3, 0xE80);
    } else {
        drum_hat = duration;
        vdp_set_color(4, 0x0EE);
    }
}

void psg_drum_update(void) {
    if (drum_kick > 0) {
        psg_set_tone(2, 48 + ((8 - drum_kick) * 14));
        psg_set_volume(2, 3 + (8 - drum_kick));
        drum_kick = drum_kick - 1;
    } else if (drum_tom > 0) {
        psg_set_tone(2, 96 + ((7 - drum_tom) * 18));
        psg_set_volume(2, 5 + (7 - drum_tom));
        drum_tom = drum_tom - 1;
    } else {
        psg_set_volume(2, 15);
        vdp_set_color(3, 0x222);
    }

    if (drum_snare > 0) {
        psg_set_noise(4);
        psg_set_volume(3, 4 + (7 - drum_snare));
        drum_snare = drum_snare - 1;
    } else if (drum_hat > 0) {
        psg_set_noise(7);
        psg_set_volume(3, 6 + (4 - drum_hat));
        drum_hat = drum_hat - 1;
    } else {
        psg_set_volume(3, 15);
        vdp_set_color(4, 0x222);
    }
}

void fm_release(int ch) {
    ym_key_off(ch);
    fm_vchannel[ch] = 255;
    fm_end_frame[ch] = 0;
    fm_note[ch] = 0;
    vdp_set_color(8 + ch, 0x111);
}

void fm_release_expired(void) {
    int ch;
    ch = 0;
    while (ch < 6) {
        if ((fm_vchannel[ch] != 255) && (fm_end_frame[ch] <= song_frame)) {
            fm_release(ch);
        }
        ch = ch + 1;
    }
}

void fm_note_on(int ch, int vchannel, int note, int velocity, int duration) {
    int algo;
    int base_tl;
    if (ch < 0 || ch >= 6) {
        return;
    }
    fm_apply_patch(ch, vchannel);
    algo = fm_algo_for_vchannel(vchannel);
    base_tl = fm_base_tl_for_vchannel(vchannel);
    ym_key_off(ch);
    ym_set_carrier_volume(ch, algo, base_tl, velocity);
    ym_set_freq(ch, note);
    fm_vchannel[ch] = vchannel;
    fm_end_frame[ch] = song_frame + duration;
    fm_note[ch] = note;
    vdp_set_color(8 + ch, note_colors[note % 12]);
    ym_key_on(ch);
    tone_note_on(vchannel, note, velocity, duration);
}

void music_silence(void) {
    int ch;
    ch = 0;
    while (ch < 6) {
        fm_release(ch);
        ch = ch + 1;
    }
    tone_release(0);
    tone_release(1);
    psg_note_off(2, 3);
    psg_note_off(3, 4);
}

void music_init(void) {
    song_frame = 0;
    event_index = 0;
    drum_kick = 0;
    drum_snare = 0;
    drum_tom = 0;
    drum_hat = 0;
    music_silence();
}

void music_update(void) {
    int event_frame;
    int duration;
    int vchannel;
    int voice;
    fm_release_expired();
    tone_release_expired();
    while (event_index < EVENT_COUNT) {
        event_frame = event_frame_lo[event_index] | (event_frame_hi[event_index] << 8);
        if (event_frame > song_frame) {
            break;
        }
        duration = event_duration_lo[event_index] | (event_duration_hi[event_index] << 8);
        vchannel = event_vchannel[event_index];
        if (vchannel == 2) {
            psg_drum_trigger(event_note[event_index], event_velocity[event_index], duration);
        } else {
            voice = event_voice[event_index];
            fm_note_on(voice, vchannel, event_note[event_index], event_velocity[event_index], duration);
        }
        event_index = event_index + 1;
    }
    psg_drum_update();
    song_frame = song_frame + 1;
    if (song_frame >= SONG_FRAMES) {
        music_init();
    }
}

int main(void) {
    vdp_init();
    ym_init();
    psg_init();
    music_init();
    while (1) {
        vdp_wait_vblank();
        music_update();
    }
    return 0;
}
";

#[allow(dead_code)]
const FULL_DEBUG_DRIVER_C: &str = r"
#define VDP_DATA   0xC00000
#define VDP_CTRL   0xC00004
#define PSG_PORT   0xC00011
#define YM_ADDR0   0xA04000
#define YM_DATA0   0xA04001
#define YM_ADDR1   0xA04002
#define YM_DATA1   0xA04003

int note_fnum[12] = {644, 682, 723, 766, 811, 859, 910, 964, 1021, 1081, 1146, 1214};
int op_offsets[4] = {0, 8, 4, 12};
int psg_note_div[12] = {428, 404, 381, 360, 340, 321, 303, 286, 270, 255, 241, 227};
int fm_active_note[6] = {255, 255, 255, 255, 255, 255};
int debug_voice_note[6] = {0, 0, 0, 0, 0, 0};
int debug_voice_velocity[6] = {0, 0, 0, 0, 0, 0};
int debug_dirty = 1;
int psg_melody_active = 0;
int psg_melody_note = 255;
int melody_index = 0;
int song_frame = 0;
int song_index = 0;
int drum_kick = 0;
int drum_snare = 0;
int drum_tom = 0;
int drum_hat = 0;

void write_vdp_ctrl(int value) {
    short *ptr;
    ptr = (short *)VDP_CTRL;
    *ptr = (short)value;
}

void write_vdp_data(int value) {
    short *ptr;
    ptr = (short *)VDP_DATA;
    *ptr = (short)value;
}

int read_vdp_status(void) {
    short *ptr;
    ptr = (short *)VDP_CTRL;
    return *ptr;
}

void vdp_set_register(int reg, int value) {
    write_vdp_ctrl(0x8000 | (reg << 8) | value);
}

void vdp_init(void) {
    vdp_set_register(0, 0x04);
    vdp_set_register(1, 0x44);
    vdp_set_register(2, 0x30);
    vdp_set_register(3, 0x3C);
    vdp_set_register(4, 0x07);
    vdp_set_register(5, 0x78);
    vdp_set_register(7, 0x00);
    vdp_set_register(10, 0xFF);
    vdp_set_register(11, 0x00);
    vdp_set_register(12, 0x81);
    vdp_set_register(13, 0x3F);
    vdp_set_register(15, 0x02);
    vdp_set_register(16, 0x01);
}

void vdp_wait_vblank(void) {
    int status;
    status = read_vdp_status();
    while (status & 0x08) { status = read_vdp_status(); }
    status = read_vdp_status();
    while ((status & 0x08) == 0) { status = read_vdp_status(); }
}

void vdp_set_vram_addr(int addr) {
    write_vdp_ctrl(0x4000 | (addr & 0x3FFF));
    write_vdp_ctrl((addr >> 14) & 3);
}

void vdp_set_cram_addr(int index) {
    write_vdp_ctrl(0xC000 | ((index & 63) << 1));
    write_vdp_ctrl(0);
}

void debug_set_color(int index, int color) {
    vdp_set_cram_addr(index);
    write_vdp_data(color);
}

void debug_make_tiles(void) {
    int tile;
    int row;
    int word;
    tile = 1;
    while (tile < 16) {
        vdp_set_vram_addr(tile * 32);
        word = (tile << 12) | (tile << 8) | (tile << 4) | tile;
        row = 0;
        while (row < 16) {
            write_vdp_data(word);
            row = row + 1;
        }
        tile = tile + 1;
    }
}

void debug_clear_plane(void) {
    int i;
    vdp_set_vram_addr(0xC000);
    i = 0;
    while (i < 2048) {
        write_vdp_data(0);
        i = i + 1;
    }
}

void debug_draw_bar(int row, int len, int tile) {
    int col;
    int value;
    col = 0;
    while (col < 40) {
        if (col < len) {
            value = tile;
        } else {
            value = 0;
        }
        vdp_set_vram_addr(0xC000 + (row * 128) + (col * 2));
        write_vdp_data(value);
        col = col + 1;
    }
}

void debug_draw_voice(int row, int note, int velocity, int tile) {
    int col;
    int pos;
    int value;
    pos = note - 24;
    if (pos < 0) { pos = 0; }
    if (pos > 72) { pos = 72; }
    pos = (pos * 39) / 72;
    col = 0;
    while (col < 40) {
        if ((velocity > 0) && (col == pos)) {
            value = tile;
        } else {
            value = 0;
        }
        vdp_set_vram_addr(0xC000 + (row * 128) + (col * 2));
        write_vdp_data(value);
        col = col + 1;
    }
}

void debug_init(void) {
    debug_set_color(0, 0x000);
    debug_set_color(1, 0x00E);
    debug_set_color(2, 0x0E0);
    debug_set_color(3, 0xE00);
    debug_set_color(4, 0x0EE);
    debug_set_color(5, 0xE0E);
    debug_set_color(6, 0xEE0);
    debug_set_color(7, 0xEEE);
    debug_set_color(8, 0x008);
    debug_set_color(9, 0x080);
    debug_set_color(10, 0x800);
    debug_set_color(11, 0x088);
    debug_set_color(12, 0x808);
    debug_set_color(13, 0x880);
    debug_set_color(14, 0xAAA);
    debug_set_color(15, 0x444);
    debug_make_tiles();
    debug_clear_plane();
    vdp_set_register(7, 0);
    debug_dirty = 1;
}

void debug_note_on(int ch, int note, int velocity) {
    if (ch < 6) {
        debug_voice_note[ch] = note;
        debug_voice_velocity[ch] = velocity;
        debug_dirty = 1;
    }
}

void debug_note_off(int ch, int note) {
    if (ch < 6) {
        debug_voice_velocity[ch] = 0;
        debug_dirty = 1;
    }
}

void debug_update(void) {
    int ch;
    int progress;
    if (debug_dirty || ((song_frame & 3) == 0)) {
        ch = 0;
        while (ch < 6) {
            debug_draw_voice(4 + (ch * 2), debug_voice_note[ch], debug_voice_velocity[ch], ch + 1);
            ch = ch + 1;
        }
        progress = (song_frame * 40) / SONG_FRAMES;
        debug_draw_bar(20, progress, 14);
        debug_dirty = 0;
    }
}

void ym_delay(void) {
    int i;
    i = 0;
    while (i < 96) { i = i + 1; }
}

void ym_write0(int reg, int val) {
    char *a;
    char *d;
    a = (char *)YM_ADDR0;
    d = (char *)YM_DATA0;
    *a = (char)reg;
    ym_delay();
    *d = (char)val;
    ym_delay();
}

void ym_write1(int reg, int val) {
    char *a;
    char *d;
    a = (char *)YM_ADDR1;
    d = (char *)YM_DATA1;
    *a = (char)reg;
    ym_delay();
    *d = (char)val;
    ym_delay();
}

void ym_write_ch(int ch, int reg, int val) {
    if (ch < 3) { ym_write0(reg + ch, val); }
    else { ym_write1(reg + ch - 3, val); }
}

void ym_write_op(int ch, int op, int reg, int val) {
    int offset;
    offset = op_offsets[op];
    if (ch < 3) { ym_write0(reg + offset + ch, val); }
    else { ym_write1(reg + offset + ch - 3, val); }
}

void ym_key_on(int ch) {
    int slot;
    if (ch < 3) { slot = ch; } else { slot = (ch - 3) | 4; }
    ym_write0(0x28, 0xF0 | slot);
}

void ym_key_off(int ch) {
    int slot;
    if (ch < 3) { slot = ch; } else { slot = (ch - 3) | 4; }
    ym_write0(0x28, slot);
}

void ym_set_freq(int ch, int block, int fnum) {
    int hi;
    hi = ((block & 7) << 3) | ((fnum >> 8) & 7);
    ym_write_ch(ch, 0xA4, hi);
    ym_write_ch(ch, 0xA0, fnum & 255);
}

void ym_set_pan(int ch, int pan) {
    ym_write_ch(ch, 0xB4, pan);
}

void ym_set_algo(int ch, int algo, int feedback) {
    ym_write_ch(ch, 0xB0, ((feedback & 7) << 3) | (algo & 7));
}

void ym_set_carrier_volume(int ch, int algo, int base_tl, int velocity) {
    int attenuation;
    int tl;
    attenuation = (127 - velocity) >> 2;
    tl = base_tl + attenuation;
    if (tl > 127) { tl = 127; }
    if (algo == 7) { ym_write_op(ch, 0, 0x40, tl); }
    if (algo >= 4) { ym_write_op(ch, 1, 0x40, tl); }
    if (algo >= 5) { ym_write_op(ch, 2, 0x40, tl); }
    ym_write_op(ch, 3, 0x40, tl);
}

void ym_patch_guitar(int ch, int pan) {
    ym_set_algo(ch, 5, 7);
    ym_set_pan(ch, pan);
    ym_write_op(ch, 0, 0x30, 0x71); ym_write_op(ch, 0, 0x40, 0x1A); ym_write_op(ch, 0, 0x50, 0x1F); ym_write_op(ch, 0, 0x60, 0x0D); ym_write_op(ch, 0, 0x70, 0x02); ym_write_op(ch, 0, 0x80, 0x2A);
    ym_write_op(ch, 1, 0x30, 0x01); ym_write_op(ch, 1, 0x40, 0x12); ym_write_op(ch, 1, 0x50, 0x1F); ym_write_op(ch, 1, 0x60, 0x0A); ym_write_op(ch, 1, 0x70, 0x02); ym_write_op(ch, 1, 0x80, 0x2A);
    ym_write_op(ch, 2, 0x30, 0x32); ym_write_op(ch, 2, 0x40, 0x1C); ym_write_op(ch, 2, 0x50, 0x1F); ym_write_op(ch, 2, 0x60, 0x0A); ym_write_op(ch, 2, 0x70, 0x02); ym_write_op(ch, 2, 0x80, 0x2A);
    ym_write_op(ch, 3, 0x30, 0x01); ym_write_op(ch, 3, 0x40, 0x14); ym_write_op(ch, 3, 0x50, 0x1F); ym_write_op(ch, 3, 0x60, 0x0A); ym_write_op(ch, 3, 0x70, 0x02); ym_write_op(ch, 3, 0x80, 0x2A);
}

void ym_patch_bass(int ch, int pan) {
    ym_set_algo(ch, 0, 5);
    ym_set_pan(ch, pan);
    ym_write_op(ch, 0, 0x30, 0x01); ym_write_op(ch, 0, 0x40, 0x20); ym_write_op(ch, 0, 0x50, 0x1F); ym_write_op(ch, 0, 0x60, 0x08); ym_write_op(ch, 0, 0x70, 0x04); ym_write_op(ch, 0, 0x80, 0x1A);
    ym_write_op(ch, 1, 0x30, 0x02); ym_write_op(ch, 1, 0x40, 0x28); ym_write_op(ch, 1, 0x50, 0x1F); ym_write_op(ch, 1, 0x60, 0x0A); ym_write_op(ch, 1, 0x70, 0x05); ym_write_op(ch, 1, 0x80, 0x18);
    ym_write_op(ch, 2, 0x30, 0x01); ym_write_op(ch, 2, 0x40, 0x7F); ym_write_op(ch, 2, 0x50, 0x1F); ym_write_op(ch, 2, 0x60, 0x00); ym_write_op(ch, 2, 0x70, 0x00); ym_write_op(ch, 2, 0x80, 0x0F);
    ym_write_op(ch, 3, 0x30, 0x01); ym_write_op(ch, 3, 0x40, 0x08); ym_write_op(ch, 3, 0x50, 0x1F); ym_write_op(ch, 3, 0x60, 0x06); ym_write_op(ch, 3, 0x70, 0x03); ym_write_op(ch, 3, 0x80, 0x1C);
}

void ym_patch_lead(int ch, int pan) {
    ym_set_algo(ch, 4, 4);
    ym_set_pan(ch, pan);
    ym_write_op(ch, 0, 0x30, 0x31); ym_write_op(ch, 0, 0x40, 0x24); ym_write_op(ch, 0, 0x50, 0x1F); ym_write_op(ch, 0, 0x60, 0x08); ym_write_op(ch, 0, 0x70, 0x03); ym_write_op(ch, 0, 0x80, 0x27);
    ym_write_op(ch, 1, 0x30, 0x01); ym_write_op(ch, 1, 0x40, 0x10); ym_write_op(ch, 1, 0x50, 0x1F); ym_write_op(ch, 1, 0x60, 0x08); ym_write_op(ch, 1, 0x70, 0x03); ym_write_op(ch, 1, 0x80, 0x27);
    ym_write_op(ch, 2, 0x30, 0x02); ym_write_op(ch, 2, 0x40, 0x20); ym_write_op(ch, 2, 0x50, 0x1F); ym_write_op(ch, 2, 0x60, 0x08); ym_write_op(ch, 2, 0x70, 0x03); ym_write_op(ch, 2, 0x80, 0x27);
    ym_write_op(ch, 3, 0x30, 0x01); ym_write_op(ch, 3, 0x40, 0x12); ym_write_op(ch, 3, 0x50, 0x1F); ym_write_op(ch, 3, 0x60, 0x08); ym_write_op(ch, 3, 0x70, 0x03); ym_write_op(ch, 3, 0x80, 0x27);
}

void ym_patch_tone(int ch, int pan) {
    ym_set_algo(ch, 7, 0);
    ym_set_pan(ch, pan);
    ym_write_op(ch, 0, 0x30, 0x01); ym_write_op(ch, 0, 0x40, 0x18); ym_write_op(ch, 0, 0x50, 0x1F); ym_write_op(ch, 0, 0x60, 0x08); ym_write_op(ch, 0, 0x70, 0x02); ym_write_op(ch, 0, 0x80, 0x0F);
    ym_write_op(ch, 1, 0x30, 0x01); ym_write_op(ch, 1, 0x40, 0x28); ym_write_op(ch, 1, 0x50, 0x1F); ym_write_op(ch, 1, 0x60, 0x08); ym_write_op(ch, 1, 0x70, 0x02); ym_write_op(ch, 1, 0x80, 0x0F);
    ym_write_op(ch, 2, 0x30, 0x01); ym_write_op(ch, 2, 0x40, 0x28); ym_write_op(ch, 2, 0x50, 0x1F); ym_write_op(ch, 2, 0x60, 0x08); ym_write_op(ch, 2, 0x70, 0x02); ym_write_op(ch, 2, 0x80, 0x0F);
    ym_write_op(ch, 3, 0x30, 0x01); ym_write_op(ch, 3, 0x40, 0x18); ym_write_op(ch, 3, 0x50, 0x1F); ym_write_op(ch, 3, 0x60, 0x08); ym_write_op(ch, 3, 0x70, 0x02); ym_write_op(ch, 3, 0x80, 0x0F);
}

void ym_init(void) {
    int ch;
    ym_write0(0x22, 0x00);
    ym_write0(0x27, 0x00);
    ym_write0(0x2B, 0x00);
    ch = 0;
    while (ch < 6) {
        ym_key_off(ch);
        ch = ch + 1;
    }
    ym_patch_tone(0, 0xC0);
    ym_patch_tone(1, 0x80);
    ym_patch_tone(2, 0x40);
    ym_patch_tone(3, 0xC0);
    ym_patch_tone(4, 0x80);
    ym_patch_tone(5, 0x40);
}

void psg_write(int value) {
    char *ptr;
    ptr = (char *)PSG_PORT;
    *ptr = (char)value;
}

void psg_set_volume(int ch, int vol) {
    psg_write(0x90 | (ch << 5) | (vol & 15));
}

void psg_set_tone(int ch, int divider) {
    psg_write(0x80 | ((ch & 3) << 5) | (divider & 15));
    psg_write((divider >> 4) & 63);
}

void psg_set_noise(int mode) {
    psg_write(0xE0 | (mode & 7));
}

void psg_init(void) {
    psg_set_volume(0, 15);
    psg_set_volume(1, 15);
    psg_set_volume(2, 15);
    psg_set_volume(3, 15);
    psg_set_noise(4);
}

void psg_melody_on(int note, int velocity) {
    int octave;
    int divider;
    int note_index;
    int volume;
    note_index = note % 12;
    divider = psg_note_div[note_index];
    octave = (note / 12) - 1;
    while (octave > 4) {
        divider = divider >> 1;
        octave = octave - 1;
    }
    while (octave < 4) {
        divider = divider << 1;
        octave = octave + 1;
    }
    if (divider < 1) { divider = 1; }
    if (divider > 1023) { divider = 1023; }
    volume = 14 - (velocity >> 4);
    if (volume < 2) { volume = 2; }
    if (volume > 12) { volume = 12; }
    psg_set_tone(0, divider);
    psg_set_volume(0, volume);
    psg_melody_active = 1;
    psg_melody_note = note;
}

void psg_melody_off(int note) {
    if (psg_melody_note == note) {
        psg_melody_active = 0;
        psg_melody_note = 255;
        psg_set_volume(0, 15);
    }
}

void psg_melody_update(void) {
    int event_frame;
    while (melody_index < MELODY_EVENT_COUNT) {
        event_frame = melody_event_frame_lo[melody_index] | (melody_event_frame_hi[melody_index] << 8);
        if (event_frame > song_frame) {
            break;
        }
        if (melody_event_velocity[melody_index] > 0) {
            psg_melody_on(melody_event_note[melody_index], melody_event_velocity[melody_index]);
        } else {
            psg_melody_off(psg_melody_note);
        }
        melody_index = melody_index + 1;
    }
}

void fm_note_on(int ch, int note, int velocity) {
    int block;
    int note_index;
    int algo;
    int base_tl;
    block = (note / 12) - 1;
    if (block < 1) { block = 1; }
    if (block > 7) { block = 7; }
    note_index = note % 12;
    algo = 7;
    base_tl = 18;
    ym_key_off(ch);
    ym_set_carrier_volume(ch, algo, base_tl, velocity);
    ym_set_freq(ch, block, note_fnum[note_index]);
    fm_active_note[ch] = note;
    debug_note_on(ch, note, velocity);
    ym_key_on(ch);
}

void fm_note_off(int ch, int note) {
    if (fm_active_note[ch] == note) {
        ym_key_off(ch);
        fm_active_note[ch] = 255;
        debug_note_off(ch, note);
    }
}

void drum_trigger(int drum, int velocity) {
}

void drums_update(void) {
    if (drum_kick > 0) {
        psg_set_tone(0, 24 + ((8 - drum_kick) * 7));
        psg_set_volume(0, 10 - drum_kick);
        drum_kick = drum_kick - 1;
    } else if (psg_melody_active == 0) {
        psg_set_volume(0, 15);
    }

    if (drum_snare > 0) {
        psg_set_volume(1, 12 - drum_snare);
        psg_set_volume(3, 13 - drum_snare);
        drum_snare = drum_snare - 1;
    } else if (drum_hat == 0) {
        psg_set_volume(1, 15);
        psg_set_volume(3, 15);
    }

    if (drum_tom > 0) {
        psg_set_tone(2, 80 + ((9 - drum_tom) * 12));
        psg_set_volume(2, 12 - drum_tom);
        drum_tom = drum_tom - 1;
    } else if (psg_melody_active == 0) {
        psg_set_volume(2, 15);
    }

    if (drum_hat > 0) {
        psg_set_noise(4);
        psg_set_volume(3, 10 - drum_hat);
        drum_hat = drum_hat - 1;
    } else if (drum_snare == 0) {
        psg_set_volume(3, 15);
    }
}

void music_dispatch(int kind, int ch, int note, int velocity) {
    if (kind == 0) {
        fm_note_on(ch, note, velocity);
    } else if (kind == 1) {
        fm_note_off(ch, note);
    } else {
        drum_trigger(ch, velocity);
    }
}

void music_silence(void) {
    int ch;
    ch = 0;
    while (ch < 6) {
        ym_key_off(ch);
        fm_active_note[ch] = 255;
        ch = ch + 1;
    }
    psg_init();
}

void music_init(void) {
    song_frame = 0;
    song_index = 0;
    melody_index = 0;
    psg_melody_active = 0;
    psg_melody_note = 255;
    drum_kick = 0;
    drum_snare = 0;
    drum_tom = 0;
    drum_hat = 0;
    music_silence();
}

void music_update(void) {
    int event_frame;
    while (song_index < SONG_EVENT_COUNT) {
        event_frame = song_event_frame_lo[song_index] | (song_event_frame_hi[song_index] << 8);
        if (event_frame > song_frame) {
            break;
        }
        music_dispatch(
            song_event_kind[song_index],
            song_event_channel[song_index],
            song_event_note[song_index],
            song_event_velocity[song_index]
        );
        song_index = song_index + 1;
    }

    drums_update();
    psg_melody_update();
    debug_update();
    song_frame = song_frame + 1;

    if (song_frame >= SONG_FRAMES) {
        music_init();
    }
}

int main(void) {
    vdp_init();
    debug_init();
    ym_init();
    psg_init();
    music_init();

    while (1) {
        vdp_wait_vblank();
        music_update();
    }

    return 0;
}
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_to_micros_uses_tempo_changes() {
        let tempos = vec![
            TempoEvent {
                tick: 0,
                micros_per_quarter: 500_000,
            },
            TempoEvent {
                tick: 480,
                micros_per_quarter: 1_000_000,
            },
        ];

        assert_eq!(tick_to_micros(960, 480, &tempos), 1_500_000);
    }

    #[test]
    fn test_drum_mapping() {
        assert_eq!(map_drum_note(36), Some(0));
        assert_eq!(map_drum_note(38), Some(1));
        assert_eq!(map_drum_note(45), Some(2));
        assert_eq!(map_drum_note(42), Some(3));
        assert_eq!(map_drum_note(60), None);
    }

    #[test]
    fn test_drum_durations_match_reserved_psg_channels() {
        assert_eq!(drum_duration(0), 8);
        assert_eq!(drum_duration(1), 7);
        assert_eq!(drum_duration(2), 7);
        assert_eq!(drum_duration(3), 4);
    }

    #[test]
    fn test_arrangement_selects_bass_rhythm_and_lead_tracks() {
        let notes = vec![
            test_note(0, 12, 2, 1, 40, 100),
            test_note(0, 6, 3, 2, 52, 110),
            test_note(0, 6, 3, 2, 59, 110),
            test_note(4, 10, 4, 3, 69, 120),
        ];

        let tracks = choose_arrangement_tracks(&notes);

        assert_eq!(tracks.bass, Some(2));
        assert_eq!(tracks.rhythm, Some(3));
        assert_eq!(tracks.lead, Some(4));
    }

    #[test]
    fn test_arranged_events_use_fixed_role_voices() {
        let midi = test_midi();
        let notes = vec![
            test_note(0, 12, 2, 1, 40, 100),
            test_note(0, 6, 3, 2, 52, 110),
            test_note(0, 6, 3, 2, 59, 110),
            test_note(2, 4, 6, 9, 38, 127),
            test_note(4, 10, 4, 3, 69, 120),
        ];

        let mut events = build_arranged_note_events(&midi, &notes, 30);
        events.sort_by_key(sequenced_event_sort_key);

        assert!(events.iter().any(|event| {
            event.vchannel == VirtualChannel::Bass
                && event.voice == 0
                && event.note == 40
                && event.duration == 12
        }));
        assert!(events.iter().any(|event| {
            event.vchannel == VirtualChannel::Melody && event.voice == 1 && event.note == 52
        }));
        assert!(events.iter().any(|event| {
            event.vchannel == VirtualChannel::Melody && event.voice == 2 && event.note == 59
        }));
        assert!(events.iter().any(|event| {
            event.vchannel == VirtualChannel::Melody && event.voice == 3 && event.note == 69
        }));
        assert!(events.iter().any(|event| {
            event.vchannel == VirtualChannel::Drums && event.voice == 255 && event.note == 1
        }));
    }

    #[test]
    fn test_manual_cover_uses_driver_event_stream_without_midi_tracks() {
        let generated = generate_manual_paranoid_cover("manual");

        assert_eq!(generated.stats.midi_tracks, 0);
        assert!(generated.stats.rendered_events > 300);
        assert!(generated.stats.drum_events > 100);
        assert!(generated.c_source.contains("event_voice"));
        assert!(generated.c_source.contains("void fm_note_on"));
        assert!(!generated.c_source.contains("SONG_EVENT_COUNT"));
    }

    fn test_midi() -> MidiFile {
        MidiFile {
            division: 30,
            tracks: Vec::new(),
            tempos: vec![TempoEvent {
                tick: 0,
                micros_per_quarter: DEFAULT_TEMPO_US_PER_QUARTER,
            }],
        }
    }

    fn test_note(
        start_tick: u64,
        end_tick: u64,
        track_index: usize,
        channel: u8,
        note: u8,
        velocity: u8,
    ) -> NoteEvent {
        NoteEvent {
            start_tick,
            end_tick,
            track_index,
            channel,
            note,
            velocity,
        }
    }
}
