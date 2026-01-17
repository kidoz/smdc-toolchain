/**
 * Simplest possible VDP test - should show red screen
 */

int main(void) {
    short *vdp_ctrl;

    // Direct address assignment
    vdp_ctrl = (short *)0xC00004;

    // VDP register setup (minimal for display)
    *vdp_ctrl = 0x8004;  // Reg 0: Normal color mode
    *vdp_ctrl = 0x8144;  // Reg 1: Display ON (bit 6), 224 lines (bit 3=0)
    *vdp_ctrl = 0x8230;  // Reg 2: Plane A name table at 0xC000
    *vdp_ctrl = 0x8400;  // Reg 4: Plane B name table at 0x0000
    *vdp_ctrl = 0x8700;  // Reg 7: Background = palette 0, color 0
    *vdp_ctrl = 0x8C81;  // Reg 12: H40 mode (320 pixels)
    *vdp_ctrl = 0x8F02;  // Reg 15: Auto-increment = 2

    // Write to CRAM at color index 0
    *vdp_ctrl = 0xC000;  // CRAM write + address 0 (color 0)
    *vdp_ctrl = 0x0000;

    // Write red color to VDP data port
    short *vdp_data;
    vdp_data = (short *)0xC00000;
    *vdp_data = 0x000E;  // Red color in Genesis format (0BGR)

    // Infinite loop
    while (1) {
    }

    return 0;
}
