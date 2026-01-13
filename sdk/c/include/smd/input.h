/*
 * smd/input.h - Controller input handling for Sega Mega Drive
 *
 * The Genesis supports two controller ports. Each port can have:
 * - 3-button controller (D-pad + A, B, C, Start)
 * - 6-button controller (adds X, Y, Z, Mode)
 *
 * Note: smdc uses int for all parameters.
 */

#ifndef SMD_INPUT_H
#define SMD_INPUT_H

/* Hardware Addresses */
#define JOY1_DATA_ADDR   0xA10003
#define JOY1_CTRL_ADDR   0xA10009
#define JOY2_DATA_ADDR   0xA10005
#define JOY2_CTRL_ADDR   0xA1000B

/* Button Constants (raw hardware read, active-low) */
#define BTN_UP      0x01
#define BTN_DOWN    0x02
#define BTN_LEFT    0x04
#define BTN_RIGHT   0x08
#define BTN_B       0x10
#define BTN_C       0x20
#define BTN_A       0x40
#define BTN_START   0x80

/* Processed Input Flags (active-high, for convenience) */
#define INPUT_UP        0x0001
#define INPUT_DOWN      0x0002
#define INPUT_LEFT      0x0004
#define INPUT_RIGHT     0x0008
#define INPUT_A         0x0010
#define INPUT_B         0x0020
#define INPUT_C         0x0040
#define INPUT_START     0x0080
#define INPUT_X         0x0100
#define INPUT_Y         0x0200
#define INPUT_Z         0x0400
#define INPUT_MODE      0x0800

/* Functions */
void input_init(void);
int joy1_read(void);
int joy2_read(void);
int input_read(int port);
void input_update(void);
int input_held(int port);
int input_pressed(int port);
int input_released(int port);
int input_is_6button(int port);

#endif
