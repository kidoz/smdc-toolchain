/*
 * smd/types.h - Common type definitions for Sega Mega Drive
 *
 * Note: smdc compiler has limited typedef support.
 * Use int for function parameters, unsigned char* for byte pointers.
 */

#ifndef SMD_TYPES_H
#define SMD_TYPES_H

/* Basic integer typedefs */
typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef signed char s8;
typedef short s16;
typedef int s32;

/* Boolean */
#define TRUE  1
#define FALSE 0

/* NULL pointer */
#ifndef NULL
#define NULL ((void*)0)
#endif

#endif
