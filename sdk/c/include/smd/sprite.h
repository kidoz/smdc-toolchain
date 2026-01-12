/**
 * @file smd/sprite.h
 * @brief Sprite management for Sega Mega Drive
 * @version 1.0
 *
 * The Genesis VDP supports up to 80 hardware sprites.
 * Each sprite can be 1-4 tiles wide and 1-4 tiles tall.
 * Sprites are stored in the Sprite Attribute Table (SAT) in VRAM.
 */

#ifndef SMD_SPRITE_H
#define SMD_SPRITE_H

#include "types.h"

/* ============================================================================
 * Sprite Size Constants
 * ============================================================================
 *
 * Size byte format: %----WWHH
 *   HH = Height in tiles - 1 (0-3)
 *   WW = Width in tiles - 1 (0-3)
 */

/** @brief Sprite size: 8x8 pixels (1x1 tiles) */
#define SPRITE_SIZE_1x1     0x00
/** @brief Sprite size: 8x16 pixels (1x2 tiles) */
#define SPRITE_SIZE_1x2     0x01
/** @brief Sprite size: 8x24 pixels (1x3 tiles) */
#define SPRITE_SIZE_1x3     0x02
/** @brief Sprite size: 8x32 pixels (1x4 tiles) */
#define SPRITE_SIZE_1x4     0x03

/** @brief Sprite size: 16x8 pixels (2x1 tiles) */
#define SPRITE_SIZE_2x1     0x04
/** @brief Sprite size: 16x16 pixels (2x2 tiles) */
#define SPRITE_SIZE_2x2     0x05
/** @brief Sprite size: 16x24 pixels (2x3 tiles) */
#define SPRITE_SIZE_2x3     0x06
/** @brief Sprite size: 16x32 pixels (2x4 tiles) */
#define SPRITE_SIZE_2x4     0x07

/** @brief Sprite size: 24x8 pixels (3x1 tiles) */
#define SPRITE_SIZE_3x1     0x08
/** @brief Sprite size: 24x16 pixels (3x2 tiles) */
#define SPRITE_SIZE_3x2     0x09
/** @brief Sprite size: 24x24 pixels (3x3 tiles) */
#define SPRITE_SIZE_3x3     0x0A
/** @brief Sprite size: 24x32 pixels (3x4 tiles) */
#define SPRITE_SIZE_3x4     0x0B

/** @brief Sprite size: 32x8 pixels (4x1 tiles) */
#define SPRITE_SIZE_4x1     0x0C
/** @brief Sprite size: 32x16 pixels (4x2 tiles) */
#define SPRITE_SIZE_4x2     0x0D
/** @brief Sprite size: 32x24 pixels (4x3 tiles) */
#define SPRITE_SIZE_4x3     0x0E
/** @brief Sprite size: 32x32 pixels (4x4 tiles) */
#define SPRITE_SIZE_4x4     0x0F

/* ============================================================================
 * Sprite Attribute Flags
 * ============================================================================ */

/** @brief Sprite priority (draw in front of planes) */
#define SPRITE_PRIORITY     0x8000
/** @brief Sprite palette 0 */
#define SPRITE_PAL0         0x0000
/** @brief Sprite palette 1 */
#define SPRITE_PAL1         0x2000
/** @brief Sprite palette 2 */
#define SPRITE_PAL2         0x4000
/** @brief Sprite palette 3 */
#define SPRITE_PAL3         0x6000
/** @brief Sprite vertical flip */
#define SPRITE_VFLIP        0x1000
/** @brief Sprite horizontal flip */
#define SPRITE_HFLIP        0x0800

/**
 * @brief Create sprite attribute word
 * @param tile Base tile index (0-2047)
 * @param pal Palette number (0-3)
 * @param priority Priority flag (0 or 1)
 * @param hflip Horizontal flip (0 or 1)
 * @param vflip Vertical flip (0 or 1)
 * @return Sprite attribute word
 */
#define SPRITE_ATTR(tile, pal, priority, hflip, vflip) \
    ((tile) | ((pal) << 13) | ((priority) << 15) | ((hflip) << 11) | ((vflip) << 12))

/* ============================================================================
 * Sprite Structure
 * ============================================================================ */

/**
 * @brief Sprite definition structure
 *
 * This structure can be used to manage sprite state in your game.
 * Use sprite_set() or sprite_update() to apply to hardware.
 */
typedef struct {
    s16 x;          /**< X position on screen */
    s16 y;          /**< Y position on screen */
    u8  size;       /**< Sprite size (use SPRITE_SIZE_* constants) */
    u16 tile;       /**< Base tile index */
    u16 attr;       /**< Attribute flags (palette, flip, priority) */
} Sprite;

/* ============================================================================
 * Functions
 * ============================================================================ */

/**
 * @brief Initialize sprite system
 *
 * Clears all 80 sprite entries in the sprite attribute table.
 * Call this once during game initialization.
 */
void sprite_init(void);

/**
 * @brief Set sprite attributes directly
 * @param index Sprite index (0-79)
 * @param x X position on screen
 * @param y Y position on screen
 * @param size Sprite size (use SPRITE_SIZE_* constants)
 * @param attr Tile index and attributes (use SPRITE_ATTR macro)
 *
 * @code
 * // Create a 1x4 tile sprite at position (100, 50)
 * sprite_set(0, 100, 50, SPRITE_SIZE_1x4, SPRITE_ATTR(1, 0, 0, 0, 0));
 * @endcode
 */
void sprite_set(u8 index, s16 x, s16 y, u8 size, u16 attr);

/**
 * @brief Update sprite from Sprite structure
 * @param index Sprite index (0-79)
 * @param spr Pointer to Sprite structure
 */
void sprite_update(u8 index, const Sprite* spr);

/**
 * @brief Set sprite position only
 * @param index Sprite index (0-79)
 * @param x New X position
 * @param y New Y position
 *
 * Faster than sprite_set() when only position changes.
 */
void sprite_set_pos(u8 index, s16 x, s16 y);

/**
 * @brief Hide a sprite (move off-screen)
 * @param index Sprite index (0-79)
 */
void sprite_hide(u8 index);

/**
 * @brief Clear a sprite entry
 * @param index Sprite index (0-79)
 *
 * Sets all sprite attributes to zero.
 */
void sprite_clear(u8 index);

/**
 * @brief Clear all sprites
 *
 * Hides all 80 sprites by clearing the sprite attribute table.
 */
void sprite_clear_all(void);

/**
 * @brief Set sprite link (for sprite chain)
 * @param index Sprite index (0-79)
 * @param next Next sprite index (0 = end of list)
 *
 * Sprites are rendered in link order. By default, sprites are
 * linked sequentially (0->1->2->...). Use this to change order
 * or create multiple sprite lists.
 */
void sprite_set_link(u8 index, u8 next);

/**
 * @brief Get width in pixels for a sprite size
 * @param size Sprite size constant
 * @return Width in pixels
 */
u8 sprite_get_width(u8 size);

/**
 * @brief Get height in pixels for a sprite size
 * @param size Sprite size constant
 * @return Height in pixels
 */
u8 sprite_get_height(u8 size);

#endif /* SMD_SPRITE_H */
