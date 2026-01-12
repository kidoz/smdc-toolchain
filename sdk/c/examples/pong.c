/**
 * Pong Game for Sega Genesis/Megadrive
 * Classic arcade-style Pong with center line and score display
 */

// ============================================================================
// Hardware Register Addresses
// ============================================================================

int VDP_DATA_ADDR = 0xC00000;
int VDP_CTRL_ADDR = 0xC00004;
int CTRL_DATA_ADDR = 0xA10003;
int CTRL_CTRL_ADDR = 0xA10009;
int PSG_PORT_ADDR = 0xC00011;

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
    ptr = VDP_CTRL_ADDR;
    *ptr = value;
}

void write_vdp_data(int value) {
    short *ptr;
    ptr = VDP_DATA_ADDR;
    *ptr = value;
}

int read_vdp_status() {
    short *ptr;
    ptr = VDP_CTRL_ADDR;
    return *ptr;
}

int read_controller() {
    char *ctrl;
    char *data;
    ctrl = CTRL_CTRL_ADDR;
    *ctrl = 0x40;
    data = CTRL_DATA_ADDR;
    return *data;
}

// ============================================================================
// PSG Sound Functions
// ============================================================================

void psg_write(int value) {
    char *port;
    port = PSG_PORT_ADDR;
    *port = value;
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

void psg_init() {
    // Silence all 4 channels
    psg_set_volume(0, 15);
    psg_set_volume(1, 15);
    psg_set_volume(2, 15);
    psg_set_volume(3, 15);
}

void psg_stop() {
    psg_set_volume(0, 15);
    psg_set_volume(1, 15);
    psg_set_volume(2, 15);
    psg_set_volume(3, 15);
}

// Sound effect: paddle hit (high-pitched blip)
void sound_paddle_hit() {
    psg_set_tone(0, 880);    // A5
    psg_set_volume(0, 2);    // Slightly quieter
    sound_timer = 4;
}

// Sound effect: wall bounce (lower blip)
void sound_wall_bounce() {
    psg_set_tone(0, 440);    // A4
    psg_set_volume(0, 4);    // Medium volume
    sound_timer = 3;
}

// Sound effect: score (longer descending tone)
void sound_score() {
    psg_set_tone(0, 220);    // A3
    psg_set_volume(0, 0);    // Max volume
    sound_timer = 15;
}

void sound_update() {
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

void vdp_init() {
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

void vdp_wait_vblank() {
    int status;
    status = read_vdp_status();
    while (!(status & 0x08)) {
        status = read_vdp_status();
    }
}

// ============================================================================
// Palette Setup
// ============================================================================

void setup_palette() {
    write_vdp_ctrl(0xC000);
    write_vdp_ctrl(0x0000);
    write_vdp_data(0x0000);  // Color 0 = black
    write_vdp_data(0x0EEE);  // Color 1 = white
}

// ============================================================================
// Tile Loading
// ============================================================================

void load_tiles() {
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
}

// ============================================================================
// Draw Center Line
// ============================================================================

void draw_center_line() {
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
    int row;
    int tile;

    // Simple 3x5 tile digit patterns drawn with solid tiles
    // We'll draw each digit as a 3-wide by 5-tall block pattern

    addr = 0xC000 + (y * 128) + (x * 2);

    if (digit == 0) {
        // Row 0: ###
        vdp_set_write_address(addr);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        // Row 1: # #
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0001);
        // Row 2: # #
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0001);
        // Row 3: # #
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0001);
        // Row 4: ###
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
    }
    if (digit == 1) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0000); write_vdp_data(0x0001); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000); write_vdp_data(0x0001); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000); write_vdp_data(0x0001); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0001); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000); write_vdp_data(0x0001); write_vdp_data(0x0000);
    }
    if (digit == 2) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
    }
    if (digit == 3) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
    }
    if (digit == 4) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
    }
    if (digit == 5) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
    }
    if (digit == 6) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
    }
    if (digit == 7) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
    }
    if (digit == 8) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
    }
    if (digit == 9) {
        vdp_set_write_address(addr);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0001); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000); write_vdp_data(0x0000); write_vdp_data(0x0001);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0001); write_vdp_data(0x0001); write_vdp_data(0x0001);
    }
}

void draw_scores() {
    // Player 1 score at tile position (8, 2)
    draw_digit(8, 2, player1.score);
    // Player 2 score at tile position (28, 2)
    draw_digit(28, 2, player2.score);
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

void reset_ball() {
    ball.x = 156;
    ball.y = 108;
    if (frame_count & 1) {
        ball.dx = BALL_SPEED;
    } else {
        ball.dx = -BALL_SPEED;
    }
    ball.dy = 2;
}

void update_ball() {
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

void render() {
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

void main() {
    int i;
    int buttons;
    int last_p1_score;
    int last_p2_score;

    vdp_init();
    psg_init();
    setup_palette();
    load_tiles();

    // Clear sprites
    i = 0;
    while (i < 80) {
        clear_sprite(i);
        i = i + 1;
    }

    // Draw static elements
    draw_center_line();

    last_p1_score = 0;
    last_p2_score = 0;

    while (game_running) {
        vdp_wait_vblank();

        buttons = read_controller();
        update_paddles(buttons);
        update_ball();
        render();
        sound_update();

        if (player1.score != last_p1_score) {
            last_p1_score = player1.score;
        }
        if (player2.score != last_p2_score) {
            last_p2_score = player2.score;
        }

        frame_count = frame_count + 1;

        if (player1.score >= 10) {
            game_running = 0;
        }
        if (player2.score >= 10) {
            game_running = 0;
        }
    }

    while (1) {
        vdp_wait_vblank();
    }
}
