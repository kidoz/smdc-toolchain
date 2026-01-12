/**
 * @file hello.c
 * @brief Simple "Hello World" example for SMD SDK
 *
 * Demonstrates basic SDK usage:
 * - VDP initialization
 * - Palette setup
 * - Sprite display
 * - Input reading
 *
 * Compile: smdc hello.c -o hello.bin -t rom
 */

#include <smd.h>

/* Player state */
s16 player_x = 160;
s16 player_y = 112;

void main() {
    u16 buttons;

    /* Initialize all subsystems */
    vdp_init();
    sprite_init();
    input_init();

    /* Set up a simple palette */
    vdp_set_color(0, COLOR_BLACK);   /* Background */
    vdp_set_color(1, COLOR_WHITE);   /* Sprite color */

    /* Load a simple solid tile for sprite (tile 1) */
    vdp_set_write_addr(0x0020);  /* Tile 1 = 32 bytes offset */
    {
        int i;
        for (i = 0; i < 8; i++) {
            *VDP_DATA = 0x1111;  /* 4 pixels of color 1 */
            *VDP_DATA = 0x1111;  /* 4 more pixels */
        }
    }

    /* Main game loop */
    while (1) {
        /* Wait for vertical blank */
        vdp_vsync();

        /* Read controller input */
        buttons = input_read(0);

        /* Move player based on input */
        if (buttons & INPUT_UP) {
            player_y -= 2;
        }
        if (buttons & INPUT_DOWN) {
            player_y += 2;
        }
        if (buttons & INPUT_LEFT) {
            player_x -= 2;
        }
        if (buttons & INPUT_RIGHT) {
            player_x += 2;
        }

        /* Clamp position to screen bounds */
        if (player_x < 0) player_x = 0;
        if (player_x > 312) player_x = 312;
        if (player_y < 0) player_y = 0;
        if (player_y > 216) player_y = 216;

        /* Update sprite */
        sprite_set(0, player_x, player_y, SPRITE_SIZE_1x1,
                   SPRITE_ATTR(1, 0, 0, 0, 0));

        /* End sprite list */
        sprite_hide(1);
    }
}
