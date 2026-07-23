/*
 * smd/snd_impl.h - SMD Sound Driver Implementation
 *
 * Include this file in ONE .c file to get the implementation.
 * Example:
 *   #define SND_IMPLEMENTATION
 *   #include <smd/snd.h>
 *   #include <smd/snd_impl.h>
 */

#ifndef SND_IMPL_H
#define SND_IMPL_H

#include <smd/snd.h>

/* ============================================================================
 * Hardware Registers
 * ============================================================================ */

#define YM_ADDR0    (*(volatile unsigned char *)0xA04000)
#define YM_DATA0    (*(volatile unsigned char *)0xA04001)
#define YM_ADDR1    (*(volatile unsigned char *)0xA04002)
#define YM_DATA1    (*(volatile unsigned char *)0xA04003)
#define PSG_PORT    (*(volatile unsigned char *)0xC00011)

/* ============================================================================
 * Frequency Tables (static const for ROM placement)
 * ============================================================================ */

/* YM2612 F-numbers for one octave (C to B) */
static const int snd_fm_fnum_table[12] = {
    644, 682, 723, 766, 811, 859,   /* C, C#, D, D#, E, F */
    910, 964, 1021, 1081, 1146, 1214 /* F#, G, G#, A, A#, B */
};

/* PSG dividers for notes C-2 to B-7 (72 notes) */
static const int snd_psg_div_table[72] = {
    /* Octave 2 */
    1710, 1614, 1524, 1438, 1357, 1281, 1209, 1141, 1077, 1017, 960, 906,
    /* Octave 3 */
    855, 807, 762, 719, 679, 641, 605, 571, 539, 508, 480, 453,
    /* Octave 4 */
    428, 404, 381, 360, 339, 320, 302, 285, 269, 254, 240, 226,
    /* Octave 5 */
    214, 202, 190, 180, 170, 160, 151, 143, 135, 127, 120, 113,
    /* Octave 6 */
    107, 101, 95, 90, 85, 80, 76, 71, 67, 64, 60, 57,
    /* Octave 7 */
    53, 50, 48, 45, 42, 40, 38, 36, 34, 32, 30, 28
};

/* Operator register offsets: Op1=+0, Op2=+8, Op3=+4, Op4=+12 */
static const int snd_op_offset[4] = { 0, 8, 4, 12 };

/* Vibrato sine table (64 entries, -127 to +127) */
static const signed char snd_vibrato_table[64] = {
    0, 12, 25, 37, 49, 60, 71, 81, 90, 98, 106, 112, 117, 122, 125, 126,
    127, 126, 125, 122, 117, 112, 106, 98, 90, 81, 71, 60, 49, 37, 25, 12,
    0, -12, -25, -37, -49, -60, -71, -81, -90, -98, -106, -112, -117, -122, -125, -126,
    -127, -126, -125, -122, -117, -112, -106, -98, -90, -81, -71, -60, -49, -37, -25, -12
};

/* ============================================================================
 * Channel State
 * ============================================================================ */

struct SndChannelState {
    /* Current state */
    unsigned char note;         /* Current note */
    unsigned char inst;         /* Current instrument */
    unsigned char volume;       /* Channel volume (0-127) */
    unsigned char playing;      /* Is channel sounding? */
    unsigned char sfx_active;   /* Is SFX playing on this channel? */
    unsigned char sfx_priority; /* SFX priority level */

    /* Frequency state */
    int freq;                   /* Current frequency value */
    int base_freq;              /* Base frequency (before effects) */
    int target_freq;            /* Target frequency (for portamento) */

    /* Effect state */
    unsigned char effect;       /* Current effect */
    unsigned char effect_param; /* Effect parameter */

    /* Vibrato */
    unsigned char vib_speed;
    unsigned char vib_depth;
    unsigned char vib_phase;

    /* Portamento */
    int porta_speed;

    /* Arpeggio */
    unsigned char arp_notes[3];
    unsigned char arp_pos;

    /* Volume slide */
    signed char vol_slide;

