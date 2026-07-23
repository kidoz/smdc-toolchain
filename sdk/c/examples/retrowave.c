/*
 * retrowave.c - Retrowave loop using the SMD SDK sound driver
 *
 * 8-bar loop (~17s at 60Hz) with:
 * - FM1: Bass
 * - FM2/FM3: Pad chords
 * - FM4: Lead melody
 * - FM5: Arpeggio
 * - FM6: Kick/Snare
 * - PSG Noise: Hi-hat
 *
 * Compile:
 *   smdc sdk/c/examples/retrowave.c -o retrowave.bin -t rom -I sdk/c/include
 */

#include <smd/vdp.h>

#define SND_IMPLEMENTATION
#include <smd/snd.h>
#include <smd/snd_impl.h>
#include <smd/snd_patches.h>

#define PATTERN_ROWS 64
#define ROWS_PER_BAR 16

#define NOTE(oct, note) SND_NOTE((oct), (note))

#define INST_NONE 255
#define VOL_NONE 255

/* Instrument indices (using defines instead of enum for compiler compatibility) */
#define INST_BASS   0
#define INST_PAD    1
#define INST_LEAD   2
#define INST_ARP    3
#define INST_KICK   4
#define INST_SNARE  5
#define INST_COUNT  6

struct Chord {
    int root;
    unsigned char minor;
};

struct LeadEvent {
    unsigned char row;
    unsigned char note;
};

static const struct Chord chords_a[4] = {
    { NOTE(2, SND_NOTE_A), 1 }, /* Am */
    { NOTE(2, SND_NOTE_F), 0 }, /* F  */
    { NOTE(2, SND_NOTE_C), 0 }, /* C  */
    { NOTE(2, SND_NOTE_G), 0 }  /* G  */
};

static const struct Chord chords_b[4] = {
    { NOTE(2, SND_NOTE_A), 1 }, /* Am */
    { NOTE(2, SND_NOTE_C), 0 }, /* C  */
    { NOTE(2, SND_NOTE_F), 0 }, /* F  */
    { NOTE(2, SND_NOTE_G), 0 }  /* G  */
};

static const struct LeadEvent lead_a[] = {
    { 0,  NOTE(4, SND_NOTE_A) },
    { 8,  NOTE(5, SND_NOTE_C) },
    { 12, NOTE(5, SND_NOTE_E) },
    { 15, SND_NOTE_OFF },
    { 16, NOTE(4, SND_NOTE_A) },
    { 24, NOTE(4, SND_NOTE_G) },
    { 28, NOTE(4, SND_NOTE_F) },
    { 31, SND_NOTE_OFF },
    { 32, NOTE(4, SND_NOTE_E) },
    { 40, NOTE(4, SND_NOTE_G) },
    { 44, NOTE(5, SND_NOTE_C) },
    { 47, SND_NOTE_OFF },
    { 48, NOTE(4, SND_NOTE_D) },
    { 56, NOTE(4, SND_NOTE_G) },
    { 60, NOTE(4, SND_NOTE_B) },
    { 63, SND_NOTE_OFF }
};

static const struct LeadEvent lead_b[] = {
    { 0,  NOTE(5, SND_NOTE_C) },
    { 8,  NOTE(5, SND_NOTE_E) },
    { 12, NOTE(4, SND_NOTE_A) },
    { 15, SND_NOTE_OFF },
    { 16, NOTE(4, SND_NOTE_G) },
    { 24, NOTE(4, SND_NOTE_E) },
    { 28, NOTE(4, SND_NOTE_C) },
    { 31, SND_NOTE_OFF },
    { 32, NOTE(4, SND_NOTE_A) },
    { 40, NOTE(5, SND_NOTE_C) },
    { 44, NOTE(4, SND_NOTE_A) },
    { 47, SND_NOTE_OFF },
    { 48, NOTE(4, SND_NOTE_B) },
    { 56, NOTE(5, SND_NOTE_D) },
    { 60, NOTE(4, SND_NOTE_G) },
    { 63, SND_NOTE_OFF }
};

