/*
 * smd/snd.h - SMD Sound Driver v1.0
 *
 * Modern sound driver for Sega Genesis/Megadrive
 * Designed for SMD Compiler SDK (2025-2026)
 *
 * Features:
 * - 6 FM channels + 4 PSG channels
 * - Pattern-based music sequencing
 * - Effects: vibrato, portamento, arpeggio, volume slide
 * - PSG software envelopes
 * - SFX priority system
 * - VBlank-driven (call snd_update every frame)
 *
 * Architecture:
 * - Runs on 68000 (simple, debuggable)
 * - Compact data format
 * - Memory efficient (~2KB RAM)
 */

#ifndef SMD_SND_H
#define SMD_SND_H

/* ============================================================================
 * Configuration
 * ============================================================================ */

#define SND_FM_CHANNELS     6
#define SND_PSG_CHANNELS    4
#define SND_TOTAL_CHANNELS  10

#define SND_MAX_PATTERNS    64
#define SND_MAX_INSTRUMENTS 32
#define SND_MAX_SFX         32
#define SND_ROWS_PER_PATTERN 64

/* Channel indices */
#define SND_CH_ANY      255 /* Let the driver pick an SFX channel */
#define SND_CH_FM1      0
#define SND_CH_FM2      1
#define SND_CH_FM3      2
#define SND_CH_FM4      3
#define SND_CH_FM5      4
#define SND_CH_FM6      5   /* Can be used for DAC/PCM */
#define SND_CH_PSG1     6
#define SND_CH_PSG2     7
#define SND_CH_PSG3     8
#define SND_CH_NOISE    9

/* Priority levels (higher = more important) */
#define SND_PRIORITY_LOW      1
#define SND_PRIORITY_NORMAL   2
#define SND_PRIORITY_HIGH     3
#define SND_PRIORITY_CRITICAL 4

/* ============================================================================
 * Note Definitions
 * ============================================================================ */

/* Note format: octave * 12 + semitone (C=0, C#=1, ..., B=11) */
/* C-0 = 0, C-1 = 12, C-2 = 24, C-3 = 36, C-4 = 48 (middle C), etc. */

#define SND_NOTE_C      0
#define SND_NOTE_CS     1
#define SND_NOTE_D      2
#define SND_NOTE_DS     3
#define SND_NOTE_E      4
#define SND_NOTE_F      5
#define SND_NOTE_FS     6
#define SND_NOTE_G      7
#define SND_NOTE_GS     8
#define SND_NOTE_A      9
#define SND_NOTE_AS     10
#define SND_NOTE_B      11

#define SND_OCTAVE(n)   ((n) * 12)
#define SND_NOTE(oct, note) (SND_OCTAVE(oct) + (note))

/* Special note values */
#define SND_NOTE_OFF    254
#define SND_NOTE_NONE   255
#define SND_NOTE_CUT    253  /* Instant note cut */

/* ============================================================================
 * Effect Definitions
 * ============================================================================ */

#define SND_FX_NONE         0x00
#define SND_FX_ARPEGGIO     0x00  /* xy: +x semitones, +y semitones */
#define SND_FX_PORTA_UP     0x01  /* xx: speed */
#define SND_FX_PORTA_DOWN   0x02  /* xx: speed */
#define SND_FX_PORTA_NOTE   0x03  /* xx: speed (slide to note) */
#define SND_FX_VIBRATO      0x04  /* xy: speed, depth */
#define SND_FX_VOLSLIDE_DN  0x05  /* xx: speed down */
#define SND_FX_VOLSLIDE_UP  0x06  /* xx: speed up */
#define SND_FX_TREMOLO      0x07  /* xy: speed, depth */
#define SND_FX_PAN          0x08  /* xx: 00=left, 80=center, FF=right */
#define SND_FX_SPEED        0x09  /* xx: ticks per row */
#define SND_FX_VOLUME       0x0A  /* xx: volume (0-127) */
#define SND_FX_JUMP         0x0B  /* xx: pattern index */
#define SND_FX_BREAK        0x0D  /* xx: row in next pattern */
#define SND_FX_TEMPO        0x0F  /* xx: BPM (if >= 0x20) or speed (if < 0x20) */

