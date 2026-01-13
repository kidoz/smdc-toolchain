/**
 * Pong Game for Sega Genesis/Megadrive
 * Classic arcade-style Pong with center line and score display
 */

// ============================================================================
// Hardware Register Addresses
// ============================================================================

#define VDP_DATA  0xC00000
#define VDP_CTRL  0xC00004
#define CTRL_DATA 0xA10003
#define CTRL_CTRL 0xA10009
#define PSG_PORT  0xC00011

// ============================================================================
// Constants
// ============================================================================

int SCREEN_WIDTH = 320;
int SCREEN_HEIGHT = 224;
int PADDLE_HEIGHT = 32;
int PADDLE_SPEED = 4;
int BALL_SIZE = 8;
int BALL_SPEED = 3;
int LEFT_MARGIN = 16;
int RIGHT_MARGIN = 304;
int TOP_MARGIN = 16;
int BOTTOM_MARGIN = 208;

// ============================================================================
// Game State
// ============================================================================

struct Paddle {
    int x;
    int y;
    int score;
};

struct Ball {
    int x;
    int y;
    int dx;
    int dy;
};

struct Paddle player1 = {16, 96, 0};
struct Paddle player2 = {296, 96, 0};
struct Ball ball = {156, 108, 3, 2};
int game_running = 1;
int frame_count = 0;
int sound_timer = 0;

// ============================================================================
// Hardware Access Functions
// ============================================================================

void write_vdp_ctrl(int value) {
    short *ptr;
    ptr = (short *)VDP_CTRL;
    *ptr = (short)value;
}

void write_vdp_data(int value) {
    short *ptr;
    ptr = (short *)VDP_DATA;
    *ptr = (short)value;
}

int read_vdp_status(void) {
    short *ptr;
    ptr = (short *)VDP_CTRL;
    return *ptr;
}

void init_controller(void) {
    char *ctrl;
    ctrl = (char *)CTRL_CTRL;
    *ctrl = 0x40;  /* Set TH pin as output */
}

int read_controller(void) {
    char *data;
    int buttons;
    int buttons2;

    data = (char *)CTRL_DATA;

    /* Set TH high and read: Up, Down, Left, Right, B, C */
    *data = 0x40;
    buttons = *data & 0x3F;

    /* Set TH low and read: Up, Down, 0, 0, A, Start */
    *data = 0x00;
    buttons2 = *data;

    /* Combine: bits 0-5 = UDLRBC, bit 6 = A, bit 7 = Start */
    buttons = buttons | ((buttons2 & 0x10) << 2) | ((buttons2 & 0x20) << 2);

    return buttons;
}

// ============================================================================
// PSG Sound Functions
// ============================================================================

void psg_write(int value) {
    char *port;
    port = (char *)PSG_PORT;
    *port = (char)value;
}

void psg_set_volume(int channel, int volume) {
    int cmd;
    cmd = 0x90 | (channel << 5) | (volume & 0x0F);
    psg_write(cmd);
}

void psg_set_tone(int channel, int freq) {
    int divider;
    int ch;
    int byte1;
    int byte2;

    if (freq == 0) {
        return;
    }

    // PSG clock = 3579545 Hz, divider = clock / (32 * freq)
    divider = 3579545 / (32 * freq);
    if (divider > 1023) {
        divider = 1023;
    }

    ch = (channel & 0x03) << 5;
    byte1 = 0x80 | ch | (divider & 0x0F);
    byte2 = (divider >> 4) & 0x3F;

    psg_write(byte1);
    psg_write(byte2);
}

void psg_init(void) {
    /* Silence all 4 channels */
    psg_set_volume(0, 15);
    psg_set_volume(1, 15);
    psg_set_volume(2, 15);
    psg_set_volume(3, 15);
}

void psg_stop(void) {
    psg_set_volume(0, 15);
    psg_set_volume(1, 15);
    psg_set_volume(2, 15);
    psg_set_volume(3, 15);
}