    /* PSG envelope */
    unsigned char env_pos;
    unsigned char env_tick;
};

/* ============================================================================
 * Driver State
 * ============================================================================ */

static struct {
    /* Song state */
    struct SndSong *song;
    int playing;
    int paused;

    /* Cached pointers (workaround for compiler limitation) */
    struct SndPattern *patterns;
    struct SndInstrument *instruments;
    unsigned char *sequence;
    int num_instruments;
    int sequence_length;
    int loop_point;

    /* Position */
    int seq_pos;                /* Current sequence position */
    int pattern;                /* Current pattern index */
    int row;                    /* Current row in pattern */
    int tick;                   /* Current tick in row */
    int ticks_per_row;          /* Speed setting */

    /* Volume/fade */
    int master_volume;          /* Master volume (0-127) */
    int fade_speed;             /* Fade speed (0 = no fade) */
    int fade_target;            /* Fade target volume */

    /* Pattern loop */
    int loop_row;
    int loop_count;

    /* Channel states */
    struct SndChannelState ch[SND_TOTAL_CHANNELS];

} snd_state;

/* SFX playback state (parallel arrays keep struct access simple) */
static unsigned char *snd_sfx_data[SND_TOTAL_CHANNELS];
static unsigned char snd_sfx_len[SND_TOTAL_CHANNELS];
static unsigned char snd_sfx_pos[SND_TOTAL_CHANNELS];

/* Channel preference for SND_CH_ANY: FM6..FM4 first (usually free of
 * melody), then PSG3..PSG1, noise, then the remaining FM channels */
static const unsigned char snd_sfx_ch_order[SND_TOTAL_CHANNELS] = {
    5, 4, 3, 8, 7, 6, 9, 2, 1, 0
};

/* ============================================================================
 * YM2612 Low-Level Functions
 * ============================================================================ */

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

/* ============================================================================
 * PSG Low-Level Functions
 * ============================================================================ */

static void psg_write(int val) {
    PSG_PORT = val;
}

/* ============================================================================
 * Public: Low-Level Channel Control
 * ============================================================================ */

void snd_fm_key_on(int channel) {
    int slot;
    if (channel < 0 || channel >= SND_FM_CHANNELS) return;
    slot = (channel < 3) ? channel : (channel - 3 + 4);
    ym_write0(0x28, 0xF0 | slot);
}

void snd_fm_key_off(int channel) {
    int slot;
    if (channel < 0 || channel >= SND_FM_CHANNELS) return;
    slot = (channel < 3) ? channel : (channel - 3 + 4);
    ym_write0(0x28, slot);
}

void snd_fm_set_freq(int channel, int block, int fnum) {
    int hi;
    int lo;
    if (channel < 0 || channel >= SND_FM_CHANNELS) return;
    hi = ((block & 7) << 3) | ((fnum >> 8) & 7);
    lo = fnum & 0xFF;
    ym_write_ch(channel, 0xA4, hi);
    ym_write_ch(channel, 0xA0, lo);
}

void snd_fm_set_patch(int channel, struct SndFmPatch *patch) {
    int op;
    int offset;
    int idx;
    unsigned char *data;

    if (channel < 0 || channel >= SND_FM_CHANNELS || !patch) return;

    /* Treat patch as byte array to avoid nested struct access issues */
    data = (unsigned char *)patch;

    /* Algorithm and feedback (byte 0) */
    ym_write_ch(channel, 0xB0, data[0]);
    /* Pan and LFO sensitivity (byte 1) */
    ym_write_ch(channel, 0xB4, data[1]);

    /* Load all 4 operators (each op is 7 bytes starting at byte 2) */
    for (op = 0; op < 4; op++) {
        offset = snd_op_offset[op];
        idx = 2 + (op * 7);
        ym_write_ch(channel, 0x30 + offset, data[idx + 0]);  /* dt_mul */
        ym_write_ch(channel, 0x40 + offset, data[idx + 1]);  /* tl */
        ym_write_ch(channel, 0x50 + offset, data[idx + 2]);  /* rs_ar */
        ym_write_ch(channel, 0x60 + offset, data[idx + 3]);  /* am_d1r */
        ym_write_ch(channel, 0x70 + offset, data[idx + 4]);  /* d2r */
        ym_write_ch(channel, 0x80 + offset, data[idx + 5]);  /* d1l_rr */
        ym_write_ch(channel, 0x90 + offset, data[idx + 6]);  /* ssg_eg */
    }
}

