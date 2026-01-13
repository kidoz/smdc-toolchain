/*
 * smd/psg.h - PSG (Programmable Sound Generator) functions
 *
 * The Genesis includes a TI SN76489 PSG for simple sound effects.
 * It has 3 square wave channels and 1 noise channel.
 *
 * Note: smdc uses int for all parameters.
 */

#ifndef SMD_PSG_H
#define SMD_PSG_H

/* PSG Hardware */
#define PSG_PORT_ADDR   0xC00011
#define PSG_CLOCK       3579545

/* PSG Channel Constants */
#define PSG_CHANNEL_0       0
#define PSG_CHANNEL_1       1
#define PSG_CHANNEL_2       2
#define PSG_CHANNEL_NOISE   3

#define PSG_VOL_MAX     0
#define PSG_VOL_OFF     15

/* Noise Types */
#define PSG_NOISE_PERIODIC  0
#define PSG_NOISE_WHITE     4
#define PSG_NOISE_HI        0
#define PSG_NOISE_MED       1
#define PSG_NOISE_LO        2
#define PSG_NOISE_CH2       3

/* Preset Sound Frequencies (Hz) */
#define PSG_FREQ_LOW        220
#define PSG_FREQ_MED        440
#define PSG_FREQ_HIGH       880
#define PSG_FREQ_BLIP       1760

/* Musical note frequencies */
#define PSG_NOTE_C4         262
#define PSG_NOTE_D4         294
#define PSG_NOTE_E4         330
#define PSG_NOTE_F4         349
#define PSG_NOTE_G4         392
#define PSG_NOTE_A4         440
#define PSG_NOTE_B4         494
#define PSG_NOTE_C5         523

/* Functions */
void psg_init(void);
void psg_set_tone(int channel, int freq);
void psg_set_tone_raw(int channel, int value);
void psg_set_volume(int channel, int volume);
void psg_set_noise(int mode);
void psg_stop(void);
void psg_beep(int channel, int freq, int volume);

#endif