/* Sound effect: paddle hit (high-pitched blip) */
void sound_paddle_hit(void) {
    psg_set_tone(0, 880);    /* A5 */
    psg_set_volume(0, 2);    /* Slightly quieter */
    sound_timer = 4;
}

/* Sound effect: wall bounce (lower blip) */
void sound_wall_bounce(void) {
    psg_set_tone(0, 440);    /* A4 */
    psg_set_volume(0, 4);    /* Medium volume */
    sound_timer = 3;
}

/* Sound effect: score (longer descending tone) */
void sound_score(void) {
    psg_set_tone(0, 220);    /* A3 */
    psg_set_volume(0, 0);    /* Max volume */
    sound_timer = 15;
}

void sound_update(void) {
    if (sound_timer > 0) {
        sound_timer = sound_timer - 1;
        if (sound_timer == 0) {
            psg_stop();
        }
    }
}

// ============================================================================
// VDP Functions
// ============================================================================

void vdp_set_register(int reg, int value) {
    int cmd;
    cmd = 0x8000 | (reg << 8) | value;
    write_vdp_ctrl(cmd);
}

void vdp_init(void) {
    vdp_set_register(0, 0x04);
    vdp_set_register(1, 0x44);
    vdp_set_register(2, 0x30);  // Plane A at 0xC000
    vdp_set_register(3, 0x00);
    vdp_set_register(4, 0x07);
    vdp_set_register(5, 0x78);  // Sprite table at 0xF000
    vdp_set_register(6, 0x00);
    vdp_set_register(7, 0x00);  // Background = black
    vdp_set_register(10, 0xFF);
    vdp_set_register(11, 0x00);
    vdp_set_register(12, 0x81);  // H40 mode
    vdp_set_register(13, 0x3F);
    vdp_set_register(15, 0x02);  // Auto-increment 2
    vdp_set_register(16, 0x01);
    vdp_set_register(17, 0x00);
    vdp_set_register(18, 0x00);
}

void vdp_set_write_address(int address) {
    int cmd1;
    int cmd2;
    cmd1 = 0x4000 | (address & 0x3FFF);
    cmd2 = (address >> 14) & 0x03;
    write_vdp_ctrl(cmd1);
    write_vdp_ctrl(cmd2);
}

void vdp_wait_vblank(void) {
    int status;
    status = read_vdp_status();
    while (!(status & 0x08)) {
        status = read_vdp_status();
    }
}

// ============================================================================
// Palette Setup
// ============================================================================

void setup_palette(void) {
    write_vdp_ctrl(0xC000);
    write_vdp_ctrl(0x0000);
    write_vdp_data(0x0000);  // Color 0 = black
    write_vdp_data(0x0EEE);  // Color 1 = white
}

// ============================================================================
// Tile Loading
// ============================================================================