void snd_fm_set_volume(int channel, int volume) {
    /* Adjust TL of carrier operators based on algorithm */
    /* For simplicity, adjust Op4 TL (carrier in most algorithms) */
    int tl;
    if (channel < 0 || channel >= SND_FM_CHANNELS) return;
    tl = 127 - volume;
    if (tl < 0) tl = 0;
    if (tl > 127) tl = 127;
    ym_write_ch(channel, 0x40 + snd_op_offset[3], tl);
}

void snd_psg_set_freq(int channel, int divider) {
    int psg_ch;
    if (channel < SND_CH_PSG1 || channel > SND_CH_PSG3) return;
    psg_ch = channel - SND_CH_PSG1;
    psg_write(0x80 | (psg_ch << 5) | (divider & 0x0F));
    psg_write((divider >> 4) & 0x3F);
}

void snd_psg_set_volume(int channel, int volume) {
    int psg_ch;
    int att;
    if (channel < SND_CH_PSG1 || channel > SND_CH_NOISE) return;
    psg_ch = channel - SND_CH_PSG1;
    /* Convert 0-127 to 0-15 attenuation (inverted) */
    att = 15 - (volume >> 3);
    if (att < 0) att = 0;
    if (att > 15) att = 15;
    psg_write(0x90 | (psg_ch << 5) | att);
}

void snd_psg_set_noise(int mode) {
    psg_write(0xE0 | (mode & 0x07));
}

void snd_silence(void) {
    int i;
    /* Key off all FM channels */
    for (i = 0; i < SND_FM_CHANNELS; i++) {
        snd_fm_key_off(i);
    }
    /* Silence all PSG channels */
    for (i = SND_CH_PSG1; i <= SND_CH_NOISE; i++) {
        snd_psg_set_volume(i, 0);
    }
}

/* ============================================================================
 * Public: Utility Functions
 * ============================================================================ */

void snd_note_to_fm(int note, int *block, int *fnum) {
    int octave;
    int semitone;

    if (note >= SND_NOTE_CUT) {
        *block = 0;
        *fnum = 0;
        return;
    }

    octave = note / 12;
    semitone = note % 12;

    if (octave > 7) octave = 7;
    if (octave < 0) octave = 0;

    *block = octave;
    *fnum = snd_fm_fnum_table[semitone];
}

int snd_note_to_psg(int note) {
    int idx;
    if (note >= SND_NOTE_CUT) return 0;
    idx = note - 24;  /* Start from C-2 */
    if (idx < 0) idx = 0;
    if (idx >= 72) idx = 71;
    return snd_psg_div_table[idx];
}

/* ============================================================================
 * Internal: Process Effects
 * ============================================================================ */

static void snd_process_effect_tick(int ch_idx) {
    struct SndChannelState *ch;
    int freq_delta;

    ch = &snd_state.ch[ch_idx];

    switch (ch->effect) {
        case SND_FX_ARPEGGIO:
            if (ch->effect_param != 0) {
                ch->arp_pos = (ch->arp_pos + 1) % 3;
                /* Recalculate frequency with arpeggio offset */
            }
            break;

        case SND_FX_PORTA_UP:
            ch->freq += ch->effect_param * 4;
            break;

        case SND_FX_PORTA_DOWN:
            ch->freq -= ch->effect_param * 4;
            if (ch->freq < 0) ch->freq = 0;
            break;

        case SND_FX_PORTA_NOTE:
            if (ch->freq < ch->target_freq) {
                ch->freq += ch->porta_speed;
                if (ch->freq > ch->target_freq) ch->freq = ch->target_freq;
            } else if (ch->freq > ch->target_freq) {
                ch->freq -= ch->porta_speed;
                if (ch->freq < ch->target_freq) ch->freq = ch->target_freq;
            }
            break;

        case SND_FX_VIBRATO:
            /* Apply vibrato using sine table */
            freq_delta = (snd_vibrato_table[ch->vib_phase & 63] * ch->vib_depth) >> 7;
            ch->freq = ch->base_freq + freq_delta;
            ch->vib_phase += ch->vib_speed;
            break;

        case SND_FX_VOLSLIDE_DN:
            ch->volume -= ch->vol_slide;
            if (ch->volume < 0) ch->volume = 0;
            break;

        case SND_FX_VOLSLIDE_UP:
            ch->volume += ch->vol_slide;
            if (ch->volume > 127) ch->volume = 127;
            break;
    }
}

