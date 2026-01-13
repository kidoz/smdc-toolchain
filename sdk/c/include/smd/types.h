/**
 * @file smd/types.h
 * @brief Common type definitions for Sega Mega Drive development
 * @version 1.1
 *
 * Note: smdc compiler has limited typedef support.
 * Use int for most values, unsigned char* for byte pointers.
 */

/*
 * Type aliases (for documentation - smdc doesn't fully support typedef)
 *
 * u8  = unsigned char  (8-bit unsigned)
 * u16 = unsigned short (16-bit unsigned)
 * u32 = unsigned int   (32-bit unsigned)
 * s8  = signed char    (8-bit signed)
 * s16 = short          (16-bit signed)
 * s32 = int            (32-bit signed)
 *
 * For smdc, use:
 * - int for most values (32-bit on M68k)
 * - unsigned char* for byte pointers
 * - unsigned short* for word pointers
 */

/* Basic integer typedefs that smdc can handle */
typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef signed char s8;
typedef short s16;
typedef int s32;
