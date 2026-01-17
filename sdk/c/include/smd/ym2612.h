/*
 * smd/ym2612.h - YM2612 FM Synthesizer interface
 *
 * The YM2612 is the FM synthesis chip in the Sega Genesis:
 * - 6 FM channels (0-5)
 * - 4 operators per channel
 * - 8 FM algorithms
 * - Stereo output (L/R per channel)
 * - DAC mode on channel 6 for PCM samples
 *
 * Note: smdc uses int for all parameters.
 */

#ifndef SMD_YM2612_H
#define SMD_YM2612_H

/* ========================================================================== */
/* Hardware Addresses                                                         */
/* ========================================================================== */

#define YM2612_ADDR0    0xA04000
#define YM_ADDR_PORT0   0xA04000
#define YM2612_DATA0    0xA04001
#define YM_DATA_PORT0   0xA04001
#define YM2612_ADDR1    0xA04002
#define YM_ADDR_PORT1   0xA04002
#define YM2612_DATA1    0xA04003
#define YM_DATA_PORT1   0xA04003

/* ========================================================================== */
/* Register Definitions                                                       */
/* ========================================================================== */

/* Global Registers (Port 0 only) */
#define YM_REG_LFO          0x22
#define YM_REG_TIMER_A_HI   0x24
#define YM_REG_TIMER_A_LO   0x25
#define YM_REG_TIMER_B      0x26
#define YM_REG_TIMER_CTRL   0x27
#define YM_REG_KEY_ONOFF    0x28
#define YM_REG_DAC          0x2A
#define YM_REG_DAC_EN       0x2B

/* Per-Channel Registers (add channel offset 0-2) */
#define YM_REG_FREQ_LO      0xA0
#define YM_REG_FREQ_HI      0xA4
#define YM_REG_ALGO_FB      0xB0
#define YM_REG_STEREO_LFO   0xB4

/* Per-Operator Registers */
/* Operator offsets: Op1=0, Op2=8, Op3=4, Op4=12 */
#define YM_REG_OP_DT_MUL    0x30
#define YM_REG_OP_TL        0x40
#define YM_REG_OP_RS_AR     0x50
#define YM_REG_OP_AM_D1R    0x60
#define YM_REG_OP_D2R       0x70
#define YM_REG_OP_D1L_RR    0x80
#define YM_REG_OP_SSG_EG    0x90

/* ========================================================================== */
/* Algorithm Definitions                                                      */
/* ========================================================================== */

/*
 * Algorithm 0: [1]→[2]→[3]→[4]→OUT  (Maximum modulation - metallic/harsh)
 * Algorithm 1: [1+2]→[3]→[4]→OUT
 * Algorithm 2: [1+(2→3)]→[4]→OUT
 * Algorithm 3: [(1→2)+3]→[4]→OUT
 * Algorithm 4: [1→2]+[3→4]→OUT      (Two FM pairs - common, versatile)
 * Algorithm 5: [1→2+3+4]→OUT        (One mod, three carriers)
 * Algorithm 6: [1→2]+[3]+[4]→OUT
 * Algorithm 7: [1]+[2]+[3]+[4]→OUT  (All carriers - organ/additive)
 */
#define YM_ALGO_0           0
#define YM_ALGO_1           1
#define YM_ALGO_2           2
#define YM_ALGO_3           3
#define YM_ALGO_4           4
#define YM_ALGO_5           5
#define YM_ALGO_6           6
#define YM_ALGO_7           7

/* Friendly names */
#define YM_ALGO_SERIAL      0
#define YM_ALGO_PIANO       4
#define YM_ALGO_DISTORTION  5
#define YM_ALGO_ORGAN       7

/* ========================================================================== */
/* Stereo Panning                                                             */
/* ========================================================================== */

#define YM_PAN_OFF          0x00
#define YM_PAN_RIGHT        0x40
#define YM_PAN_LEFT         0x80
#define YM_PAN_CENTER       0xC0

/* ========================================================================== */
/* LFO Settings                                                               */
/* ========================================================================== */

#define YM_LFO_OFF          0x00
#define YM_LFO_3_98HZ       0x08
#define YM_LFO_5_56HZ       0x09
#define YM_LFO_6_02HZ       0x0A
#define YM_LFO_6_37HZ       0x0B
#define YM_LFO_6_88HZ       0x0C
#define YM_LFO_9_63HZ       0x0D
#define YM_LFO_48_1HZ       0x0E
#define YM_LFO_72_2HZ       0x0F

/* ========================================================================== */
/* Note Frequency Table (F-number values for block 4)                         */
/* ========================================================================== */