/* ============================================================================
 * Internal: Process Row
 * ============================================================================ */

static void snd_process_row(void) {
    struct SndPattern *pat;
    struct SndRow *row;
    struct SndNote *note_ptr;
    struct SndChannelState *ch;
    struct SndInstrument *inst;
    struct SndFmPatch *fm_patch;
    int i;
    int block;
    int fnum;
    int div;
    /* Cached note fields to avoid complex struct access */
    unsigned char n_note;
    unsigned char n_inst;
    unsigned char n_vol;
    unsigned char n_fx;
    unsigned char n_fx_param;

    if (!snd_state.patterns) return;

    pat = &snd_state.patterns[snd_state.pattern];
    if (snd_state.row >= pat->num_rows) return;

    row = &pat->rows[snd_state.row];

    for (i = 0; i < SND_TOTAL_CHANNELS; i++) {
        ch = &snd_state.ch[i];
        note_ptr = &row->ch[i];

        /* Cache note fields */
        n_note = note_ptr->note;
        n_inst = note_ptr->inst;
        n_vol = note_ptr->vol;
        n_fx = note_ptr->fx;
        n_fx_param = note_ptr->fx_param;

        /* Skip if SFX is active on this channel */
        if (ch->sfx_active) continue;

        /* Process instrument change */
        if (n_inst != 255 && n_inst < snd_state.num_instruments) {
            ch->inst = n_inst;
            inst = &snd_state.instruments[n_inst];

            /* Load instrument */
            if (i < SND_FM_CHANNELS && inst->type == 0) {
                fm_patch = inst->fm;
                snd_fm_set_patch(i, fm_patch);
            }
        }

        /* Process volume change */
        if (n_vol != 255) {
            ch->volume = n_vol;
        }

        /* Process note */
        if (n_note == SND_NOTE_OFF || n_note == SND_NOTE_CUT) {
            /* Note off */
            if (i < SND_FM_CHANNELS) {
                snd_fm_key_off(i);
            } else {
                snd_psg_set_volume(i, 0);
            }
            ch->playing = 0;
        } else if (n_note != SND_NOTE_NONE && n_note < 128) {
            /* New note */
            ch->note = n_note;

            if (i < SND_FM_CHANNELS) {
                /* FM channel */
                snd_note_to_fm(n_note, &block, &fnum);
                ch->base_freq = (block << 11) | fnum;
                ch->freq = ch->base_freq;

                /* Handle portamento to note */
                if (n_fx == SND_FX_PORTA_NOTE) {
                    ch->target_freq = ch->base_freq;
                    ch->porta_speed = n_fx_param * 4;
                } else {
                    snd_fm_key_off(i);
                    snd_fm_set_freq(i, block, fnum);
                    snd_fm_key_on(i);
                }
            } else {
                /* PSG channel */
                if (i == SND_CH_NOISE) {
                    snd_psg_set_noise(n_note & 7);
                } else {
                    div = snd_note_to_psg(n_note);
                    ch->freq = div;
                    ch->base_freq = div;
                    snd_psg_set_freq(i, div);
                }
                snd_psg_set_volume(i, ch->volume);
            }
            ch->playing = 1;
            ch->env_pos = 0;
            ch->env_tick = 0;
            ch->vib_phase = 0;
            ch->arp_pos = 0;
        }

        /* Setup effect */
        ch->effect = n_fx;
        ch->effect_param = n_fx_param;

        switch (n_fx) {
            case SND_FX_ARPEGGIO:
                if (n_fx_param != 0) {
                    ch->arp_notes[0] = 0;
                    ch->arp_notes[1] = (n_fx_param >> 4) & 0x0F;
                    ch->arp_notes[2] = n_fx_param & 0x0F;
                    ch->arp_pos = 0;
                }
                break;

            case SND_FX_VIBRATO:
                ch->vib_speed = (n_fx_param >> 4) & 0x0F;
                ch->vib_depth = (n_fx_param & 0x0F) * 4;
                break;

            case SND_FX_VOLSLIDE_DN:
            case SND_FX_VOLSLIDE_UP:
                ch->vol_slide = n_fx_param;
                break;

            case SND_FX_VOLUME:
                ch->volume = n_fx_param;
                if (i < SND_FM_CHANNELS) {
                    snd_fm_set_volume(i, ch->volume);
                } else {
                    snd_psg_set_volume(i, ch->volume);
                }
                break;

            case SND_FX_SPEED:
                snd_state.ticks_per_row = n_fx_param;
                break;

            case SND_FX_JUMP:
                snd_state.seq_pos = n_fx_param;
                if (snd_state.seq_pos >= snd_state.sequence_length) {
                    snd_state.seq_pos = snd_state.loop_point;
                }
                snd_state.pattern = snd_state.sequence[snd_state.seq_pos];
                snd_state.row = -1;  /* Will increment to 0 */
                break;

            case SND_FX_BREAK:
                snd_state.seq_pos++;
                if (snd_state.seq_pos >= snd_state.sequence_length) {
                    if (snd_state.loop_point != 255) {
                        snd_state.seq_pos = snd_state.loop_point;
                    } else {
                        snd_state.playing = 0;
                        return;
                    }
                }
                snd_state.pattern = snd_state.sequence[snd_state.seq_pos];
                snd_state.row = n_fx_param - 1;  /* Will increment */
                break;
        }
    }
}

