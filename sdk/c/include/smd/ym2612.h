/**
 * @file smd/ym2612.h
 * @brief YM2612 FM Synthesizer interface
 * @version 2.0
 *
 * Complete YM2612 FM synthesis support for Sega Mega Drive including:
 * - Low-level register access
 * - Operator-level control
 * - Frequency and key management
 * - Pre-defined instrument patches
 *
 * @section ym_overview Overview
 *
 * The YM2612 is the FM synthesis chip in the Sega Genesis:
 * - 6 FM channels (0-5)
 * - 4 operators per channel
 * - 8 FM algorithms
 * - Stereo output (L/R per channel)
 * - LFO for vibrato/tremolo
 * - DAC mode on channel 6
 *
 * @section ym_ports Hardware Ports
 *
 * Port 0 (0xA04000/0xA04001): Channels 0-2 + global registers
 * Port 1 (0xA04002/0xA04003): Channels 3-5
 *
 * @note smdc compiler does not support #include - copy implementations
 *       from examples or use inline in your code.
 */

#ifndef SMD_YM2612_H
#define SMD_YM2612_H

#include "types.h"

/* ============================================================================
 * Hardware Addresses
 * ============================================================================ */

/** @brief YM2612 address port 0 (channels 0-2) */
#define YM2612_ADDR0    0xA04000
#define YM_ADDR_PORT0   0xA04000

/** @brief YM2612 data port 0 */
#define YM2612_DATA0    0xA04001
#define YM_DATA_PORT0   0xA04001

/** @brief YM2612 address port 1 (channels 3-5) */
#define YM2612_ADDR1    0xA04002
#define YM_ADDR_PORT1   0xA04002

/** @brief YM2612 data port 1 */
#define YM2612_DATA1    0xA04003
#define YM_DATA_PORT1   0xA04003

/* ============================================================================
 * Status Flags
 * ============================================================================ */

/** @brief YM2612 busy flag */
#define YM2612_STATUS_BUSY  0x80

/** @brief Maximum wait cycles for busy flag */
#define YM2612_WAIT_LIMIT   0x0400

/* ============================================================================
 * Global Registers (Port 0 only)
 * ============================================================================ */

#define YM_REG_LFO          0x22    /* LFO enable and frequency */
#define YM_REG_TIMER_A_HI   0x24    /* Timer A MSB */
#define YM_REG_TIMER_A_LO   0x25    /* Timer A LSB */
#define YM_REG_TIMER_B      0x26    /* Timer B */
#define YM_REG_TIMER_CTRL   0x27    /* Timer control and Ch3 mode */
#define YM_REG_KEY_ONOFF    0x28    /* Key on/off control */
#define YM_REG_DAC          0x2A    /* DAC data */
#define YM_REG_DAC_EN       0x2B    /* DAC enable */

/* ============================================================================
 * Per-Channel Registers (add channel offset 0-2)
 * ============================================================================ */

#define YM_REG_FREQ_LO      0xA0    /* Frequency LSB */
#define YM_REG_FREQ_HI      0xA4    /* Frequency MSB + Block */
#define YM_REG_ALGO_FB      0xB0    /* Algorithm + Feedback */
#define YM_REG_STEREO_LFO   0xB4    /* Stereo + LFO sensitivity */

/* ============================================================================
 * Per-Operator Registers
 * Operator offsets: Op1=0, Op2=8, Op3=4, Op4=12
 * ============================================================================ */

#define YM_REG_OP_DT_MUL    0x30    /* Detune + Multiply */
#define YM_REG_OP_TL        0x40    /* Total Level (volume) */
#define YM_REG_OP_RS_AR     0x50    /* Rate Scale + Attack Rate */
#define YM_REG_OP_AM_D1R    0x60    /* AM enable + Decay 1 Rate */
#define YM_REG_OP_D2R       0x70    /* Decay 2 Rate */
#define YM_REG_OP_D1L_RR    0x80    /* Sustain Level + Release Rate */
#define YM_REG_OP_SSG_EG    0x90    /* SSG-EG envelope mode */

/* ============================================================================
 * Algorithm Definitions (0-7)
 * ============================================================================ */

/**
 * Algorithm 0: Serial modulation (M1->M2->M3->C)
 * Good for: Warm bass, bells
 */
