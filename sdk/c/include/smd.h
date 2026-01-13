/**
 * @file smd.h
 * @brief Sega Mega Drive / Genesis SDK
 * @version 1.0
 * @author SMD Compiler Project
 *
 * @mainpage SMD SDK Documentation
 *
 * @section intro Introduction
 *
 * The SMD SDK provides a hardware abstraction layer for developing games
 * and applications for the Sega Mega Drive (Genesis) console.
 *
 * @section features Features
 *
 * - VDP (Video Display Processor) control
 *   - Tile and palette management
 *   - Plane scrolling
 *   - Sprite handling (up to 80 sprites)
 * - PSG (Programmable Sound Generator)
 *   - 3 square wave channels
 *   - 1 noise channel
 * - YM2612 FM synth
 *   - 6 FM channels with instrument patches
 * - Z80 sound driver interface
 * - Controller input (3-button and 6-button)
 * - Fixed-point math utilities
 *
 * @section quickstart Quick Start
 *
 * @code
 * #include <smd.h>
 *
 * void main() {
 *     // Initialize hardware
 *     vdp_init();
 *     input_init();
 *     sprite_init();
 *
 *     // Set up palette
 *     vdp_set_color(0, COLOR_BLACK);
 *     vdp_set_color(1, COLOR_WHITE);
 *
 *     // Main game loop
 *     while (1) {
 *         // Wait for vblank
 *         vdp_vsync();
 *
 *         // Read input
 *         u16 buttons = input_read(0);
 *
 *         // Update game state
 *         if (buttons & INPUT_UP) {
 *             player_y -= 2;
 *         }
 *         if (buttons & INPUT_DOWN) {
 *             player_y += 2;
 *         }
 *
 *         // Update sprite
 *         sprite_set(0, player_x, player_y, SPRITE_SIZE_2x2,
 *                    SPRITE_ATTR(1, 0, 0, 0, 0));
 *     }
 * }
 * @endcode
 *
 * @section compile Compilation
 *
 * Compile with the SMD compiler:
 * @code
 * smdc game.c -o game.bin -t rom
 * @endcode
 *
 * @section hardware Hardware Reference
 *
 * | Component | Description |
 * |-----------|-------------|
 * | CPU | Motorola 68000 @ 7.67 MHz |
 * | VDP | Custom Yamaha YM7101 |
 * | Resolution | 320x224 (H40) or 256x224 (H32) |
 * | Colors | 64 on-screen from 512 |
 * | Sprites | 80 max, 20 per scanline |
 * | Sound | Yamaha YM2612 + TI SN76489 |
 *
 * @section files File Overview
 *
 * - smd/types.h - Type definitions (u8, u16, s32, fix16, etc.)
 * - smd/vdp.h - Video Display Processor functions
 * - smd/sprite.h - Sprite management
 * - smd/input.h - Controller input handling
 * - smd/psg.h - PSG sound generation
 * - smd/ym2612.h - YM2612 FM synth
 * - smd/z80.h - Z80 sound driver interface
 */

#ifndef SMD_H
#define SMD_H

/* Include all SDK headers */
#include "smd/types.h"
#include "smd/vdp.h"
#include "smd/sprite.h"
#include "smd/input.h"
#include "smd/psg.h"
#include "smd/ym2612.h"
#include "smd/z80.h"

/* ============================================================================
 * SDK Version
 * ============================================================================ */

/** @brief SDK major version */
#define SMD_VERSION_MAJOR   1
/** @brief SDK minor version */
#define SMD_VERSION_MINOR   0
/** @brief SDK patch version */
#define SMD_VERSION_PATCH   0

/** @brief SDK version as string */
#define SMD_VERSION_STRING  "1.0.0"

/* ============================================================================
 * System Functions
 * ============================================================================ */

/**
 * @brief Initialize all SDK subsystems
 *
 * Convenience function that initializes VDP, sprites, and input.
 * Equivalent to calling vdp_init(), sprite_init(), input_init().
 */
void smd_init(void);

/**
 * @brief Disable interrupts
 * @return Previous interrupt mask (pass to smd_enable_ints)
 *
 * Use when performing time-critical operations.
 */
u16 smd_disable_ints(void);

/**
 * @brief Restore interrupt state
 * @param mask Previous interrupt mask from smd_disable_ints()
 */
void smd_enable_ints(u16 mask);

/**
 * @brief Get current frame counter
 * @return Number of vblanks since startup
 *
 * Incremented each vblank. Useful for timing and animation.
 */
u32 smd_get_frame(void);

/**
 * @brief Delay for specified number of frames
 * @param frames Number of frames to wait
 */
void smd_delay(u16 frames);

/**
 * @brief Check if running on PAL system
 * @return TRUE if PAL (50Hz), FALSE if NTSC (60Hz)
 */
bool smd_is_pal(void);

#endif /* SMD_H */