static struct SndRow pattern_a_rows[PATTERN_ROWS];
static struct SndRow pattern_b_rows[PATTERN_ROWS];
static struct SndPattern patterns[2];
static unsigned char sequence[2] = { 0, 1 };

static struct SndInstrument instruments[INST_COUNT];
static struct SndSong song;

static const struct SndNote empty_note = {
    SND_NOTE_NONE, INST_NONE, VOL_NONE, SND_FX_NONE, 0
};

static void clear_pattern(struct SndRow *rows) {
    int r;
    int ch;
    struct SndRow *row_ptr;
    struct SndNote *note_ptr;
    for (r = 0; r < PATTERN_ROWS; r++) {
        row_ptr = &rows[r];
        for (ch = 0; ch < SND_TOTAL_CHANNELS; ch++) {
            note_ptr = &row_ptr->ch[ch];
            note_ptr->note = empty_note.note;
            note_ptr->inst = empty_note.inst;
            note_ptr->vol = empty_note.vol;
            note_ptr->fx = empty_note.fx;
            note_ptr->fx_param = empty_note.fx_param;
        }
    }
}

static void set_note(struct SndRow *rows, int row, int ch, int note, int inst,
                     int vol, int fx, int fx_param) {
    struct SndRow *row_ptr;
    struct SndNote *note_ptr;
    row_ptr = &rows[row];
    note_ptr = &row_ptr->ch[ch];
    note_ptr->note = note;
    note_ptr->inst = inst;
    note_ptr->vol = vol;
    note_ptr->fx = fx;
    note_ptr->fx_param = fx_param;
}

static void apply_lead(struct SndRow *rows,
                       const struct LeadEvent *events, int count) {
    int i;
    for (i = 0; i < count; i++) {
        int inst = (events[i].row == 0) ? INST_LEAD : INST_NONE;
        set_note(rows, events[i].row, SND_CH_FM4, events[i].note, inst, 112,
                 SND_FX_NONE, 0);
    }
}

static void build_pattern(struct SndRow *rows, const struct Chord *chords) {
    int bar;
    for (bar = 0; bar < 4; bar++) {
        int base = bar * ROWS_PER_BAR;
        int root = chords[bar].root;
        int third = root + (chords[bar].minor ? 3 : 4);
        int fifth = root + 7;
        int step;

        for (step = 0; step < 8; step++) {
            int row = base + (step * 2);
            int inst = (bar == 0 && step == 0) ? INST_BASS : INST_NONE;
            set_note(rows, row, SND_CH_FM1, root, inst, 110, SND_FX_NONE, 0);
        }

        set_note(rows, base, SND_CH_FM2, root + 12,
                 (bar == 0) ? INST_PAD : INST_NONE, 92, SND_FX_NONE, 0);
        set_note(rows, base, SND_CH_FM3, third + 12,
                 (bar == 0) ? INST_PAD : INST_NONE, 92, SND_FX_NONE, 0);
        set_note(rows, base + 15, SND_CH_FM2, SND_NOTE_OFF,
                 INST_NONE, VOL_NONE, SND_FX_NONE, 0);
        set_note(rows, base + 15, SND_CH_FM3, SND_NOTE_OFF,
                 INST_NONE, VOL_NONE, SND_FX_NONE, 0);

        for (step = 0; step < ROWS_PER_BAR; step++) {
            int row = base + step;
            int inst = (bar == 0 && step == 0) ? INST_ARP : INST_NONE;
            int arp_note;
            switch (step & 3) {
                case 0: arp_note = root + 24; break;
                case 1: arp_note = fifth + 24; break;
                case 2: arp_note = third + 24; break;
                default: arp_note = root + 36; break;
            }
            set_note(rows, row, SND_CH_FM5, arp_note, inst, 84, SND_FX_NONE, 0);
        }

        set_note(rows, base + 0, SND_CH_FM6, NOTE(2, SND_NOTE_C),
                 INST_KICK, 120, SND_FX_NONE, 0);
        set_note(rows, base + 8, SND_CH_FM6, NOTE(2, SND_NOTE_C),
                 INST_KICK, 120, SND_FX_NONE, 0);
        set_note(rows, base + 4, SND_CH_FM6, NOTE(3, SND_NOTE_D),
                 INST_SNARE, 112, SND_FX_NONE, 0);
        set_note(rows, base + 12, SND_CH_FM6, NOTE(3, SND_NOTE_D),
                 INST_SNARE, 112, SND_FX_NONE, 0);

        for (step = 0; step < ROWS_PER_BAR; step++) {
            int row = base + step;
            if ((step & 1) == 0) {
                set_note(rows, row, SND_CH_NOISE, 4,
                         INST_NONE, 96, SND_FX_NONE, 0);
            } else {
                set_note(rows, row, SND_CH_NOISE, SND_NOTE_NONE,
                         INST_NONE, 0, SND_FX_NONE, 0);
            }
        }
    }
}