void load_tiles(void) {
    int row;

    // Tiles 1-4: Paddle outline (top/middle/bottom)
    // Tile addresses: 1=0x0020, 2=0x0040, 3=0x0060, 4=0x0080
    vdp_set_write_address(0x0020);  // Tile 1: top
    row = 0;
    while (row < 8) {
        if (row == 0) {
            write_vdp_data(0x1111);
            write_vdp_data(0x1111);
        } else {
            write_vdp_data(0x1000);
            write_vdp_data(0x0001);
        }
        row = row + 1;
    }

    vdp_set_write_address(0x0040);  // Tile 2: middle
    row = 0;
    while (row < 8) {
        write_vdp_data(0x1000);
        write_vdp_data(0x0001);
        row = row + 1;
    }

    vdp_set_write_address(0x0060);  // Tile 3: middle
    row = 0;
    while (row < 8) {
        write_vdp_data(0x1000);
        write_vdp_data(0x0001);
        row = row + 1;
    }

    vdp_set_write_address(0x0080);  // Tile 4: bottom
    row = 0;
    while (row < 8) {
        if (row == 7) {
            write_vdp_data(0x1111);
            write_vdp_data(0x1111);
        } else {
            write_vdp_data(0x1000);
            write_vdp_data(0x0001);
        }
        row = row + 1;
    }

    // Tile 5: Center line segment (small dotted dash)
    vdp_set_write_address(0x00A0);
    row = 0;
    while (row < 8) {
        if (row == 0 || row == 1 || row == 4 || row == 5) {
            write_vdp_data(0x0001);
            write_vdp_data(0x1000);
        } else {
            write_vdp_data(0x0000);
            write_vdp_data(0x0000);
        }
        row = row + 1;
    }

    // Tile 6: Ball (small 2x2 dot)
    vdp_set_write_address(0x00C0);
    row = 0;
    while (row < 8) {
        if (row == 3 || row == 4) {
            write_vdp_data(0x0001);
            write_vdp_data(0x1000);
        } else {
            write_vdp_data(0x0000);
            write_vdp_data(0x0000);
        }
        row = row + 1;
    }

    // Tile 7: Solid white tile (for score digits)
    vdp_set_write_address(0x00E0);
    row = 0;
    while (row < 8) {
        write_vdp_data(0x1111);
        write_vdp_data(0x1111);
        row = row + 1;
    }
}

// ============================================================================
// Draw Center Line
// ============================================================================

void draw_center_line(void) {
    int y;
    int addr;

    // Plane A is at 0xC000, 64 words per row (128 bytes)
    // Center column is at x=20 (tile column 20 in H40 mode)
    y = 0;
    while (y < 28) {
        addr = 0xC000 + (y * 128) + (20 * 2);
        vdp_set_write_address(addr);
        write_vdp_data(0x0005);  // Tile 5 (dashed line)
        y = y + 1;
    }
}

// ============================================================================
// Score Display - Draw a digit at tile position
// ============================================================================

void draw_digit(int x, int y, int digit) {
    int addr;

    /* Simple 3x5 tile digit patterns drawn with solid tiles */
    /* We'll draw each digit as a 3-wide by 5-tall block pattern */

    addr = 0xC000 + (y * 128) + (x * 2);

    if (digit == 0) {
        // Row 0: ###
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        // Row 1: # #
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        // Row 2: # #
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        // Row 3: # #
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        // Row 4: ###
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
    } else if (digit == 1) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
    } else if (digit == 2) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
    } else if (digit == 3) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
    } else if (digit == 4) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
    } else if (digit == 5) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
    } else if (digit == 6) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
    } else if (digit == 7) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
    } else if (digit == 8) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
    } else if (digit == 9) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
    }
}

void draw_scores(void) {
    // Player 1 score at tile position (8, 2)
    draw_digit(8, 2, player1.score);
    // Player 2 score at tile position (28, 2)
    draw_digit(28, 2, player2.score);
}

// Draw a single tile at position
void draw_tile(int x, int y, int tile) {
    int addr;
    addr = 0xC000 + (y * 128) + (x * 2);
    vdp_set_write_address(addr);
    write_vdp_data(tile);
}

// Draw letter using 3x5 tile pattern (similar to digits)
void draw_letter(int x, int y, int letter) {
    int addr;
    addr = 0xC000 + (y * 128) + (x * 2);

    if (letter == 'R') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
    }
    if (letter == 'E') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
    }
    if (letter == 'A') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
    }
    if (letter == 'D') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0000);
    }
    if (letter == 'Y') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
    }
    if (letter == '?') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
    }
    if (letter == 'P') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0000);
    }
    if (letter == 'U') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
    }
    if (letter == 'S') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
    }
    if (letter == 'H') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007); write_vdp_data(0x0000); write_vdp_data(0x0007);
    }
    if (letter == 'T') {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007); write_vdp_data(0x0007); write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000); write_vdp_data(0x0007); write_vdp_data(0x0000);
    }
}