#define YM_NOTE_C   644
#define YM_NOTE_CS  682
#define YM_NOTE_D   723
#define YM_NOTE_DS  766
#define YM_NOTE_E   811
#define YM_NOTE_F   859
#define YM_NOTE_FS  910
#define YM_NOTE_G   964
#define YM_NOTE_GS  1021
#define YM_NOTE_A   1081
#define YM_NOTE_AS  1146
#define YM_NOTE_B   1214

/* Octave/Block definitions */
#define YM_OCTAVE_1     1
#define YM_OCTAVE_2     2
#define YM_OCTAVE_3     3
#define YM_OCTAVE_4     4
#define YM_OCTAVE_5     5
#define YM_OCTAVE_6     6
#define YM_OCTAVE_7     7

/* ========================================================================== */
/* FM Patch Structure                                                         */
/* ========================================================================== */

/*
 * Operator parameters structure
 * Each operator has these 7 parameters
 */
struct YM_Operator {
    unsigned char dt_mul;   /* Detune (bits 4-6) + Multiply (bits 0-3) */
    unsigned char tl;       /* Total Level (volume) 0-127 */
    unsigned char rs_ar;    /* Rate Scale (bits 6-7) + Attack Rate (bits 0-4) */
    unsigned char am_d1r;   /* AM enable (bit 7) + Decay 1 Rate (bits 0-4) */
    unsigned char d2r;      /* Decay 2 Rate (bits 0-4) */
    unsigned char d1l_rr;   /* Decay 1 Level (bits 4-7) + Release Rate (bits 0-3) */
    unsigned char ssg_eg;   /* SSG-EG mode */
};

/*
 * Complete FM instrument patch
 */
struct YM_Patch {
    unsigned char algo_fb;  /* Algorithm (bits 0-2) + Feedback (bits 3-5) */
    unsigned char pan_ams_pms; /* Stereo (bits 6-7) + AMS (bits 4-5) + PMS (bits 0-2) */
    struct YM_Operator op[4];  /* Four operators */
};

/* ========================================================================== */
/* Low-Level Functions                                                        */
/* ========================================================================== */

/* Read YM2612 status register */
int ym_read_status(void);

/* Wait for YM2612 to be ready */
void ym_wait(void);

/* Write to YM2612 port 0 (channels 1-3, global) */
void ym_write0(int reg, int val);

/* Write to YM2612 port 1 (channels 4-6) */
void ym_write1(int reg, int val);

/* Write to channel (auto-selects port) */
void ym_write_ch(int ch, int reg, int val);

/* Write to specific operator on channel */
void ym_write_op(int ch, int op, int reg, int val);

/* ========================================================================== */
/* Channel Control Functions                                                  */
/* ========================================================================== */

/* Initialize YM2612 - call at startup */
void ym_init(void);

/* Reset all channels */
void ym_reset(void);

/* Key on - start playing note on channel */
void ym_key_on(int ch);

/* Key off - release note on channel */
void ym_key_off(int ch);

/* Key on specific operators (bitmask: bit0=op1, bit1=op2, etc.) */
void ym_key_on_ops(int ch, int ops);

/* Set frequency (F-number and block/octave) */
void ym_set_freq(int ch, int block, int fnum);

/* Set frequency with fine detune for vibrato/pitch bend */
void ym_set_freq_detune(int ch, int block, int fnum, int detune);

/* Set algorithm and feedback */
void ym_set_algo(int ch, int algo, int feedback);

/* Set stereo panning (use YM_PAN_* constants) */
void ym_set_pan(int ch, int pan);

/* Set channel volume (affects carrier operator TL) */
void ym_set_volume(int ch, int vol);

/* ========================================================================== */
/* LFO and Modulation                                                         */
/* ========================================================================== */

/* Enable/configure global LFO */
void ym_set_lfo(int mode);

/* Set channel LFO sensitivity */
void ym_set_lfo_sensitivity(int ch, int ams, int pms);

/* ========================================================================== */
/* Patch Loading                                                              */
/* ========================================================================== */

/* Load a complete patch to channel */
void ym_load_patch(int ch, struct YM_Patch *patch);

/* Load operator parameters */
void ym_load_operator(int ch, int op, struct YM_Operator *oper);

/* ========================================================================== */
/* DAC / PCM Sample Playback                                                  */
/* ========================================================================== */

/*
 * Enable DAC mode on channel 6
 * This disables FM on channel 6 and allows PCM sample output
 */
void ym_dac_enable(void);

/*
 * Disable DAC mode - restore FM on channel 6
 */
void ym_dac_disable(void);

