/*
 * hello.c - Simple "Hello World" sprite example for SMD SDK
 *
 * Demonstrates basic SDK usage:
 * - VDP initialization
 * - Palette setup
 * - Sprite display
 * - Input reading
 *
 * Compile: smdc hello.c -o hello.bin -t rom -I sdk/c/include
 */

#include <smd/types.h>
#include <smd/vdp.h>
#include <smd/sprite.h>
#include <smd/input.h>
#include <smd/psg.h>

/* Player state */
int player_x;
int player_y;

void main(void) {
    int buttons;
    int i;

    /* Initialize player position */
    player_x = 160;
    player_y = 112;

    /* Initialize all subsystems */
    vdp_init();
    sprite_init();
    input_init();
    psg_init();

    /* Set up a simple palette */
    vdp_set_color(0, COLOR_BLACK);
    vdp_set_color(1, COLOR_WHITE);
    vdp_set_color(2, COLOR_RED);

    /* Load a simple solid tile for sprite (tile 1) */
    vdp_set_write_addr(0x0020);
    i = 0;
_tile_loop:
    *VDP_DATA = 0x1111;
    *VDP_DATA = 0x1111;
    i = i + 1;
    if (i < 8) { goto _tile_loop; }

    /* Main game loop */
mainloop:
    vdp_vsync();

    /* Read controller input */
    buttons = input_read(0);

    /* Move player based on input */
    if (buttons & INPUT_UP) {
        player_y = player_y - 2;
    }
    if (buttons & INPUT_DOWN) {
        player_y = player_y + 2;
    }
    if (buttons & INPUT_LEFT) {
        player_x = player_x - 2;
    }
    if (buttons & INPUT_RIGHT) {
        player_x = player_x + 2;
    }

    /* Play beep on button A */
    if (buttons & INPUT_A) {
        psg_beep(0, PSG_NOTE_A4, 4);
    } else {
        psg_set_volume(0, 15);
    }

    /* Clamp position to screen bounds */
    if (player_x < 0) { player_x = 0; }
    if (player_x > 312) { player_x = 312; }
    if (player_y < 0) { player_y = 0; }
    if (player_y > 216) { player_y = 216; }

    /* Update sprite */
    sprite_set(0, player_x, player_y, SPRITE_SIZE_1x1,
               sprite_attr(1, 0, 0, 0, 0));

    /* End sprite list */
    sprite_hide(1);

    goto mainloop;
}