/* Extended effects (Exy) */
#define SND_FX_EXT          0x0E
#define SND_FX_EXT_FINE_PORTA_UP    0x10  /* 1x: fine porta up */
#define SND_FX_EXT_FINE_PORTA_DN    0x20  /* 2x: fine porta down */
#define SND_FX_EXT_VIBRATO_WAVE     0x40  /* 4x: vibrato waveform */
#define SND_FX_EXT_FINETUNE         0x50  /* 5x: finetune */
#define SND_FX_EXT_LOOP             0x60  /* 6x: pattern loop */
#define SND_FX_EXT_TREMOLO_WAVE     0x70  /* 7x: tremolo waveform */
#define SND_FX_EXT_NOTE_CUT         0xC0  /* Cx: cut note after x ticks */
#define SND_FX_EXT_NOTE_DELAY       0xD0  /* Dx: delay note x ticks */

/* ============================================================================
 * Data Structures
 * ============================================================================ */

/*
 * FM Instrument (30 bytes) - YM2612 patch
 */
struct SndFmPatch {
    unsigned char algo_fb;      /* Algorithm (0-7) + Feedback (0-7) << 3 */
    unsigned char pan_ams_pms;  /* L/R/AMS/PMS */
    struct {
        unsigned char dt_mul;   /* Detune + Multiple */
        unsigned char tl;       /* Total Level (volume, 0=loud, 127=quiet) */
        unsigned char rs_ar;    /* Rate Scale + Attack Rate */
        unsigned char am_d1r;   /* AM enable + Decay 1 Rate */
        unsigned char d2r;      /* Decay 2 Rate */
        unsigned char d1l_rr;   /* Decay 1 Level + Release Rate */
        unsigned char ssg_eg;   /* SSG-EG (usually 0) */
    } op[4];
};

/*
 * PSG Envelope (variable length)
 */
struct SndPsgEnvelope {
    unsigned char length;       /* Number of entries */
    unsigned char loop_point;   /* Loop back point (255 = no loop) */
    unsigned char sustain_point;/* Sustain point (255 = none) */
    unsigned char data[32];     /* Volume values (0-15, 15=silent) */
};

/*
 * PSG Instrument
 */
struct SndPsgPatch {
    unsigned char noise_mode;   /* 0=tone, 1-7=noise modes */
    unsigned char duty;         /* Reserved for future use */
    struct SndPsgEnvelope *vol_env;   /* Volume envelope (NULL = none) */
    struct SndPsgEnvelope *pitch_env; /* Pitch envelope (NULL = none) */
};

/*
 * Instrument container
 * Note: Union not supported by compiler, using separate pointer
 */
struct SndInstrument {
    unsigned char type;         /* 0=FM, 1=PSG */
    struct SndFmPatch *fm;      /* FM patch (type=0) */
};

/*
 * Pattern note entry (5 bytes, compact)
 */
struct SndNote {
    unsigned char note;         /* Note value (0-127, or special) */
    unsigned char inst;         /* Instrument (255 = no change) */
    unsigned char vol;          /* Volume (255 = no change, 0-127 = set) */
    unsigned char fx;           /* Effect type */
    unsigned char fx_param;     /* Effect parameter */
};

/*
 * Pattern row (all channels)
 */
struct SndRow {
    struct SndNote ch[SND_TOTAL_CHANNELS];
};

/*
 * Pattern
 */
struct SndPattern {
    unsigned char num_rows;     /* Rows in this pattern (1-64) */
    struct SndRow *rows;        /* Row data */
};

/*
 * Song
 */
struct SndSong {
    unsigned char num_patterns;     /* Number of patterns */
    unsigned char sequence_length;  /* Length of play sequence */
    unsigned char loop_point;       /* Where to loop (255 = no loop) */
    unsigned char initial_speed;    /* Initial ticks per row */
    unsigned char initial_tempo;    /* Initial BPM (for timing reference) */
    unsigned char num_instruments;  /* Number of instruments */

    unsigned char *sequence;        /* Pattern play order */
    struct SndPattern *patterns;    /* Pattern data */
    struct SndInstrument *instruments; /* Instrument data */
};

