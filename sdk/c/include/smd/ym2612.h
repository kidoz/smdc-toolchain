/*
 * smd/ym2612.h - YM2612 FM Synthesizer interface
 *
 * The YM2612 is the FM synthesis chip in the Sega Genesis:
 * - 6 FM channels (0-5)
 * - 4 operators per channel
 * - 8 FM algorithms
 * - Stereo output (L/R per channel)
 * - DAC mode on channel 6
 *
 * Note: smdc uses int for all parameters.
 */

#ifndef SMD_YM2612_H
#define SMD_YM2612_H

/* Hardware Addresses */
#define YM2612_ADDR0    0xA04000
#define YM_ADDR_PORT0   0xA04000
#define YM2612_DATA0    0xA04001
#define YM_DATA_PORT0   0xA04001
#define YM2612_ADDR1    0xA04002
#define YM_ADDR_PORT1   0xA04002
#define YM2612_DATA1    0xA04003
#define YM_DATA_PORT1   0xA04003

/* Status Flags */
#define YM2612_STATUS_BUSY  0x80
#define YM2612_WAIT_LIMIT   0x0400

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

/* Per-Operator Registers (Op offsets: 0=0, 1=8, 2=4, 3=12) */
#define YM_REG_OP_DT_MUL    0x30
#define YM_REG_OP_TL        0x40
#define YM_REG_OP_RS_AR     0x50
#define YM_REG_OP_AM_D1R    0x60
#define YM_REG_OP_D2R       0x70
#define YM_REG_OP_D1L_RR    0x80
#define YM_REG_OP_SSG_EG    0x90

/* Algorithm Definitions */
#define YM_ALGO_SERIAL      0   /* Serial modulation (M1->M2->M3->C) */
#define YM_ALGO_PIANO       4   /* Three parallel carriers with modulator */
#define YM_ALGO_DISTORTION  5   /* Parallel carriers with feedback modulator */
#define YM_ALGO_ORGAN       7   /* All operators in parallel (additive) */

/* Note Frequency Table (F-number values for block 4) */
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
#define YM_NOTE_AS  1145
#define YM_NOTE_B   1214

/* Low-Level Functions */
int ym2612_read_status(void);
void ym2612_wait_ready(void);
void ym_write_port0(int reg, int val);
void ym_write_port1(int reg, int val);

/* Channel Functions */
void ym_write_op(int ch, int op, int reg, int val);
void ym_key_on(int ch);
void ym_key_off(int ch);
void ym_set_freq(int ch, int block, int fnum);
void ym_set_algorithm(int ch, int algo, int feedback);
void ym_set_stereo(int ch, int left, int right);
void ym_init(void);

/* Instrument Patches */
void ym_patch_dist_guitar(int ch);
void ym_patch_synth_bass(int ch);
void ym_patch_epiano(int ch);
void ym_patch_strings(int ch);
void ym_patch_brass(int ch);
void ym_patch_organ(int ch);
void ym_patch_synth_lead(int ch);

#endif
