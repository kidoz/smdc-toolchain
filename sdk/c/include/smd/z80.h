/*
 * smd/z80.h - Z80 CPU control and sound driver interface
 *
 * The Z80 has 8KB of RAM at 0xA00000-0xA01FFF (68000 view).
 * The 68000 must request the bus before accessing Z80 RAM.
 *
 * Note: smdc uses int for all parameters.
 */

#ifndef SMD_Z80_H
#define SMD_Z80_H

/* Z80 Control Addresses (68000 view) */
#define Z80_RAM         0xA00000
#define Z80_BUS_REQ     0xA11100
#define Z80_RESET       0xA11200
#define Z80_CMD_ADDR    0xA01F00
#define Z80_DATA_ADDR   0xA01F01

/* Z80 Driver Commands */
#define Z80_CMD_NOP         0x00
#define Z80_CMD_PLAY_NOTE   0x01
#define Z80_CMD_STOP_NOTE   0x02
#define Z80_CMD_SET_PATCH   0x03
#define Z80_CMD_SET_TEMPO   0x04
#define Z80_CMD_PLAY_SEQ    0x10
#define Z80_CMD_STOP_SEQ    0x11

/* Z80 Bus Control Functions */
void z80_request_bus(void);
void z80_release_bus(void);
void z80_reset_on(void);
void z80_reset_off(void);

/* Z80 Driver Functions */
void z80_load_driver(void);
void z80_init(void);
void z80_send_command(int cmd, int d1, int d2, int d3);
void z80_play_note(int ch, int note, int octave);
void z80_stop_note(int ch);

#endif
