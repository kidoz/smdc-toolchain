/*
 * snd_test.c - SMD Sound Driver Test (Standalone)
 *
 * Tests FM sound with embedded driver code (no complex structs).
 *
 * Compile:
 *   smdc sdk/c/examples/snd_test.c -o snd_test.bin -t rom -I sdk/c/include
 */

#include <smd/vdp.h>

/* ============================================================================
 * YM2612 Hardware Access
 * ============================================================================ */

#define YM_ADDR0    (*(volatile unsigned char *)0xA04000)
#define YM_DATA0    (*(volatile unsigned char *)0xA04001)
#define YM_ADDR1    (*(volatile unsigned char *)0xA04002)
#define YM_DATA1    (*(volatile unsigned char *)0xA04003)

static void ym_wait(void) {
    while (YM_ADDR0 & 0x80);
}

static void ym_write0(int reg, int val) {
    ym_wait();
    YM_ADDR0 = reg;
    ym_wait();
    YM_DATA0 = val;
}

static void ym_write1(int reg, int val) {
    ym_wait();
    YM_ADDR1 = reg;
    ym_wait();
    YM_DATA1 = val;
}

static void ym_write_ch(int ch, int reg, int val) {
    if (ch < 3) {
        ym_write0(reg + ch, val);
    } else {
        ym_write1(reg + (ch - 3), val);
    }
}

static void ym_key_on(int ch) {
    int slot;
    slot = (ch < 3) ? ch : (ch - 3 + 4);
    ym_write0(0x28, 0xF0 | slot);
}

static void ym_key_off(int ch) {
    int slot;
    slot = (ch < 3) ? ch : (ch - 3 + 4);
    ym_write0(0x28, slot);
}

static void ym_set_freq(int ch, int block, int fnum) {
    int hi;
    int lo;
    hi = ((block & 7) << 3) | ((fnum >> 8) & 7);
    lo = fnum & 0xFF;
    ym_write_ch(ch, 0xA4, hi);
    ym_write_ch(ch, 0xA0, lo);
}

/* ============================================================================
 * Frequency Tables
 * ============================================================================ */

static int fm_fnum[12] = {
    644, 682, 723, 766, 811, 859,
    910, 964, 1021, 1081, 1146, 1214
};

static int op_offset[4] = { 0, 8, 4, 12 };

static void note_to_freq(int note, int *block, int *fnum_out) {
    int octave;
    int semi;
    octave = note / 12;
    semi = note % 12;
    if (octave > 7) octave = 7;
    *block = octave;
    *fnum_out = fm_fnum[semi];
}

/* ============================================================================
 * FM Patches (inline data)
 * ============================================================================ */

/* Each patch: algo_fb, pan, then 4 ops x 7 params */
static unsigned char patch_bass[30] = {
    0x2C, 0xC0,  /* Algo 4, FB 5 */
    0x01, 32, 0x1F, 0x0E, 8, 0x37, 0,
    0x02, 8,  0x1F, 0x0A, 5, 0x29, 0,
    0x00, 28, 0x1F, 0x0C, 6, 0x27, 0,
    0x01, 5,  0x1F, 0x08, 4, 0x1A, 0
};

static unsigned char patch_pad[30] = {
    0x13, 0xC3,  /* Algo 7, FB 2, PMS=3 */
    0x31, 18, 0x10, 0x02, 1, 0x23, 0,
    0x42, 20, 0x10, 0x02, 1, 0x23, 0,
    0x03, 22, 0x10, 0x02, 1, 0x23, 0,
    0x71, 20, 0x10, 0x02, 1, 0x23, 0
};

static unsigned char patch_lead[30] = {
    0x35, 0xC5,  /* Algo 5, FB 6, PMS=5 */
    0x33, 36, 0x5F, 0x0A, 5, 0x36, 0,
    0x01, 10, 0x5F, 0x06, 3, 0x28, 0,
    0x02, 14, 0x5F, 0x07, 4, 0x28, 0,
    0x04, 14, 0x5F, 0x08, 4, 0x38, 0
};

static unsigned char patch_arp[30] = {
    0x24, 0xC0,  /* Algo 4, FB 4 */
    0x02, 40, 0x9F, 0x16, 18, 0x6C, 0,
    0x01, 14, 0x9F, 0x14, 15, 0x5E, 0,
    0x03, 44, 0x9F, 0x18, 18, 0x7C, 0,
    0x01, 12, 0x9F, 0x12, 14, 0x4E, 0
};

static unsigned char patch_kick[30] = {
    0x3C, 0xC0,  /* Algo 4, FB 7 */
    0x01, 18, 0x1F, 0x1F, 31, 0xFF, 0,
    0x00, 0,  0x1F, 0x12, 10, 0x8A, 0,
    0x01, 22, 0x1F, 0x1C, 20, 0xCC, 0,
    0x00, 4,  0x1F, 0x0F, 8,  0x6A, 0
};

