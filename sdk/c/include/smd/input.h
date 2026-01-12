/**
 * @file smd/input.h
 * @brief Controller input handling for Sega Mega Drive
 * @version 1.0
 *
 * The Genesis supports two controller ports. Each port can have:
 * - 3-button controller (D-pad + A, B, C, Start)
 * - 6-button controller (adds X, Y, Z, Mode)
 *
 * Button state is active-low: bit is 0 when pressed, 1 when released.
 */

#ifndef SMD_INPUT_H
#define SMD_INPUT_H

#include "types.h"

/* ============================================================================
 * Hardware Addresses
 * ============================================================================ */

/** @brief Controller 1 data port */
#define JOY1_DATA   ((volatile u8*)0xA10003)
/** @brief Controller 1 control port */
#define JOY1_CTRL   ((volatile u8*)0xA10009)
/** @brief Controller 2 data port */
#define JOY2_DATA   ((volatile u8*)0xA10005)
/** @brief Controller 2 control port */
#define JOY2_CTRL   ((volatile u8*)0xA1000B)

/* ============================================================================
 * Button Constants (directly from hardware read, active-low)
 * ============================================================================ */

/** @brief D-pad Up button */
#define BTN_UP      0x01
/** @brief D-pad Down button */
#define BTN_DOWN    0x02
/** @brief D-pad Left button */
#define BTN_LEFT    0x04
/** @brief D-pad Right button */
#define BTN_RIGHT   0x08
/** @brief B button */
#define BTN_B       0x10
/** @brief C button */
#define BTN_C       0x20
/** @brief A button */
#define BTN_A       0x40
/** @brief Start button */
#define BTN_START   0x80

/* 6-button controller extra buttons (from second read) */
/** @brief Z button (6-button controller) */
#define BTN_Z       0x01
/** @brief Y button (6-button controller) */
#define BTN_Y       0x02
/** @brief X button (6-button controller) */
#define BTN_X       0x04
/** @brief Mode button (6-button controller) */
#define BTN_MODE    0x08

/* ============================================================================
 * Processed Input Flags (active-high, for convenience)
 * ============================================================================ */

/** @brief Input flag: Up pressed */
#define INPUT_UP        0x0001
/** @brief Input flag: Down pressed */
#define INPUT_DOWN      0x0002
/** @brief Input flag: Left pressed */
#define INPUT_LEFT      0x0004
/** @brief Input flag: Right pressed */
#define INPUT_RIGHT     0x0008
/** @brief Input flag: A pressed */
#define INPUT_A         0x0010
/** @brief Input flag: B pressed */
#define INPUT_B         0x0020
/** @brief Input flag: C pressed */
#define INPUT_C         0x0040
/** @brief Input flag: Start pressed */
#define INPUT_START     0x0080
/** @brief Input flag: X pressed (6-button) */
#define INPUT_X         0x0100
/** @brief Input flag: Y pressed (6-button) */
#define INPUT_Y         0x0200
/** @brief Input flag: Z pressed (6-button) */
#define INPUT_Z         0x0400
/** @brief Input flag: Mode pressed (6-button) */
#define INPUT_MODE      0x0800

/* ============================================================================
 * Functions
 * ============================================================================ */

/**
 * @brief Initialize controller ports
 *
 * Sets up controller data/control ports for reading.
 * Call this once during game initialization.
 */
void input_init(void);

/**
 * @brief Read controller 1 raw state (3-button)
 * @return Button state (active-low: 0 = pressed)
 *
 * @code
 * u8 buttons = joy1_read();
 * if (!(buttons & BTN_UP)) {
 *     // Up is pressed
 *     player_y -= 1;
 * }
 * @endcode
 */
u8 joy1_read(void);

/**
 * @brief Read controller 2 raw state (3-button)
 * @return Button state (active-low: 0 = pressed)
 */
u8 joy2_read(void);

/**
 * @brief Read controller 1 state (processed, active-high)
 * @return Button flags (use INPUT_* constants)
 *
 * This is more convenient than joy1_read() as buttons are active-high.
 *
 * @code
 * u16 input = input_read(0);
 * if (input & INPUT_UP) {
 *     // Up is pressed
 *     player_y -= 1;
 * }
 * if (input & INPUT_A) {
 *     // A is pressed
 *     player_shoot();
 * }
 * @endcode
 */
u16 input_read(u8 port);

/**
 * @brief Update input state (call once per frame)
 *
 * Reads both controllers and updates internal state.
 * Also tracks pressed/released transitions.
 */
void input_update(void);

/**
 * @brief Get current button state for controller
 * @param port Controller port (0 or 1)
 * @return Button flags currently held
 */
u16 input_held(u8 port);

/**
 * @brief Get buttons just pressed this frame
 * @param port Controller port (0 or 1)
 * @return Button flags that transitioned from released to pressed
 *
 * @code
 * if (input_pressed(0) & INPUT_START) {
 *     // Start was just pressed (not held from last frame)
 *     toggle_pause();
 * }
 * @endcode
 */
u16 input_pressed(u8 port);

/**
 * @brief Get buttons just released this frame
 * @param port Controller port (0 or 1)
 * @return Button flags that transitioned from pressed to released
 */
u16 input_released(u8 port);

/**
 * @brief Check if controller is 6-button type
 * @param port Controller port (0 or 1)
 * @return TRUE if 6-button controller detected
 */
bool input_is_6button(u8 port);

#endif /* SMD_INPUT_H */