static void init_song_data(void) {
    struct SndInstrument *inst_ptr;

    /* Initialize instruments at runtime (compiler doesn't support complex initializers) */
    /* INST_BASS = 0 */
    inst_ptr = &instruments[0];
    inst_ptr->type = 0;
    inst_ptr->fm = (struct SndFmPatch *)&SND_PATCH_BASS_SYNTH;
    /* INST_PAD = 1 */
    inst_ptr = &instruments[1];
    inst_ptr->type = 0;
    inst_ptr->fm = (struct SndFmPatch *)&SND_PATCH_PAD_STRINGS;
    /* INST_LEAD = 2 */
    inst_ptr = &instruments[2];
    inst_ptr->type = 0;
    inst_ptr->fm = (struct SndFmPatch *)&SND_PATCH_LEAD_BRIGHT;
    /* INST_ARP = 3 */
    inst_ptr = &instruments[3];
    inst_ptr->type = 0;
    inst_ptr->fm = (struct SndFmPatch *)&SND_PATCH_PLUCK;
    /* INST_KICK = 4 */
    inst_ptr = &instruments[4];
    inst_ptr->type = 0;
    inst_ptr->fm = (struct SndFmPatch *)&SND_PATCH_DRUM_KICK;
    /* INST_SNARE = 5 */
    inst_ptr = &instruments[5];
    inst_ptr->type = 0;
    inst_ptr->fm = (struct SndFmPatch *)&SND_PATCH_DRUM_SNARE;

    /* Initialize song struct at runtime */
    song.num_patterns = 2;
    song.sequence_length = 2;
    song.loop_point = 0;
    song.initial_speed = 8;
    song.initial_tempo = 120;
    song.num_instruments = 6;  /* INST_COUNT */
    song.sequence = sequence;
    song.patterns = patterns;
    song.instruments = instruments;

    {
        struct SndPattern *pat_ptr;
        pat_ptr = &patterns[0];
        pat_ptr->num_rows = PATTERN_ROWS;
        pat_ptr->rows = pattern_a_rows;
        pat_ptr = &patterns[1];
        pat_ptr->num_rows = PATTERN_ROWS;
        pat_ptr->rows = pattern_b_rows;
    }

    clear_pattern(pattern_a_rows);
    clear_pattern(pattern_b_rows);

    build_pattern(pattern_a_rows, chords_a);
    build_pattern(pattern_b_rows, chords_b);

    apply_lead(pattern_a_rows, lead_a, sizeof(lead_a) / sizeof(lead_a[0]));
    apply_lead(pattern_b_rows, lead_b, sizeof(lead_b) / sizeof(lead_b[0]));
}

void main(void) {
    vdp_init();
    vdp_set_color(0, 0x0000);

    snd_init();
    init_song_data();
    snd_play_song(&song);

    while (1) {
        vdp_vsync();
        snd_update();
    }
}
