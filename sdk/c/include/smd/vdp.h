/**
 * @file smd/vdp.h
 * @brief Video Display Processor (VDP) interface
 * @version 1.0
 *
 * The VDP is responsible for all graphics rendering on the Genesis.
 * It supports two scrolling background planes, up to 80 sprites,
 * and a 64-color palette (from 512 possible colors).
 */

#ifndef SMD_VDP_H
#define SMD_VDP_H

#include "types.h"

/* ============================================================================
 * Hardware Addresses
 * ============================================================================ */

/** @brief VDP data port - write tile/palette data here */
#define VDP_DATA        ((volatile u16*)0xC00000)

/** @brief VDP control port - write commands and read status */
#define VDP_CTRL        ((volatile u16*)0xC00004)

/** @brief VDP HV counter - horizontal/vertical position */
#define VDP_HVCOUNTER   ((volatile u16*)0xC00008)

/* ============================================================================
 * VDP Status Flags
 * ============================================================================ */

/** @brief VDP is in vertical blank period */
#define VDP_STATUS_VBLANK   0x0008
/** @brief VDP is in horizontal blank period */
#define VDP_STATUS_HBLANK   0x0004
/** @brief DMA is in progress */
#define VDP_STATUS_DMA      0x0002

/* ============================================================================
 * Default VRAM Addresses
 * ============================================================================ */

/** @brief Default Plane A nametable address */
#define VRAM_PLANE_A    0xC000
/** @brief Default Plane B nametable address */
#define VRAM_PLANE_B    0xE000
/** @brief Default Window plane address */
#define VRAM_WINDOW     0xD000
/** @brief Default Sprite attribute table address */
#define VRAM_SPRITES    0xF000
/** @brief Default HScroll table address */
#define VRAM_HSCROLL    0xFC00

/* ============================================================================
 * Color Definitions (BGR format: 0x0BGR)
 * ============================================================================ */

/** @brief Black color (0, 0, 0) */
#define COLOR_BLACK     0x0000
/** @brief White color (255, 255, 255) */
#define COLOR_WHITE     0x0EEE
/** @brief Red color (255, 0, 0) */
#define COLOR_RED       0x000E
/** @brief Green color (0, 255, 0) */
#define COLOR_GREEN     0x00E0
/** @brief Blue color (0, 0, 255) */
#define COLOR_BLUE      0x0E00
/** @brief Yellow color (255, 255, 0) */
#define COLOR_YELLOW    0x00EE
/** @brief Cyan color (0, 255, 255) */
#define COLOR_CYAN      0x0EE0
/** @brief Magenta color (255, 0, 255) */
#define COLOR_MAGENTA   0x0E0E

/**
 * @brief Create a color from RGB components
 * @param r Red component (0-14)
 * @param g Green component (0-14)
 * @param b Blue component (0-14)
 * @return Color value in BGR format
 */
#define RGB(r, g, b)    (((b) << 8) | ((g) << 4) | (r))

/* ============================================================================
 * Tile Attributes
 * ============================================================================ */

/** @brief Tile priority flag (draw in front of sprites) */
#define TILE_PRIORITY   0x8000
/** @brief Tile palette 0 */
#define TILE_PAL0       0x0000
/** @brief Tile palette 1 */
#define TILE_PAL1       0x2000
/** @brief Tile palette 2 */
#define TILE_PAL2       0x4000
/** @brief Tile palette 3 */
#define TILE_PAL3       0x6000
/** @brief Tile vertical flip */
#define TILE_VFLIP      0x1000
/** @brief Tile horizontal flip */
#define TILE_HFLIP      0x0800

/**
 * @brief Create tile attribute word
 * @param index Tile index (0-2047)
 * @param pal Palette (0-3)
 * @param priority Priority flag (0 or 1)
 * @param hflip Horizontal flip (0 or 1)
 * @param vflip Vertical flip (0 or 1)
 * @return Tile attribute word
 */
#define TILE_ATTR(index, pal, priority, hflip, vflip) \
    ((index) | ((pal) << 13) | ((priority) << 15) | ((hflip) << 11) | ((vflip) << 12))

/* ============================================================================
 * Functions
 * ============================================================================ */

