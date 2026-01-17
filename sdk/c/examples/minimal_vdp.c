/**
 * Minimal VDP test - just set background color to red
 */

int main(void) {
    short *vdp_ctrl;
    short *vdp_data;

    vdp_ctrl = (short *)0xC00004;
    vdp_data = (short *)0xC00000;

    // Initialize VDP registers first
    *vdp_ctrl = 0x8004;  // Reg 0 = 0x04
    *vdp_ctrl = 0x8144;  // Reg 1 = 0x44 (display enable, mode 5)
    *vdp_ctrl = 0x8230;  // Reg 2 = 0x30 (plane A)
    *vdp_ctrl = 0x8407;  // Reg 4 = 0x07 (plane B)
    *vdp_ctrl = 0x8578;  // Reg 5 = 0x78 (sprite table)
    *vdp_ctrl = 0x8703;  // Reg 7 = 0x03 (backdrop = color 3)
    *vdp_ctrl = 0x8AFF;  // Reg 10 = 0xFF
    *vdp_ctrl = 0x8B00;  // Reg 11 = 0x00
    *vdp_ctrl = 0x8C81;  // Reg 12 = 0x81 (H40 mode)
    *vdp_ctrl = 0x8D3F;  // Reg 13 = 0x3F
    *vdp_ctrl = 0x8F02;  // Reg 15 = 0x02 (auto-inc)
    *vdp_ctrl = 0x9001;  // Reg 16 = 0x01

    // Set palette - CRAM write to address 0
    *vdp_ctrl = 0xC000;
    *vdp_ctrl = 0x0000;

    // Write colors
    *vdp_data = 0x0000;  // Color 0 = black
    *vdp_data = 0x0EEE;  // Color 1 = white
    *vdp_data = 0x00E0;  // Color 2 = green
    *vdp_data = 0x000E;  // Color 3 = red

    // Infinite loop
    while (1) {
    }

    return 0;
}