/* ============================================================================
 * Internal: Update Channels
 * ============================================================================ */

static void snd_update_channels(void) {
    struct SndChannelState *ch;
    int i;
    int block;
    int fnum;

    for (i = 0; i < SND_TOTAL_CHANNELS; i++) {
        ch = &snd_state.ch[i];

        if (!ch->playing || ch->sfx_active) continue;

        /* Process effect every tick (except first tick of row) */
        if (snd_state.tick > 0) {
            snd_process_effect_tick(i);
        }

        /* Update FM frequency */
        if (i < SND_FM_CHANNELS) {
            block = (ch->freq >> 11) & 7;
            fnum = ch->freq & 0x7FF;
            snd_fm_set_freq(i, block, fnum);

            /* Update volume if volume slide active */
            if (ch->effect == SND_FX_VOLSLIDE_DN || ch->effect == SND_FX_VOLSLIDE_UP) {
                snd_fm_set_volume(i, (ch->volume * snd_state.master_volume) >> 7);
            }
        } else {
            /* PSG: update frequency and volume */
            if (i != SND_CH_NOISE) {
                snd_psg_set_freq(i, ch->freq);
            }
            snd_psg_set_volume(i, (ch->volume * snd_state.master_volume) >> 7);
        }
    }
}

/* ============================================================================
 * Internal: SFX Engine
 * ============================================================================ */

/* Cut whatever is sounding on a channel */
static void snd_sfx_silence_channel(int ch_idx) {
    if (ch_idx < SND_FM_CHANNELS) {
        snd_fm_key_off(ch_idx);
    } else {
        snd_psg_set_volume(ch_idx, 0);
    }
}