// Clear a 3x5 tile area (for erasing text)
void clear_text_area(int x, int y, int width) {
    int addr;
    int row;
    int col;
    row = 0;
    while (row < 5) {
        addr = 0xC000 + ((y + row) * 128) + (x * 2);
        vdp_set_write_address(addr);
        col = 0;
        while (col < width) {
            write_vdp_data(0x0000);
            col = col + 1;
        }
        row = row + 1;
    }
}

// Display "READY?" text
void draw_ready_text(void) {
    // "READY?" centered on screen (screen is 40 tiles wide)
    // Each letter is 3 tiles wide + 1 space = 4 tiles per letter
    // "READY?" = 6 letters = 24 tiles, center at x = 8
    draw_letter(8, 10, 'R');
    draw_letter(12, 10, 'E');
    draw_letter(16, 10, 'A');
    draw_letter(20, 10, 'D');
    draw_letter(24, 10, 'Y');
    draw_letter(28, 10, '?');
}

// Display "Push start" text
void draw_push_start_text(void) {
    // "PUSH START" centered on screen (40 tiles wide)
    // Each letter is 3 tiles wide + 1 space = 4 tiles per letter
    // Start at x=2 to center properly
    draw_letter(2, 17, 'P');
    draw_letter(6, 17, 'U');
    draw_letter(10, 17, 'S');
    draw_letter(14, 17, 'H');
    // space gap
    draw_letter(20, 17, 'S');
    draw_letter(24, 17, 'T');
    draw_letter(28, 17, 'A');
    draw_letter(32, 17, 'R');
    draw_letter(36, 17, 'T');
}

// Clear ready screen text
void clear_ready_screen(void) {
    clear_text_area(8, 10, 24);   // READY? from x=8 to x=31
    clear_text_area(2, 17, 38);   // PUSH START from x=2 to x=39
}

// Wait for START button press
void wait_for_start(void) {
    int buttons;

    draw_ready_text();
    draw_push_start_text();

    /* Wait for START to be released first (in case already held) */
    buttons = read_controller();
    while (!(buttons & 0x80)) {
        vdp_wait_vblank();
        buttons = read_controller();
    }

    /* Wait for START to be pressed */
    while (buttons & 0x80) {
        vdp_wait_vblank();
        buttons = read_controller();
    }

    /* Clear the ready screen */
    clear_ready_screen();
}

// ============================================================================
// Sprite Functions
// ============================================================================

void update_sprite(int index, int x, int y, int size, int tile) {
    int addr;
    addr = 0xF000 + (index * 8);
    vdp_set_write_address(addr);
    write_vdp_data(y + 128);
    write_vdp_data((size << 8) | (index + 1));
    write_vdp_data(tile);
    write_vdp_data(x + 128);
}

void clear_sprite(int index) {
    int addr;
    addr = 0xF000 + (index * 8);
    vdp_set_write_address(addr);
    write_vdp_data(0);
    write_vdp_data(0);
    write_vdp_data(0);
    write_vdp_data(0);
}

// ============================================================================
// Game Logic
// ============================================================================

void reset_ball(void) {
    ball.x = 156;
    ball.y = 108;
    if (frame_count & 1) {
        ball.dx = BALL_SPEED;
    } else {
        ball.dx = -BALL_SPEED;
    }
    ball.dy = 2;
}

