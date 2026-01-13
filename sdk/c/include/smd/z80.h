/**
 * @file smd/z80.h
 * @brief Z80 CPU control and sound driver interface
 *
 * The Sega Genesis has a Z80 CPU that can be used for sound processing
 * independently from the main 68000 CPU. This header provides:
 * - Z80 bus control (request/release)
 * - Z80 reset control
 * - Sound driver loading and communication
 *
 * @section z80_arch Architecture
 *
 * The Z80 has 8KB of RAM at 0xA00000-0xA01FFF (68000 view).
 * The 68000 must request the bus before accessing Z80 RAM.
 *
 * @section z80_protocol Command Protocol
 *
 * Commands are sent via a shared memory area in Z80 RAM:
 * - 0xA01F00: Command byte (write last to trigger)
 * - 0xA01F01+: Command data bytes
 */

#ifndef SMD_Z80_H
#define SMD_Z80_H

#include "types.h"

/* ============================================================================
 * Z80 Control Addresses (68000 view)
 * ============================================================================ */

/** @brief Z80 RAM start (8KB) */
#define Z80_RAM         0xA00000

/** @brief Z80 bus request register */
#define Z80_BUS_REQ     0xA11100

/** @brief Z80 reset control register */
#define Z80_RESET       0xA11200

/** @brief Command byte address in Z80 RAM */
#define Z80_CMD_ADDR    0xA01F00

/** @brief Command data address in Z80 RAM */
#define Z80_DATA_ADDR   0xA01F01

/* ============================================================================
 * Z80 Driver Commands
 * ============================================================================ */

/** @brief No operation / idle */
#define Z80_CMD_NOP         0x00

/** @brief Play note: ch, note, octave */
#define Z80_CMD_PLAY_NOTE   0x01

/** @brief Stop note: ch */
#define Z80_CMD_STOP_NOTE   0x02

/** @brief Set patch: ch, patch_id */
#define Z80_CMD_SET_PATCH   0x03

/** @brief Set tempo: tempo_value */
#define Z80_CMD_SET_TEMPO   0x04

/** @brief Start sequence playback */
#define Z80_CMD_PLAY_SEQ    0x10

/** @brief Stop sequence playback */
#define Z80_CMD_STOP_SEQ    0x11

/* ============================================================================
 * Z80 Bus Control Functions
 * ============================================================================ */

/**
 * @brief Request Z80 bus
 *
 * The 68000 must request the bus before accessing Z80 RAM.
 * This function blocks until the bus is granted.
 */
void z80_request_bus(void);

/**
 * @brief Release Z80 bus
 *
 * Release the bus so the Z80 can continue executing.
 */
void z80_release_bus(void);

/**
 * @brief Assert Z80 reset
 *
 * Holds the Z80 in reset state.
 */
void z80_reset_on(void);

/**
 * @brief Release Z80 reset
 *
 * Allows the Z80 to start executing from address 0.
 */
void z80_reset_off(void);

/* ============================================================================
 * Z80 Driver Functions
 * ============================================================================ */

/**
 * @brief Load Z80 driver to Z80 RAM
 *
 * Loads the sound driver binary into Z80 RAM and starts the Z80.
 * Call once at initialization.
 */
void z80_load_driver(void);

/**
 * @brief Initialize Z80 sound driver
 *
 * Loads the driver and performs initial setup.
 */
void z80_init(void);

/**
 * @brief Send command to Z80 driver
 * @param cmd Command byte
 * @param d1 Data byte 1
 * @param d2 Data byte 2
 * @param d3 Data byte 3
 *
 * Writes data bytes first, then command byte to trigger processing.
 */
void z80_send_command(int cmd, int d1, int d2, int d3);

/**
 * @brief Play a note via Z80 driver
 * @param ch YM2612 channel (0-5)
 * @param note Note number (0-11, C=0, B=11)
 * @param octave Octave (0-7)
 */
void z80_play_note(int ch, int note, int octave);

/**
 * @brief Stop a note via Z80 driver
 * @param ch YM2612 channel (0-5)
 */
void z80_stop_note(int ch);

#endif /* SMD_Z80_H */