/* Release a channel back to the music engine */
static void snd_sfx_end(int ch_idx) {
    struct SndChannelState *ch_ptr;
    ch_ptr = &snd_state.ch[ch_idx];
    ch_ptr->sfx_active = 0;
    ch_ptr->sfx_priority = 0;
    snd_sfx_data[ch_idx] = 0;
    snd_sfx_len[ch_idx] = 0;
    snd_sfx_pos[ch_idx] = 0;
    snd_sfx_silence_channel(ch_idx);
    /* Music retriggers on this channel at its next note */
}

static void snd_sfx_apply_frame(int ch_idx, int note, int vol) {
    int block;
    int fnum;

    if (note == SND_NOTE_OFF || note == SND_NOTE_CUT) {
        snd_sfx_silence_channel(ch_idx);
    } else if (note != SND_NOTE_NONE) {
        if (ch_idx < SND_FM_CHANNELS) {
            snd_note_to_fm(note, &block, &fnum);
            snd_fm_key_off(ch_idx);
            snd_fm_set_freq(ch_idx, block, fnum);
            snd_fm_key_on(ch_idx);
        } else if (ch_idx == SND_CH_NOISE) {
            snd_psg_set_noise(note & 0x07);
        } else {
            snd_psg_set_freq(ch_idx, snd_note_to_psg(note));
        }
    }

    if (vol != 255) {
        if (ch_idx < SND_FM_CHANNELS) {
            snd_fm_set_volume(ch_idx, (vol * snd_state.master_volume) >> 7);
        } else {
            snd_psg_set_volume(ch_idx, (vol * snd_state.master_volume) >> 7);
        }
    }
}

/* Advance all active sound effects by one frame */
static void snd_update_sfx(void) {
    struct SndChannelState *ch_ptr;
    unsigned char *frame;
    int i;

    for (i = 0; i < SND_TOTAL_CHANNELS; i++) {
        ch_ptr = &snd_state.ch[i];
        if (!ch_ptr->sfx_active) continue;

        if (snd_sfx_pos[i] >= snd_sfx_len[i]) {
            snd_sfx_end(i);
            continue;
        }

        frame = snd_sfx_data[i] + snd_sfx_pos[i] * 2;
        snd_sfx_apply_frame(i, frame[0], frame[1]);
        snd_sfx_pos[i]++;
    }
}

static int snd_sfx_pick_channel(struct SndSfx *sfx) {
    struct SndChannelState *ch_ptr;
    int i;
    int ch_idx;
    int best;
    int best_pri;

    /* Explicit channel request: honor it unless a more important SFX holds it */
    if (sfx->channel != SND_CH_ANY) {
        if (sfx->channel >= SND_TOTAL_CHANNELS) return -1;
        ch_ptr = &snd_state.ch[sfx->channel];
        if (ch_ptr->sfx_active && sfx->priority < ch_ptr->sfx_priority) return -1;
        return sfx->channel;
    }

    /* Pass 1: an idle channel (no SFX, no music note) */
    for (i = 0; i < SND_TOTAL_CHANNELS; i++) {
        ch_idx = snd_sfx_ch_order[i];
        ch_ptr = &snd_state.ch[ch_idx];
        if (!ch_ptr->sfx_active && !ch_ptr->playing) return ch_idx;
    }

    /* Pass 2: mask a music channel without active SFX */
    for (i = 0; i < SND_TOTAL_CHANNELS; i++) {
        ch_idx = snd_sfx_ch_order[i];
        ch_ptr = &snd_state.ch[ch_idx];
        if (!ch_ptr->sfx_active) return ch_idx;
    }

    /* Pass 3: steal the lowest-priority SFX we outrank or match */
    best = -1;
    best_pri = 255;
    for (i = 0; i < SND_TOTAL_CHANNELS; i++) {
        ch_idx = snd_sfx_ch_order[i];
        ch_ptr = &snd_state.ch[ch_idx];
        if (ch_ptr->sfx_priority < best_pri) {
            best_pri = ch_ptr->sfx_priority;
            best = ch_idx;
        }
    }
    if (best >= 0 && best_pri <= sfx->priority) return best;
    return -1;
}

/* ============================================================================
 * Public API Implementation
 * ============================================================================ */

