/**
 * Minimal VDP test - just set background color
 */

int VDP_DATA_ADDR = 0xC00000;
int VDP_CTRL_ADDR = 0xC00004;

int main(void) {
    short *vdp_ctrl;
    short *vdp_data;

    vdp_ctrl = (short *)VDP_CTRL_ADDR;
    vdp_data = (short *)VDP_DATA_ADDR;

    // Set VDP register 1: enable display
    *vdp_ctrl = 0x8144;

    // Set VDP register 7: background color = palette 0, color 1
    *vdp_ctrl = 0x8701;

    // Set CRAM write address to color 1
    *vdp_ctrl = 0xC002;
    *vdp_ctrl = 0x0000;

    // Write bright red color
    *vdp_data = 0x000E;

    // Infinite loop
    while (1) {
    }

    return 0;
}
