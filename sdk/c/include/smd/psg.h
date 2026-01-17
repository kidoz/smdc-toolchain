/*
 * smd/psg.h - PSG (Programmable Sound Generator) functions
 *
 * The Genesis includes a SN76489-compatible PSG for sound effects and music:
 * - 3 square wave tone channels (0-2)
 * - 1 noise channel (3)
 * - 4-bit volume per channel (16 levels)
 * - 10-bit frequency divider for tones
 *
 * Note: smdc uses int for all parameters.
 */

#ifndef SMD_PSG_H
#define SMD_PSG_H

/* ========================================================================== */
/* Hardware Constants                                                         */
/* ========================================================================== */

#define PSG_PORT        ((volatile unsigned char*)0xC00011)
#define PSG_PORT_ADDR   0xC00011
#define PSG_CLOCK       3579545

/* ========================================================================== */
/* Channel Definitions                                                        */
/* ========================================================================== */

#define PSG_CH0         0
#define PSG_CH1         1
#define PSG_CH2         2
#define PSG_CH_NOISE    3

#define PSG_CHANNEL_0       0
#define PSG_CHANNEL_1       1
#define PSG_CHANNEL_2       2
#define PSG_CHANNEL_NOISE   3

/* ========================================================================== */
/* Volume Constants                                                           */
/* ========================================================================== */

/* Volume: 0 = loudest, 15 = silent (2dB per step) */
#define PSG_VOL_MAX     0
#define PSG_VOL_OFF     15
#define PSG_VOL_LOUD    2
#define PSG_VOL_MED     6
#define PSG_VOL_SOFT    10
#define PSG_VOL_QUIET   13

/* ========================================================================== */
/* Noise Channel Modes                                                        */
/* ========================================================================== */

/* Noise type (bit 2) */
#define PSG_NOISE_PERIODIC  0x00    /* Periodic/buzzy noise */
#define PSG_NOISE_WHITE     0x04    /* White noise (hiss) */

/* Noise frequency (bits 0-1) */
#define PSG_NOISE_HI        0x00    /* High frequency (clock/16) */
#define PSG_NOISE_MED       0x01    /* Medium frequency (clock/32) */
#define PSG_NOISE_LO        0x02    /* Low frequency (clock/64) */
#define PSG_NOISE_CH2       0x03    /* Use channel 2's frequency */

/* Common noise combinations */
#define PSG_NOISE_HIHAT     (PSG_NOISE_WHITE | PSG_NOISE_HI)
#define PSG_NOISE_SNARE     (PSG_NOISE_WHITE | PSG_NOISE_MED)
#define PSG_NOISE_KICK      (PSG_NOISE_PERIODIC | PSG_NOISE_LO)
#define PSG_NOISE_CYMBAL    (PSG_NOISE_WHITE | PSG_NOISE_LO)

/* ========================================================================== */
/* Note Divider Table (10-bit values for PSG)                                 */
/* ========================================================================== */

/* Formula: divider = 3579545 / (32 * frequency) */

/* Octave 2 */
#define PSG_C2      1710
#define PSG_CS2     1614
#define PSG_D2      1524
#define PSG_DS2     1438
#define PSG_E2      1357
#define PSG_F2      1281
#define PSG_FS2     1209
#define PSG_G2      1141
#define PSG_GS2     1077
#define PSG_A2      1016
#define PSG_AS2     959
#define PSG_B2      905

/* Octave 3 */
#define PSG_C3      855
#define PSG_CS3     807
#define PSG_D3      762
#define PSG_DS3     719
#define PSG_E3      679
#define PSG_F3      640
#define PSG_FS3     604
#define PSG_G3      570
#define PSG_GS3     538
#define PSG_A3      508
#define PSG_AS3     479
#define PSG_B3      452

/* Octave 4 (Middle C) */
#define PSG_C4      427
#define PSG_CS4     403
#define PSG_D4      381
#define PSG_DS4     359
#define PSG_E4      339
#define PSG_F4      320
#define PSG_FS4     302
#define PSG_G4      285
#define PSG_GS4     269
#define PSG_A4      254
#define PSG_AS4     240
#define PSG_B4      226

/* Octave 5 */
#define PSG_C5      214
#define PSG_CS5     202
#define PSG_D5      190
#define PSG_DS5     180
#define PSG_E5      170
#define PSG_F5      160
#define PSG_FS5     151
#define PSG_G5      143
#define PSG_GS5     135
#define PSG_A5      127
#define PSG_AS5     120
#define PSG_B5      113

/* Octave 6 */
#define PSG_C6      107
#define PSG_CS6     101
#define PSG_D6      95
#define PSG_DS6     90
#define PSG_E6      85
#define PSG_F6      80
#define PSG_FS6     76
#define PSG_G6      71
#define PSG_GS6     67
#define PSG_A6      64
#define PSG_AS6     60
#define PSG_B6      57

