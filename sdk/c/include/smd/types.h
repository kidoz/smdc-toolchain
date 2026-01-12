/**
 * @file smd/types.h
 * @brief Common type definitions for Sega Mega Drive development
 * @version 1.0
 */

#ifndef SMD_TYPES_H
#define SMD_TYPES_H

/** @brief Unsigned 8-bit integer (0 to 255) */
typedef unsigned char u8;

/** @brief Unsigned 16-bit integer (0 to 65535) */
typedef unsigned short u16;

/** @brief Unsigned 32-bit integer (0 to 4294967295) */
typedef unsigned int u32;

/** @brief Signed 8-bit integer (-128 to 127) */
typedef signed char s8;

/** @brief Signed 16-bit integer (-32768 to 32767) */
typedef short s16;

/** @brief Signed 32-bit integer */
typedef int s32;

/** @brief Boolean type (0 = false, non-zero = true) */
typedef u8 bool;

/** @brief Boolean true value */
#define TRUE  1
/** @brief Boolean false value */
#define FALSE 0

/** @brief NULL pointer */
#define NULL ((void*)0)

/**
 * @brief Fixed-point 16.16 number
 *
 * Upper 16 bits are integer part, lower 16 bits are fractional.
 * Use FIX16() macro to create from integer.
 */
typedef s32 fix16;

/**
 * @brief Convert integer to fix16
 * @param x Integer value
 * @return fix16 representation
 */
#define FIX16(x) ((fix16)((x) << 16))

/**
 * @brief Convert fix16 to integer (truncates)
 * @param x fix16 value
 * @return Integer part
 */
#define FIX16_INT(x) ((s16)((x) >> 16))

/**
 * @brief Multiply two fix16 values
 * @param a First fix16 value
 * @param b Second fix16 value
 * @return Product as fix16
 */
#define FIX16_MUL(a, b) ((fix16)(((s32)(a) * (s32)(b)) >> 16))

#endif /* SMD_TYPES_H */