void update_ball(void) {
    int paddle_width;
    paddle_width = 8;

    ball.x = ball.x + ball.dx;
    ball.y = ball.y + ball.dy;

    // Bounce off top/bottom
    if (ball.y < TOP_MARGIN) {
        ball.y = TOP_MARGIN;
        ball.dy = -ball.dy;
        sound_wall_bounce();
    }
    if (ball.y > BOTTOM_MARGIN - BALL_SIZE) {
        ball.y = BOTTOM_MARGIN - BALL_SIZE;
        ball.dy = -ball.dy;
        sound_wall_bounce();
    }

    // Player 1 paddle collision
    if (ball.x < player1.x + paddle_width) {
        if (ball.x > player1.x - BALL_SIZE) {
            if (ball.y + BALL_SIZE > player1.y) {
                if (ball.y < player1.y + PADDLE_HEIGHT) {
                    ball.x = player1.x + paddle_width;
                    ball.dx = -ball.dx;
                    sound_paddle_hit();
                }
            }
        }
    }

    // Player 2 paddle collision
    if (ball.x + BALL_SIZE > player2.x) {
        if (ball.x < player2.x + paddle_width) {
            if (ball.y + BALL_SIZE > player2.y) {
                if (ball.y < player2.y + PADDLE_HEIGHT) {
                    ball.x = player2.x - BALL_SIZE;
                    ball.dx = -ball.dx;
                    sound_paddle_hit();
                }
            }
        }
    }

    // Scoring
    if (ball.x < LEFT_MARGIN) {
        player2.score = player2.score + 1;
        sound_score();
        reset_ball();
    }
    if (ball.x > RIGHT_MARGIN) {
        player1.score = player1.score + 1;
        sound_score();
        reset_ball();
    }
}

void update_paddles(int buttons) {
    // Player 1: Up/Down buttons
    if (!(buttons & 0x01)) {
        player1.y = player1.y - PADDLE_SPEED;
    }
    if (!(buttons & 0x02)) {
        player1.y = player1.y + PADDLE_SPEED;
    }

    // Clamp player 1
    if (player1.y < TOP_MARGIN) {
        player1.y = TOP_MARGIN;
    }
    if (player1.y > BOTTOM_MARGIN - PADDLE_HEIGHT) {
        player1.y = BOTTOM_MARGIN - PADDLE_HEIGHT;
    }

    // AI for player 2
    if (ball.x > 160) {
        if (player2.y + 16 < ball.y) {
            player2.y = player2.y + 3;
        }
        if (player2.y + 16 > ball.y) {
            player2.y = player2.y - 3;
        }
    }

    // Clamp player 2
    if (player2.y < TOP_MARGIN) {
        player2.y = TOP_MARGIN;
    }
    if (player2.y > BOTTOM_MARGIN - PADDLE_HEIGHT) {
        player2.y = BOTTOM_MARGIN - PADDLE_HEIGHT;
    }
}

// ============================================================================
// Rendering
// ============================================================================

void render(void) {
    // Paddle sprites: size 0x03 = 1 wide x 4 tall (8x32 pixels)
    update_sprite(0, player1.x, player1.y, 0x03, 1);
    update_sprite(1, player2.x, player2.y, 0x03, 1);
    // Ball sprite: size 0x00 = 1x1 (8x8 pixels)
    update_sprite(2, ball.x, ball.y, 0x00, 6);
    // End sprite list
    clear_sprite(3);
}

// ============================================================================
// Main
// ============================================================================

int main(void) {
    int i;
    int buttons;
    int last_p1_score;
    int last_p2_score;

    vdp_init();
    psg_init();
    init_controller();
    setup_palette();
    load_tiles();

    /* Clear sprites */
    i = 0;
    while (i < 80) {
        clear_sprite(i);
        i = i + 1;
    }

    /* Wait for START button before starting game */
    wait_for_start();

    /* Draw static elements after ready screen is cleared */
    draw_center_line();
    draw_scores();  /* Initial score display */

    last_p1_score = 0;
    last_p2_score = 0;

    while (game_running) {
        vdp_wait_vblank();

        buttons = read_controller();
        update_paddles(buttons);
        update_ball();
        render();
        sound_update();

        /* Update score display when score changes */
        if (player1.score != last_p1_score || player2.score != last_p2_score) {
            draw_scores();
            last_p1_score = player1.score;
            last_p2_score = player2.score;
        }

        frame_count = frame_count + 1;

        if (player1.score >= 10 || player2.score >= 10) {
            game_running = 0;
        }
    }

    /* Game over - infinite loop */
    while (1) {
        vdp_wait_vblank();
    }

    return 0;
}