/*
 * Output a single 8-bit sample to DAC
 * Call this at your desired sample rate (e.g., 8000-22050 Hz)
 */
void ym_dac_write(int sample);

/*
 * Play a PCM sample array
 * data: pointer to 8-bit unsigned sample data
 * length: number of samples
 * rate_div: delay divisor (higher = slower playback)
 *
 * Note: This blocks until sample finishes playing
 */
void ym_dac_play(unsigned char *data, int length, int rate_div);

/* ========================================================================== */
/* Timer Functions                                                            */
/* ========================================================================== */

/* Set Timer A value (10-bit: 0-1023) */
void ym_set_timer_a(int value);

/* Set Timer B value (8-bit: 0-255) */
void ym_set_timer_b(int value);

/* Start timers (flags: bit0=TimerA, bit1=TimerB) */
void ym_start_timers(int flags);

/* Stop timers */
void ym_stop_timers(void);

/* Check if Timer A overflowed */
int ym_timer_a_overflow(void);

/* Check if Timer B overflowed */
int ym_timer_b_overflow(void);

/* ========================================================================== */
/* Built-in Instrument Patches                                                */
/* ========================================================================== */

/* Distorted electric guitar - heavy, crunchy */
void ym_patch_dist_guitar(int ch);

/* Palm-muted guitar - tight chug sound */
void ym_patch_palm_mute(int ch);

/* Clean electric guitar */
void ym_patch_clean_guitar(int ch);

/* Screaming lead guitar - sustain, harmonics */
void ym_patch_lead_guitar(int ch);

/* Synth bass - deep, punchy */
void ym_patch_synth_bass(int ch);

/* Electric bass - finger style */
void ym_patch_elec_bass(int ch);

/* Electric piano */
void ym_patch_epiano(int ch);

/* String ensemble / pad */
void ym_patch_strings(int ch);

/* Brass section */
void ym_patch_brass(int ch);

/* Organ - all carriers */
void ym_patch_organ(int ch);

/* Synth lead - bright, cutting */
void ym_patch_synth_lead(int ch);

/* FM Kick drum - low thump */
void ym_patch_kick(int ch);

/* FM Snare drum - noisy crack */
void ym_patch_snare(int ch);

/* FM Tom - tuned percussion */
void ym_patch_tom(int ch);

/* FM Hi-hat / cymbal */
void ym_patch_hihat(int ch);

/* ========================================================================== */
/* Vibrato / Pitch Effects (software-based)                                   */
/* ========================================================================== */

/*
 * Vibrato state structure - for software vibrato
 * Initialize phase to 0, set depth and speed as desired
 */
struct YM_Vibrato {
    int base_freq;      /* Base F-number */
    int base_block;     /* Base octave/block */
    int depth;          /* Vibrato depth (0-15 recommended) */
    int speed;          /* Vibrato speed (1-8 recommended) */
    int phase;          /* Current phase (0-63) */
};

/*
 * Initialize vibrato state
 */
void ym_vibrato_init(struct YM_Vibrato *vib, int block, int fnum, int depth, int speed);

/*
 * Update vibrato - call this every tick
 * Automatically modulates the channel frequency
 */
void ym_vibrato_update(int ch, struct YM_Vibrato *vib);

/*
 * Pitch bend - slide from current frequency to target
 * Returns 1 when target reached, 0 otherwise
 */
int ym_pitch_bend(int ch, int current_freq, int target_freq, int speed);

/* ========================================================================== */
/* Convenience Macros                                                         */
/* ========================================================================== */

/* Make DT_MUL byte: detune (-3 to +3), multiply (0-15) */
#define YM_DT_MUL(dt, mul)  ((((dt) & 0x07) << 4) | ((mul) & 0x0F))

/* Make RS_AR byte: rate scale (0-3), attack rate (0-31) */
#define YM_RS_AR(rs, ar)    ((((rs) & 0x03) << 6) | ((ar) & 0x1F))

/* Make AM_D1R byte: AM enable (0-1), decay 1 rate (0-31) */
#define YM_AM_D1R(am, d1r)  ((((am) & 0x01) << 7) | ((d1r) & 0x1F))

/* Make D1L_RR byte: decay 1 level (0-15), release rate (0-15) */
#define YM_D1L_RR(d1l, rr)  ((((d1l) & 0x0F) << 4) | ((rr) & 0x0F))

/* Make ALGO_FB byte: algorithm (0-7), feedback (0-7) */
#define YM_ALGO_FB(algo, fb) ((((fb) & 0x07) << 3) | ((algo) & 0x07))

#endif