void snd_init(void) {
    int i;

    /* Clear state */
    snd_state.song = 0;
    snd_state.playing = 0;
    snd_state.paused = 0;
    snd_state.seq_pos = 0;
    snd_state.pattern = 0;
    snd_state.row = 0;
    snd_state.tick = 0;
    snd_state.ticks_per_row = 6;
    snd_state.master_volume = 127;
    snd_state.fade_speed = 0;
    snd_state.fade_target = 127;

    /* Clear cached pointers */
    snd_state.patterns = 0;
    snd_state.instruments = 0;
    snd_state.sequence = 0;
    snd_state.num_instruments = 0;
    snd_state.sequence_length = 0;
    snd_state.loop_point = 0;

    /* Clear channel states (use pointer to avoid compiler limitation) */
    for (i = 0; i < SND_TOTAL_CHANNELS; i++) {
        struct SndChannelState *ch_ptr;
        ch_ptr = &snd_state.ch[i];
        ch_ptr->note = 0;
        ch_ptr->inst = 0;
        ch_ptr->volume = 100;
        ch_ptr->playing = 0;
        ch_ptr->sfx_active = 0;
        ch_ptr->sfx_priority = 0;
        ch_ptr->freq = 0;
        ch_ptr->base_freq = 0;
        ch_ptr->target_freq = 0;
        ch_ptr->effect = 0;
        ch_ptr->effect_param = 0;
        ch_ptr->vib_speed = 0;
        ch_ptr->vib_depth = 0;
        ch_ptr->vib_phase = 0;
        ch_ptr->env_pos = 0;
        ch_ptr->env_tick = 0;

        snd_sfx_data[i] = 0;
        snd_sfx_len[i] = 0;
        snd_sfx_pos[i] = 0;
    }

    /* Initialize YM2612 */
    ym_write0(0x22, 0x00);  /* LFO off */
    ym_write0(0x27, 0x00);  /* Timers off */

    /* Key off all FM channels */
    for (i = 0; i < 3; i++) {
        ym_write0(0x28, i);
        ym_write0(0x28, i + 4);
    }

    /* Enable LFO for vibrato */
    ym_write0(0x22, 0x08);  /* LFO on, frequency 0 */

    /* Silence PSG */
    psg_write(0x9F);
    psg_write(0xBF);
    psg_write(0xDF);
    psg_write(0xFF);
}

void snd_update(void) {
    struct SndPattern *pat;

    /* Handle fade */
    if (snd_state.fade_speed != 0) {
        if (snd_state.fade_speed > 0) {
            snd_state.master_volume += snd_state.fade_speed;
            if (snd_state.master_volume >= snd_state.fade_target) {
                snd_state.master_volume = snd_state.fade_target;
                snd_state.fade_speed = 0;
            }
        } else {
            snd_state.master_volume += snd_state.fade_speed;
            if (snd_state.master_volume <= snd_state.fade_target) {
                snd_state.master_volume = snd_state.fade_target;
                snd_state.fade_speed = 0;
                if (snd_state.fade_target == 0) {
                    snd_stop_song();
                }
            }
        }
    }

    /* Advance sound effects (they run even when no song is playing) */
    snd_update_sfx();

    /* Don't process if not playing or paused */
    if (!snd_state.playing || snd_state.paused || !snd_state.song) {
        return;
    }

    /* Process tick */
    if (snd_state.tick == 0) {
        snd_process_row();
    }

    /* Update channel effects */
    snd_update_channels();

    /* Advance tick */
    snd_state.tick++;
    if (snd_state.tick >= snd_state.ticks_per_row) {
        snd_state.tick = 0;
        snd_state.row++;

        /* Check for pattern end */
        pat = &snd_state.patterns[snd_state.pattern];
        if (snd_state.row >= pat->num_rows) {
            snd_state.row = 0;
            snd_state.seq_pos++;

            /* Check for song end */
            if (snd_state.seq_pos >= snd_state.sequence_length) {
                if (snd_state.loop_point != 255) {
                    snd_state.seq_pos = snd_state.loop_point;
                } else {
                    snd_state.playing = 0;
                    snd_silence();
                    return;
                }
            }
            snd_state.pattern = snd_state.sequence[snd_state.seq_pos];
        }
    }
}