/*
 * Sound Effect
 *
 * Frame data: 2 bytes per frame, played one frame per snd_update call.
 *   byte 0: note  - 0-127 (re)triggers the note; SND_NOTE_OFF keys off;
 *                   SND_NOTE_CUT silences instantly; SND_NOTE_NONE keeps
 *                   the previous pitch. On SND_CH_NOISE the low 3 bits
 *                   select the noise mode instead of a pitch.
 *   byte 1: volume - 0-127 sets the channel volume; 255 = no change.
 *
 * FM sound effects use whatever patch is currently loaded on the channel;
 * set one with snd_fm_set_patch() before triggering if needed.
 */
struct SndSfx {
    unsigned char channel;      /* Preferred channel (or SND_CH_ANY) */
    unsigned char priority;     /* Priority level (SND_PRIORITY_*) */
    unsigned char length;       /* Number of frames */
    unsigned char *data;        /* Frame data: length * 2 bytes */
};

/* ============================================================================
 * API Functions
 * ============================================================================ */

/*
 * Initialize sound driver
 * Call once at startup
 */
void snd_init(void);

/*
 * Update sound driver
 * MUST be called every VBlank (60Hz NTSC, 50Hz PAL)
 */
void snd_update(void);

/*
 * Load and play a song
 */
void snd_play_song(struct SndSong *song);

/*
 * Stop current song
 */
void snd_stop_song(void);

/*
 * Pause/resume song
 */
void snd_pause_song(void);
void snd_resume_song(void);

/*
 * Check if song is playing
 * Returns: 1 if playing, 0 if stopped
 */
int snd_is_playing(void);

/*
 * Set master volume (0-127)
 */
void snd_set_master_volume(int volume);

/*
 * Play a sound effect
 * Returns: channel used, or -1 if no channel available
 */
int snd_play_sfx(struct SndSfx *sfx);

/*
 * Stop a sound effect on channel
 */
void snd_stop_sfx(int channel);

/*
 * Stop all sound effects
 */
void snd_stop_all_sfx(void);

/*
 * Fade out music over N frames
 */
void snd_fade_out(int frames);

/*
 * Fade in music over N frames
 */
void snd_fade_in(int frames);

/*
 * Set song speed (ticks per row)
 */
void snd_set_speed(int speed);

/*
 * Get current playback position
 */
int snd_get_pattern(void);
int snd_get_row(void);

/* ============================================================================
 * Low-Level Channel Control
 * ============================================================================ */

/*
 * Direct FM channel control
 */
void snd_fm_set_patch(int channel, struct SndFmPatch *patch);
void snd_fm_set_freq(int channel, int block, int fnum);
void snd_fm_key_on(int channel);
void snd_fm_key_off(int channel);
void snd_fm_set_volume(int channel, int volume);

/*
 * Direct PSG channel control
 */
void snd_psg_set_freq(int channel, int divider);
void snd_psg_set_volume(int channel, int volume);
void snd_psg_set_noise(int mode);

/*
 * Silence all channels
 */
void snd_silence(void);

/* ============================================================================
 * Utility Functions
 * ============================================================================ */

/*
 * Convert note number to FM frequency
 */
void snd_note_to_fm(int note, int *block, int *fnum);

/*
 * Convert note number to PSG divider
 */
int snd_note_to_psg(int note);

/* ============================================================================
 * Pre-defined FM Patches (optional, in snd_patches.h)
 * ============================================================================ */

/* Patch creation macros */
#define SND_FM_ALGO_FB(algo, fb)    ((((fb) & 7) << 3) | ((algo) & 7))
#define SND_FM_PAN_LR               0xC0
#define SND_FM_PAN_L                0x80
#define SND_FM_PAN_R                0x40
#define SND_FM_DT_MUL(dt, mul)      ((((dt) & 7) << 4) | ((mul) & 15))
#define SND_FM_TL(tl)               ((tl) & 127)
#define SND_FM_RS_AR(rs, ar)        ((((rs) & 3) << 6) | ((ar) & 31))
#define SND_FM_AM_D1R(am, d1r)      ((((am) & 1) << 7) | ((d1r) & 31))
#define SND_FM_D2R(d2r)             ((d2r) & 31)
#define SND_FM_D1L_RR(d1l, rr)      ((((d1l) & 15) << 4) | ((rr) & 15))

#endif /* SMD_SND_H */
