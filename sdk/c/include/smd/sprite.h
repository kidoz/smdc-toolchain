/*
 * smd/sprite.h - Sprite management for Sega Mega Drive
 *
 * The Genesis VDP supports up to 80 hardware sprites.
 * Each sprite can be 1-4 tiles wide and 1-4 tiles tall.
 *
 * Note: smdc uses int for all parameters.
 */

#ifndef SMD_SPRITE_H
#define SMD_SPRITE_H

/* Sprite Size Constants: %----WWHH */
#define SPRITE_SIZE_1x1     0x00
#define SPRITE_SIZE_1x2     0x01
#define SPRITE_SIZE_1x3     0x02
#define SPRITE_SIZE_1x4     0x03
#define SPRITE_SIZE_2x1     0x04
#define SPRITE_SIZE_2x2     0x05
#define SPRITE_SIZE_2x3     0x06
#define SPRITE_SIZE_2x4     0x07
#define SPRITE_SIZE_3x1     0x08
#define SPRITE_SIZE_3x2     0x09
#define SPRITE_SIZE_3x3     0x0A
#define SPRITE_SIZE_3x4     0x0B
#define SPRITE_SIZE_4x1     0x0C
#define SPRITE_SIZE_4x2     0x0D
#define SPRITE_SIZE_4x3     0x0E
#define SPRITE_SIZE_4x4     0x0F

/* Sprite Attribute Flags */
#define SPRITE_PRIORITY     0x8000
#define SPRITE_PAL0         0x0000
#define SPRITE_PAL1         0x2000
#define SPRITE_PAL2         0x4000
#define SPRITE_PAL3         0x6000
#define SPRITE_VFLIP        0x1000
#define SPRITE_HFLIP        0x0800

/* Functions */
void sprite_init(void);
void sprite_set(int index, int x, int y, int size, int attr);
void sprite_set_pos(int index, int x, int y);
void sprite_hide(int index);
void sprite_clear(int index);
void sprite_clear_all(void);
void sprite_set_link(int index, int next);
int sprite_get_width(int size);
int sprite_get_height(int size);
int sprite_attr(int tile, int pal, int priority, int hflip, int vflip);

#endif