#define YM_ALGO_SERIAL      0

/**
 * Algorithm 4: Three parallel carriers with modulator
 * Good for: Piano, organ
 */
#define YM_ALGO_PIANO       4

/**
 * Algorithm 5: Parallel carriers with feedback modulator
 * Good for: Distorted guitar, heavy tones
 */
#define YM_ALGO_DISTORTION  5

/**
 * Algorithm 7: All operators in parallel (additive)
 * Good for: Organ, rich pads
 */
#define YM_ALGO_ORGAN       7

/* ============================================================================
 * Note Frequency Table (F-number values for block 4)
 * ============================================================================ */

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

/* ============================================================================
 * Low-Level Functions
 * ============================================================================ */

/**
 * @brief Read YM2612 status register
 * @return Status byte (bit 7 = busy)
 */
u8 ym2612_read_status(void);

/**
 * @brief Wait until YM2612 is ready
 */
void ym2612_wait_ready(void);

/**
 * @brief Write to YM2612 port 0 (channels 0-2, global regs)
 * @param reg Register address
 * @param val Value to write
 */
void ym_write_port0(int reg, int val);

/**
 * @brief Write to YM2612 port 1 (channels 3-5)
 * @param reg Register address
 * @param val Value to write
 */
void ym_write_port1(int reg, int val);

/* ============================================================================
 * Channel Functions
 * ============================================================================ */

/**
 * @brief Write to operator register for any channel
 * @param ch Channel (0-5)
 * @param op Operator (0-3)
 * @param reg Base register address (0x30, 0x40, etc.)
 * @param val Value to write
 *
 * Handles operator offset mapping: op 0,1,2,3 -> offset 0,8,4,12
 */
void ym_write_op(int ch, int op, int reg, int val);

/**
 * @brief Key on (start note) for channel
 * @param ch Channel (0-5)
 */
void ym_key_on(int ch);

/**
 * @brief Key off (stop note) for channel
 * @param ch Channel (0-5)
 */
void ym_key_off(int ch);

/**
 * @brief Set frequency for channel
 * @param ch Channel (0-5)
 * @param block Octave block (0-7)
 * @param fnum Frequency number (0-2047), use YM_NOTE_* constants
 */
void ym_set_freq(int ch, int block, int fnum);

/**
 * @brief Set algorithm and feedback for channel
 * @param ch Channel (0-5)
 * @param algo Algorithm (0-7), use YM_ALGO_* constants
 * @param feedback Feedback level (0-7)
 */
void ym_set_algorithm(int ch, int algo, int feedback);

/**
 * @brief Set stereo output for channel
 * @param ch Channel (0-5)
 * @param left Left speaker enable (0 or 1)
 * @param right Right speaker enable (0 or 1)
 */
void ym_set_stereo(int ch, int left, int right);

/**
 * @brief Initialize YM2612 to default state
 *
 * Disables LFO, timers, DAC, and keys off all channels.
 */
void ym_init(void);

/* ============================================================================
 * Instrument Patch Functions
 * ============================================================================ */

/**
 * @brief Load distorted guitar patch
 * @param ch Channel (0-5)
 *
 * Heavy metal guitar with maximum feedback.
 * Algorithm 5, Feedback 7.
 */
void ym_patch_dist_guitar(int ch);

/**
 * @brief Load synth bass patch
 * @param ch Channel (0-5)
 *
 * Deep bass with punch.
 * Algorithm 0, Feedback 5.
 */
void ym_patch_synth_bass(int ch);

/**
 * @brief Load electric piano patch
 * @param ch Channel (0-5)
 */
void ym_patch_epiano(int ch);

/**
 * @brief Load string pad patch
 * @param ch Channel (0-5)
 */
void ym_patch_strings(int ch);

/**
 * @brief Load brass patch
 * @param ch Channel (0-5)
 */
void ym_patch_brass(int ch);

/**
 * @brief Load organ patch
 * @param ch Channel (0-5)
 *
 * All operators in parallel for rich harmonics.
 * Algorithm 7.
 */
void ym_patch_organ(int ch);

/**
 * @brief Load synth lead patch
 * @param ch Channel (0-5)
 */
void ym_patch_synth_lead(int ch);

#endif /* SMD_YM2612_H */