void snd_play_song(struct SndSong *song) {
    if (!song) return;

    snd_silence();

    snd_state.song = song;

    /* Cache pointers to avoid complex struct access */
    snd_state.patterns = song->patterns;
    snd_state.instruments = song->instruments;
    snd_state.sequence = song->sequence;
    snd_state.num_instruments = song->num_instruments;
    snd_state.sequence_length = song->sequence_length;
    snd_state.loop_point = song->loop_point;

    snd_state.playing = 1;
    snd_state.paused = 0;
    snd_state.seq_pos = 0;
    snd_state.pattern = snd_state.sequence[0];
    snd_state.row = 0;
    snd_state.tick = 0;
    snd_state.ticks_per_row = song->initial_speed;
    snd_state.master_volume = 127;
    snd_state.fade_speed = 0;
}

void snd_stop_song(void) {
    snd_state.playing = 0;
    snd_silence();
}

void snd_pause_song(void) {
    snd_state.paused = 1;
    snd_silence();
}

void snd_resume_song(void) {
    snd_state.paused = 0;
}

int snd_is_playing(void) {
    return snd_state.playing && !snd_state.paused;
}

void snd_set_master_volume(int volume) {
    if (volume < 0) volume = 0;
    if (volume > 127) volume = 127;
    snd_state.master_volume = volume;
}

void snd_fade_out(int frames) {
    if (frames <= 0) {
        snd_state.master_volume = 0;
        snd_stop_song();
        return;
    }
    snd_state.fade_speed = -(snd_state.master_volume / frames);
    if (snd_state.fade_speed == 0) snd_state.fade_speed = -1;
    snd_state.fade_target = 0;
}

void snd_fade_in(int frames) {
    int start_vol;
    if (frames <= 0) {
        snd_state.master_volume = 127;
        return;
    }
    start_vol = snd_state.master_volume;
    snd_state.fade_speed = (127 - start_vol) / frames;
    if (snd_state.fade_speed == 0) snd_state.fade_speed = 1;
    snd_state.fade_target = 127;
}

void snd_set_speed(int speed) {
    if (speed < 1) speed = 1;
    if (speed > 31) speed = 31;
    snd_state.ticks_per_row = speed;
}

int snd_get_pattern(void) {
    return snd_state.pattern;
}

int snd_get_row(void) {
    return snd_state.row;
}

int snd_play_sfx(struct SndSfx *sfx) {
    struct SndChannelState *ch_ptr;
    int ch_idx;

    if (!sfx) return -1;
    if (sfx->length == 0) return -1;
    if (!sfx->data) return -1;

    ch_idx = snd_sfx_pick_channel(sfx);
    if (ch_idx < 0) return -1;

    ch_ptr = &snd_state.ch[ch_idx];
    ch_ptr->sfx_active = 1;
    ch_ptr->sfx_priority = sfx->priority;

    snd_sfx_data[ch_idx] = sfx->data;
    snd_sfx_len[ch_idx] = sfx->length;
    snd_sfx_pos[ch_idx] = 0;

    /* Cut any note sounding on the channel; the first SFX frame is
     * applied by the next snd_update call */
    snd_sfx_silence_channel(ch_idx);

    return ch_idx;
}

void snd_stop_sfx(int channel) {
    struct SndChannelState *ch_ptr;
    if (channel >= 0 && channel < SND_TOTAL_CHANNELS) {
        ch_ptr = &snd_state.ch[channel];
        if (ch_ptr->sfx_active) {
            snd_sfx_end(channel);
        }
    }
}

void snd_stop_all_sfx(void) {
    struct SndChannelState *ch_ptr;
    int i;
    for (i = 0; i < SND_TOTAL_CHANNELS; i++) {
        ch_ptr = &snd_state.ch[i];
        if (ch_ptr->sfx_active) {
            snd_sfx_end(i);
        }
    }
}

#endif /* SND_IMPL_H */