static unsigned char patch_snare[30] = {
    0x3F, 0xC0,  /* Algo 7, FB 7 */
    0x77, 26, 0x1F, 0x14, 15, 0x8C, 0,
    0x31, 16, 0x1F, 0x12, 12, 0x7B, 0,
    0x59, 28, 0x1F, 0x16, 16, 0x9C, 0,
    0x01, 14, 0x1F, 0x10, 10, 0x6A, 0
};

static void load_patch(int ch, unsigned char *patch) {
    int op;
    int idx;
    int off;

    ym_write_ch(ch, 0xB0, patch[0]);
    ym_write_ch(ch, 0xB4, patch[1]);

    for (op = 0; op < 4; op++) {
        idx = 2 + (op * 7);
        off = op_offset[op];
        ym_write_ch(ch, 0x30 + off, patch[idx + 0]);
        ym_write_ch(ch, 0x40 + off, patch[idx + 1]);
        ym_write_ch(ch, 0x50 + off, patch[idx + 2]);
        ym_write_ch(ch, 0x60 + off, patch[idx + 3]);
        ym_write_ch(ch, 0x70 + off, patch[idx + 4]);
        ym_write_ch(ch, 0x80 + off, patch[idx + 5]);
        ym_write_ch(ch, 0x90 + off, patch[idx + 6]);
    }
}

static void ym_init(void) {
    int i;
    ym_write0(0x22, 0x00);
    ym_write0(0x27, 0x00);
    for (i = 0; i < 3; i++) {
        ym_write0(0x28, i);
        ym_write0(0x28, i + 4);
    }
    ym_write0(0x22, 0x08);
}

/* ============================================================================
 * Music Data
 * ============================================================================ */

#define N_C3  36
#define N_E3  40
#define N_G3  43
#define N_A3  45
#define N_C4  48
#define N_E4  52
#define N_G4  55
#define N_A4  57
#define N_C5  60
#define N_D5  62
#define N_E5  64

static int bass_notes[4] = { N_A3, N_G3, N_C4, N_G3 };
static int chord_root[4] = { N_A3, N_G3, N_C4, N_G3 };
static int chord_third[4] = { N_C4, N_C4, N_E4, N_C4 };
static int lead_melody[16] = {
    N_A4, 0, N_C5, 0, N_E5, 0, N_D5, 0,
    N_C5, 0, N_A4, 0, N_G4, 0, 0, 0
};
static int drum_pattern[8] = { 1, 0, 2, 0, 1, 0, 2, 0 };

/* Player state */
static int tick;
static int row;
static int chord_idx;
static int playing_lead;
static int arp_pos;

/* ============================================================================
 * Music Player
 * ============================================================================ */

static void play_note(int ch, int note) {
    int block;
    int fnum;
    if (note == 0) return;
    note_to_freq(note, &block, &fnum);
    ym_key_off(ch);
    ym_set_freq(ch, block, fnum);
    ym_key_on(ch);
}

static void process_music(void) {
    int bass_note;
    int drum;
    int lead_note;
    int arp_note;
    int block;
    int fnum;

    if (tick == 0) {
        chord_idx = (row / 4) % 4;

        bass_note = bass_notes[chord_idx];
        play_note(0, bass_note);

        if ((row % 4) == 0) {
            play_note(1, chord_root[chord_idx]);
            play_note(2, chord_third[chord_idx]);
        }

        lead_note = lead_melody[row % 16];
        if (lead_note != 0) {
            play_note(3, lead_note);
            playing_lead = 1;
        } else if (playing_lead) {
            ym_key_off(3);
            playing_lead = 0;
        }

        drum = drum_pattern[row % 8];
        if (drum & 1) {
            load_patch(5, patch_kick);
            note_to_freq(N_C3 - 12, &block, &fnum);
            ym_set_freq(5, block, fnum);
            ym_key_on(5);
        }
        if (drum & 2) {
            load_patch(5, patch_snare);
            note_to_freq(N_C4, &block, &fnum);
            ym_set_freq(5, block, fnum);
            ym_key_on(5);
        }

        row++;
        if (row >= 16) row = 0;
    }

    if ((tick % 2) == 0) {
        ym_key_off(4);
        arp_note = chord_root[chord_idx] + 12;
        if (arp_pos == 1) arp_note = chord_third[chord_idx] + 12;
        if (arp_pos == 2) arp_note = chord_root[chord_idx] + 24;
        note_to_freq(arp_note, &block, &fnum);
        ym_set_freq(4, block, fnum);
        ym_key_on(4);
        arp_pos = (arp_pos + 1) % 3;
    }

    tick++;
    if (tick >= 8) tick = 0;
}

/* ============================================================================
 * Main
 * ============================================================================ */

void main(void) {
    vdp_init();
    vdp_set_color(0, 0x0008);

    ym_init();

    load_patch(0, patch_bass);
    load_patch(1, patch_pad);
    load_patch(2, patch_pad);
    load_patch(3, patch_lead);
    load_patch(4, patch_arp);
    load_patch(5, patch_kick);

    tick = 0;
    row = 0;
    chord_idx = 0;
    playing_lead = 0;
    arp_pos = 0;

    while (1) {
        vdp_vsync();
        process_music();
    }
}
