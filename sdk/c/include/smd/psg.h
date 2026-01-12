/**
 * @file psg.h
 * @brief PSG (Programmable Sound Generator) functions
 *
 * The Genesis includes a TI SN76489 PSG for simple sound effects.
 * It has 3 square wave channels and 1 noise channel.
 *
 * @section psg_usage Usage
 *
 * @code
 * // Play a beep on channel 0
 * psg_set_tone(0, 440);      // 440 Hz (A4 note)
 * psg_set_volume(0, 0);      // Max volume
 *
 * // Wait, then silence
 * smd_delay(10);
 * psg_set_volume(0, 15);     // Silent
 * @endcode
 *
 * @section psg_notes Musical Notes
 *
 * Common frequencies (Hz):
 * - C4: 262, D4: 294, E4: 330, F4: 349
 * - G4: 392, A4: 440, B4: 494, C5: 523
 */

#ifndef SMD_PSG_H
#define SMD_PSG_H

#include "types.h"

/* ============================================================================
 * PSG Hardware
 * ============================================================================ */

/** @brief PSG port address */
#define PSG_PORT        ((volatile u8*)0xC00011)

/** @brief PSG clock frequency (Hz) - NTSC */
#define PSG_CLOCK       3579545

/* ============================================================================
 * PSG Channel Constants
 * ============================================================================ */

/** @brief Tone channel 0 */
#define PSG_CHANNEL_0   0
/** @brief Tone channel 1 */
#define PSG_CHANNEL_1   1
/** @brief Tone channel 2 */
#define PSG_CHANNEL_2   2
/** @brief Noise channel */
#define PSG_CHANNEL_NOISE 3

/** @brief Maximum volume (loudest) */
#define PSG_VOL_MAX     0
/** @brief Minimum volume (silent) */
#define PSG_VOL_OFF     15

/* ============================================================================
 * Noise Types
 * ============================================================================ */

/** @brief Periodic noise (buzzing) */
#define PSG_NOISE_PERIODIC  0
/** @brief White noise (static) */
#define PSG_NOISE_WHITE     4

/** @brief Noise shift rate: high frequency */
#define PSG_NOISE_HI        0
/** @brief Noise shift rate: medium frequency */
#define PSG_NOISE_MED       1
/** @brief Noise shift rate: low frequency */
#define PSG_NOISE_LO        2
/** @brief Noise uses channel 2 frequency */
#define PSG_NOISE_CH2       3

/* ============================================================================
 * Preset Sound Frequencies (Hz)
 * ============================================================================ */

/** @brief Low beep frequency */
#define PSG_FREQ_LOW        220
/** @brief Medium beep frequency */
#define PSG_FREQ_MED        440
/** @brief High beep frequency */
#define PSG_FREQ_HIGH       880
/** @brief Blip frequency */
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

/* ============================================================================
 * PSG Functions
 * ============================================================================ */

/**
 * @brief Initialize PSG (silence all channels)
 *
 * Call once at startup to ensure all channels are silent.
 */
void psg_init(void);

/**
 * @brief Set tone frequency for a channel
 * @param channel Channel number (0-2)
 * @param freq Frequency in Hz
 *
 * Converts frequency to PSG divider value and writes to hardware.
 * Valid range is approximately 109 Hz to 125 kHz.
 *
 * @code
 * psg_set_tone(0, 440);  // A4 note on channel 0
 * @endcode
 */
void psg_set_tone(u8 channel, u16 freq);

/**
 * @brief Set raw tone value for a channel
 * @param channel Channel number (0-2)
 * @param value 10-bit divider value (0-1023)
 *
 * Lower values = higher pitch. Value of 0 produces no sound.
 * Frequency = PSG_CLOCK / (32 * value)
 */
void psg_set_tone_raw(u8 channel, u16 value);

/**
 * @brief Set volume for a channel
 * @param channel Channel number (0-3, includes noise)
 * @param volume Volume level (0=max, 15=silent)
 *
 * @code
 * psg_set_volume(0, 0);   // Max volume
 * psg_set_volume(0, 15);  // Silent
 * psg_set_volume(0, 8);   // Half volume
 * @endcode
 */
void psg_set_volume(u8 channel, u8 volume);

/**
 * @brief Set noise channel mode
 * @param mode Noise mode (combine type and rate)
 *
 * Mode is a combination of noise type and shift rate:
 * - Type: PSG_NOISE_PERIODIC or PSG_NOISE_WHITE
 * - Rate: PSG_NOISE_HI, PSG_NOISE_MED, PSG_NOISE_LO, or PSG_NOISE_CH2
 *
 * @code
 * // White noise, medium frequency
 * psg_set_noise(PSG_NOISE_WHITE | PSG_NOISE_MED);
 *
 * // Periodic noise using channel 2 frequency
 * psg_set_noise(PSG_NOISE_PERIODIC | PSG_NOISE_CH2);
 * @endcode
 */
void psg_set_noise(u8 mode);

/**
 * @brief Silence all PSG channels
 *
 * Sets all 4 channels to volume 15 (silent).
 */
void psg_stop(void);

/**
 * @brief Play a simple beep
 * @param channel Channel to use (0-2)
 * @param freq Frequency in Hz
 * @param volume Volume (0=max, 15=silent)
 *
 * Convenience function to start a tone. Call psg_stop() or
 * psg_set_volume(channel, 15) to silence it.
 *
 * @code
 * psg_beep(0, 440, 0);  // Play A4 at max volume
 * @endcode
 */
void psg_beep(u8 channel, u16 freq, u8 volume);

#endif /* SMD_PSG_H */