/* Octave 7 (high limit) */
#define PSG_C7      53
#define PSG_D7      48
#define PSG_E7      42
#define PSG_G7      36

/* ========================================================================== */
/* Legacy Frequency Definitions (Hz values)                                   */
/* ========================================================================== */

#define PSG_FREQ_LOW        220
#define PSG_FREQ_MED        440
#define PSG_FREQ_HIGH       880
#define PSG_FREQ_BLIP       1760

#define PSG_NOTE_C4_HZ      262
#define PSG_NOTE_D4_HZ      294
#define PSG_NOTE_E4_HZ      330
#define PSG_NOTE_F4_HZ      349
#define PSG_NOTE_G4_HZ      392
#define PSG_NOTE_A4_HZ      440
#define PSG_NOTE_B4_HZ      494
#define PSG_NOTE_C5_HZ      523

/* ========================================================================== */
/* Core Functions                                                             */
/* ========================================================================== */

/*
 * Initialize PSG - silences all channels
 */
void psg_init(void);

/*
 * Write raw byte to PSG port
 */
void psg_write(int value);

/*
 * Set tone channel frequency (10-bit divider value)
 * channel: 0-2
 * divider: 0-1023 (use PSG_* note constants)
 */
void psg_set_tone(int channel, int divider);

/*
 * Set tone from frequency in Hz
 * channel: 0-2
 * freq: frequency in Hz
 */
void psg_set_freq(int channel, int freq);

/*
 * Set channel volume
 * channel: 0-3 (3 = noise)
 * volume: 0 (loudest) to 15 (silent)
 */
void psg_set_volume(int channel, int volume);

/*
 * Configure noise channel
 * mode: combination of PSG_NOISE_* flags
 */
void psg_set_noise(int mode);

/*
 * Stop all channels (set volume to 15)
 */
void psg_stop(void);

/*
 * Stop single channel
 */
void psg_stop_channel(int channel);

/* ========================================================================== */
/* Convenience Functions                                                      */
/* ========================================================================== */

/*
 * Play a simple beep
 * channel: 0-2
 * divider: tone divider (use PSG_* note constants)
 * volume: 0-15
 */
void psg_beep(int channel, int divider, int volume);

/*
 * Play note with envelope (attack-decay)
 * channel: 0-2
 * divider: tone frequency
 * attack_vol: starting volume (0-15)
 * sustain_vol: sustain volume (0-15)
 */
void psg_note_on(int channel, int divider, int attack_vol);

/*
 * Release note (start decay)
 */
void psg_note_off(int channel);

/* ========================================================================== */
/* Drum/Percussion Helpers                                                    */
/* ========================================================================== */

/*
 * Trigger hi-hat sound
 * volume: 0-15
 */
void psg_hihat(int volume);

/*
 * Trigger snare-like noise burst
 * volume: 0-15
 */
void psg_snare_noise(int volume);

/*
 * Trigger kick-like thump (uses ch2 + noise)
 * volume: 0-15
 */
void psg_kick(int volume);

/*
 * Trigger cymbal crash
 * volume: 0-15
 */
void psg_cymbal(int volume);

/* ========================================================================== */
/* Envelope State (for software envelopes)                                    */
/* ========================================================================== */

struct PSG_Envelope {
    int channel;        /* PSG channel (0-3) */
    int volume;         /* Current volume (0-15) */
    int target;         /* Target volume */
    int speed;          /* Envelope speed (frames per step) */
    int counter;        /* Frame counter */
    int active;         /* Is envelope active? */
};

/*
 * Initialize envelope state
 */
void psg_env_init(struct PSG_Envelope *env, int channel);

/*
 * Start attack phase
 */
void psg_env_attack(struct PSG_Envelope *env, int target_vol, int speed);

/*
 * Start decay/release phase
 */
void psg_env_release(struct PSG_Envelope *env, int speed);

/*
 * Update envelope - call every frame
 * Returns 1 if envelope changed, 0 otherwise
 */
int psg_env_update(struct PSG_Envelope *env);

/* ========================================================================== */
/* Utility Macros                                                             */
/* ========================================================================== */

/* Convert Hz to PSG divider */
#define PSG_HZ_TO_DIV(hz)   (PSG_CLOCK / (32 * (hz)))

/* Latch byte format: %1 CC T DDDD */
#define PSG_LATCH(ch, type, data) \
    (0x80 | (((ch) & 3) << 5) | (((type) & 1) << 4) | ((data) & 0x0F))

/* Data byte format: %0 -DDDDDD */
#define PSG_DATA(data)  ((data) & 0x3F)

#endif