/**
 * @brief Initialize VDP with default settings
 *
 * Configures the VDP for 320x224 (H40) display mode with:
 * - Display enabled
 * - Mode 5 (Genesis mode)
 * - Default plane addresses
 * - Auto-increment of 2
 *
 * @code
 * vdp_init();
 * @endcode
 */
void vdp_init(void);

/**
 * @brief Set a VDP register value
 * @param reg Register number (0-23)
 * @param value Value to write (0-255)
 *
 * @code
 * // Enable display with Mode 5
 * vdp_set_reg(1, 0x44);
 * @endcode
 */
void vdp_set_reg(u8 reg, u8 value);

/**
 * @brief Wait for vertical blank period
 *
 * Blocks until the VDP enters vertical blank. Use this to
 * synchronize game updates with display refresh (60Hz NTSC, 50Hz PAL).
 *
 * @code
 * while (1) {
 *     update_game();
 *     vdp_vsync();
 *     render();
 * }
 * @endcode
 */
void vdp_vsync(void);

/**
 * @brief Read VDP status register
 * @return Status flags (see VDP_STATUS_* constants)
 */
u16 vdp_get_status(void);

/**
 * @brief Set VRAM write address
 * @param addr VRAM address (0x0000-0xFFFF)
 *
 * After calling this, write data to VDP_DATA to fill VRAM.
 */
void vdp_set_write_addr(u16 addr);

/**
 * @brief Set CRAM (palette) write address
 * @param index Color index (0-63)
 *
 * After calling this, write colors to VDP_DATA.
 */
void vdp_set_cram_addr(u8 index);

/**
 * @brief Set a single palette color
 * @param index Color index (0-63)
 * @param color Color value in BGR format (use RGB() or COLOR_* macros)
 *
 * @code
 * vdp_set_color(0, COLOR_BLACK);  // Background
 * vdp_set_color(1, COLOR_WHITE);  // Foreground
 * vdp_set_color(2, RGB(14, 0, 0)); // Bright red
 * @endcode
 */
void vdp_set_color(u8 index, u16 color);

/**
 * @brief Load multiple palette colors
 * @param index Starting color index (0-63)
 * @param colors Pointer to color data
 * @param count Number of colors to load
 */
void vdp_load_palette(u8 index, const u16* colors, u8 count);

/**
 * @brief Load tile data to VRAM
 * @param tiles Pointer to tile data (32 bytes per tile)
 * @param index Starting tile index (0-2047)
 * @param count Number of tiles to load
 *
 * @code
 * // Load 4 tiles starting at tile index 1
 * vdp_load_tiles(my_tiles, 1, 4);
 * @endcode
 */
void vdp_load_tiles(const u32* tiles, u16 index, u16 count);

/**
 * @brief Set a tile in Plane A
 * @param x Tile X position (0-39 in H40 mode)
 * @param y Tile Y position (0-27)
 * @param tile Tile attribute word (use TILE_ATTR macro)
 */
void vdp_set_tile_a(u8 x, u8 y, u16 tile);

/**
 * @brief Set a tile in Plane B
 * @param x Tile X position (0-39 in H40 mode)
 * @param y Tile Y position (0-27)
 * @param tile Tile attribute word
 */
void vdp_set_tile_b(u8 x, u8 y, u16 tile);

/**
 * @brief Clear Plane A (fill with tile 0)
 */
void vdp_clear_plane_a(void);

/**
 * @brief Clear Plane B (fill with tile 0)
 */
void vdp_clear_plane_b(void);

/**
 * @brief Set background color
 * @param palette Palette number (0-3)
 * @param color Color index within palette (0-15)
 *
 * The background color is used for the backdrop and borders.
 */
void vdp_set_background(u8 palette, u8 color);

/**
 * @brief Set horizontal scroll for Plane A
 * @param scroll Scroll value in pixels (negative = scroll right)
 */
void vdp_set_hscroll_a(s16 scroll);

/**
 * @brief Set horizontal scroll for Plane B
 * @param scroll Scroll value in pixels
 */
void vdp_set_hscroll_b(s16 scroll);

/**
 * @brief Set vertical scroll for Plane A
 * @param scroll Scroll value in pixels (negative = scroll down)
 */
void vdp_set_vscroll_a(s16 scroll);

/**
 * @brief Set vertical scroll for Plane B
 * @param scroll Scroll value in pixels
 */
void vdp_set_vscroll_b(s16 scroll);

#endif /* SMD_VDP_H */
