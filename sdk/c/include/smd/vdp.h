/*
 * smd/vdp.h - Video Display Processor interface
 *
 * The VDP handles all graphics on the Genesis.
 * Note: smdc uses int for all parameters.
 */

#ifndef SMD_VDP_H
#define SMD_VDP_H

/* Hardware Addresses */
#define VDP_DATA        ((volatile unsigned short*)0xC00000)
#define VDP_CTRL        ((volatile unsigned short*)0xC00004)
#define VDP_HVCOUNTER   ((volatile unsigned short*)0xC00008)

/* VDP Status Flags */
#define VDP_STATUS_VBLANK   0x0008
#define VDP_STATUS_HBLANK   0x0004
#define VDP_STATUS_DMA      0x0002

/* Timing Constants */
#define FRAMES_PER_SEC_NTSC 60
#define FRAMES_PER_SEC_PAL  50

/* Default VRAM Addresses */
#define VRAM_PLANE_A    0xC000
#define VRAM_PLANE_B    0xE000
#define VRAM_WINDOW     0xD000
#define VRAM_SPRITES    0xF000
#define VRAM_HSCROLL    0xFC00

/* Colors (BGR format: 0x0BGR) */
#define COLOR_BLACK     0x0000
#define COLOR_WHITE     0x0EEE
#define COLOR_RED       0x000E
#define COLOR_GREEN     0x00E0
#define COLOR_BLUE      0x0E00
#define COLOR_YELLOW    0x00EE
#define COLOR_CYAN      0x0EE0
#define COLOR_MAGENTA   0x0E0E

/* Tile Attributes */
#define TILE_PRIORITY   0x8000
#define TILE_PAL0       0x0000
#define TILE_PAL1       0x2000
#define TILE_PAL2       0x4000
#define TILE_PAL3       0x6000
#define TILE_VFLIP      0x1000
#define TILE_HFLIP      0x0800

/* Functions */
void vdp_init(void);
void vdp_set_reg(int reg, int value);
void vdp_vsync(void);
int vdp_get_status(void);
void vdp_set_write_addr(int addr);
void vdp_set_cram_addr(int index);
void vdp_set_color(int index, int color);
void vdp_load_palette(int index, int *colors, int count);
void vdp_load_tiles(int *tiles, int index, int count);
void vdp_set_tile_a(int x, int y, int tile);
void vdp_set_tile_b(int x, int y, int tile);
void vdp_clear_plane_a(void);
void vdp_clear_plane_b(void);
void vdp_set_background(int palette, int color);
void vdp_set_hscroll_a(int scroll);
void vdp_set_hscroll_b(int scroll);
void vdp_set_vscroll_a(int scroll);
void vdp_set_vscroll_b(int scroll);

/* ========================================================================== */
/* Timing Functions - for music and game sync                                 */
/* ========================================================================== */

/*
 * Wait for VBlank start - blocks until VBlank begins
 * Use this for consistent 60Hz (NTSC) or 50Hz (PAL) timing
 */
void vdp_wait_vblank_start(void);

/*
 * Wait for VBlank end - blocks until VBlank ends
 */
void vdp_wait_vblank_end(void);

/*
 * Wait for next frame - full VBlank cycle
 * This ensures consistent frame-locked timing for music
 */
void vdp_wait_frame(void);

/*
 * Check if currently in VBlank (non-blocking)
 * Returns: 1 if in VBlank, 0 otherwise
 */
int vdp_in_vblank(void);

/*
 * Get frame counter - increments each VBlank
 * Useful for timing music and animations
 */
unsigned int vdp_get_frame_count(void);

/*
 * Reset frame counter to 0
 */
void vdp_reset_frame_count(void);

#endif
