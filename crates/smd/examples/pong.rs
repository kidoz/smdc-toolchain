/// Pong Game for Sega Genesis/Megadrive (Rust Version)
///
/// Classic arcade-style Pong with center line and score display

// Hardware addresses
const VDP_DATA: i32 = 0xC00000;
const VDP_CTRL: i32 = 0xC00004;
const CTRL_DATA: i32 = 0xA10003;
const CTRL_CTRL: i32 = 0xA10009;
const PSG_PORT: i32 = 0xC00011;

// Game constants
const SCREEN_WIDTH: i32 = 320;
const SCREEN_HEIGHT: i32 = 224;
const PADDLE_HEIGHT: i32 = 32;
const PADDLE_SPEED: i32 = 4;
const BALL_SIZE: i32 = 8;
const BALL_SPEED: i32 = 3;
const LEFT_MARGIN: i32 = 16;
const RIGHT_MARGIN: i32 = 304;
const TOP_MARGIN: i32 = 16;
const BOTTOM_MARGIN: i32 = 208;

// Global state
static mut PLAYER1_X: i32 = 16;
static mut PLAYER1_Y: i32 = 96;
static mut PLAYER1_SCORE: i32 = 0;
static mut PLAYER2_X: i32 = 296;
static mut PLAYER2_Y: i32 = 96;
static mut PLAYER2_SCORE: i32 = 0;
static mut BALL_X: i32 = 156;
static mut BALL_Y: i32 = 108;
static mut BALL_DX: i32 = 3;
static mut BALL_DY: i32 = 2;
static mut GAME_RUNNING: i32 = 1;
static mut FRAME_COUNT: i32 = 0;
static mut SOUND_TIMER: i32 = 0;

// ============================================================================
// Hardware Access Functions
// ============================================================================

fn poke16(addr: i32, value: i32) {
    unsafe {
        let ptr: *mut i16 = addr as *mut i16;
        let val: i16 = value as i16;
        *ptr = val;
    }
}

fn peek16(addr: i32) -> i32 {
    unsafe {
        let ptr: *mut i16 = addr as *mut i16;
        let val: i16 = *ptr;
        val as i32
    }
}

fn poke8(addr: i32, value: i32) {
    unsafe {
        let ptr: *mut i8 = addr as *mut i8;
        let val: i8 = value as i8;
        *ptr = val;
    }
}

fn peek8(addr: i32) -> i32 {
    unsafe {
        let ptr: *mut i8 = addr as *mut i8;
        let val: i8 = *ptr;
        val as i32
    }
}

fn write_vdp_ctrl(value: i32) {
    poke16(VDP_CTRL, value);
}

fn write_vdp_data(value: i32) {
    poke16(VDP_DATA, value);
}

fn read_vdp_status() -> i32 {
    peek16(VDP_CTRL)
}

fn init_controller() {
    poke8(CTRL_CTRL, 0x40);
}

fn read_controller() -> i32 {
    // Set TH high and read: Up, Down, Left, Right, B, C
    poke8(CTRL_DATA, 0x40);
    let buttons: i32 = peek8(CTRL_DATA) & 0x3F;

    // Set TH low and read: Up, Down, 0, 0, A, Start
    poke8(CTRL_DATA, 0x00);
    let buttons2: i32 = peek8(CTRL_DATA);

    // Combine: bits 0-5 = UDLRBC, bit 6 = A, bit 7 = Start
    buttons | ((buttons2 & 0x10) << 2) | ((buttons2 & 0x20) << 2)
}

// ============================================================================
// PSG Sound Functions
// ============================================================================

fn psg_write(value: i32) {
    poke8(PSG_PORT, value);
}

fn psg_set_volume(channel: i32, volume: i32) {
    let cmd: i32 = 0x90 | (channel << 5) | (volume & 0x0F);
    psg_write(cmd);
}

fn psg_set_tone(channel: i32, freq: i32) {
    if freq == 0 {
        return;
    }

    // PSG clock = 3579545 Hz, divider = clock / (32 * freq)
    let mut divider: i32 = 3579545 / (32 * freq);
    if divider > 1023 {
        divider = 1023;
    }

    let ch: i32 = (channel & 0x03) << 5;
    let byte1: i32 = 0x80 | ch | (divider & 0x0F);
    let byte2: i32 = (divider >> 4) & 0x3F;

    psg_write(byte1);
    psg_write(byte2);
}

fn psg_init() {
    // Silence all 4 channels
    psg_set_volume(0, 15);
    psg_set_volume(1, 15);
    psg_set_volume(2, 15);
    psg_set_volume(3, 15);
}

fn psg_stop() {
    psg_set_volume(0, 15);
    psg_set_volume(1, 15);
    psg_set_volume(2, 15);
    psg_set_volume(3, 15);
}

// Sound effect: paddle hit (high-pitched blip)
fn sound_paddle_hit() {
    unsafe {
        psg_set_tone(0, 880); // A5
        psg_set_volume(0, 2); // Slightly quieter
        SOUND_TIMER = 4;
    }
}

// Sound effect: wall bounce (lower blip)
fn sound_wall_bounce() {
    unsafe {
        psg_set_tone(0, 440); // A4
        psg_set_volume(0, 4); // Medium volume
        SOUND_TIMER = 3;
    }
}

// Sound effect: score (longer descending tone)
fn sound_score() {
    unsafe {
        psg_set_tone(0, 220); // A3
        psg_set_volume(0, 0); // Max volume
        SOUND_TIMER = 15;
    }
}

fn sound_update() {
    unsafe {
        if SOUND_TIMER > 0 {
            SOUND_TIMER -= 1;
            if SOUND_TIMER == 0 {
                psg_stop();
            }
        }
    }
}

// ============================================================================
// VDP Functions
// ============================================================================

fn vdp_set_register(reg: i32, value: i32) {
    let cmd: i32 = 0x8000 | (reg << 8) | value;
    write_vdp_ctrl(cmd);
}

fn vdp_init() {
    vdp_set_register(0, 0x04);
    vdp_set_register(1, 0x44);
    vdp_set_register(2, 0x30); // Plane A at 0xC000
    vdp_set_register(3, 0x00);
    vdp_set_register(4, 0x07);
    vdp_set_register(5, 0x78); // Sprite table at 0xF000
    vdp_set_register(6, 0x00);
    vdp_set_register(7, 0x00); // Background = black
    vdp_set_register(10, 0xFF);
    vdp_set_register(11, 0x00);
    vdp_set_register(12, 0x81); // H40 mode
    vdp_set_register(13, 0x3F);
    vdp_set_register(15, 0x02); // Auto-increment 2
    vdp_set_register(16, 0x01);
    vdp_set_register(17, 0x00);
    vdp_set_register(18, 0x00);
}

fn vdp_set_write_address(address: i32) {
    let cmd1: i32 = 0x4000 | (address & 0x3FFF);
    let cmd2: i32 = (address >> 14) & 0x03;
    write_vdp_ctrl(cmd1);
    write_vdp_ctrl(cmd2);
}

fn vdp_wait_vblank() {
    let mut status: i32 = read_vdp_status();
    while (status & 0x08) == 0 {
        status = read_vdp_status();
    }
}

// ============================================================================
// Palette Setup
// ============================================================================

fn setup_palette() {
    write_vdp_ctrl(0xC000);
    write_vdp_ctrl(0x0000);
    write_vdp_data(0x0000); // Color 0 = black
    write_vdp_data(0x0EEE); // Color 1 = white
}

// ============================================================================
// Tile Loading
// ============================================================================

fn load_tiles() {
    let mut row: i32;

    // Tile 1: top of paddle
    vdp_set_write_address(0x0020);
    row = 0;
    while row < 8 {
        if row == 0 {
            write_vdp_data(0x1111);
            write_vdp_data(0x1111);
        } else {
            write_vdp_data(0x1000);
            write_vdp_data(0x0001);
        }
        row += 1;
    }

    // Tile 2: middle of paddle
    vdp_set_write_address(0x0040);
    row = 0;
    while row < 8 {
        write_vdp_data(0x1000);
        write_vdp_data(0x0001);
        row += 1;
    }

    // Tile 3: middle of paddle
    vdp_set_write_address(0x0060);
    row = 0;
    while row < 8 {
        write_vdp_data(0x1000);
        write_vdp_data(0x0001);
        row += 1;
    }

    // Tile 4: bottom of paddle
    vdp_set_write_address(0x0080);
    row = 0;
    while row < 8 {
        if row == 7 {
            write_vdp_data(0x1111);
            write_vdp_data(0x1111);
        } else {
            write_vdp_data(0x1000);
            write_vdp_data(0x0001);
        }
        row += 1;
    }

    // Tile 5: Center line segment
    vdp_set_write_address(0x00A0);
    row = 0;
    while row < 8 {
        if row == 0 || row == 1 || row == 4 || row == 5 {
            write_vdp_data(0x0001);
            write_vdp_data(0x1000);
        } else {
            write_vdp_data(0x0000);
            write_vdp_data(0x0000);
        }
        row += 1;
    }

    // Tile 6: Ball
    vdp_set_write_address(0x00C0);
    row = 0;
    while row < 8 {
        if row == 3 || row == 4 {
            write_vdp_data(0x0001);
            write_vdp_data(0x1000);
        } else {
            write_vdp_data(0x0000);
            write_vdp_data(0x0000);
        }
        row += 1;
    }

    // Tile 7: Solid white tile (for score digits)
    vdp_set_write_address(0x00E0);
    row = 0;
    while row < 8 {
        write_vdp_data(0x1111);
        write_vdp_data(0x1111);
        row += 1;
    }
}

// ============================================================================
// Draw Center Line
// ============================================================================

fn draw_center_line() {
    let mut y: i32 = 0;
    while y < 28 {
        let addr: i32 = 0xC000 + (y * 128) + (20 * 2);
        vdp_set_write_address(addr);
        write_vdp_data(0x0005); // Tile 5 (dashed line)
        y += 1;
    }
}

// ============================================================================
// Score Display
// ============================================================================

fn draw_digit(x: i32, y: i32, digit: i32) {
    let addr: i32 = 0xC000 + (y * 128) + (x * 2);

    if digit == 0 {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
    } else if digit == 1 {
        vdp_set_write_address(addr);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
    } else if digit == 2 {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
    } else if digit == 3 {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
    } else if digit == 4 {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
    } else if digit == 5 {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
    } else if digit == 6 {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
    } else if digit == 7 {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
    } else if digit == 8 {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
    } else if digit == 9 {
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
    }
}

fn draw_scores() {
    unsafe {
        // Player 1 score at tile position (8, 2)
        draw_digit(8, 2, PLAYER1_SCORE);
        // Player 2 score at tile position (28, 2)
        draw_digit(28, 2, PLAYER2_SCORE);
    }
}

// ============================================================================
// Letter Drawing
// ============================================================================

fn draw_letter(x: i32, y: i32, letter: i32) {
    let addr: i32 = 0xC000 + (y * 128) + (x * 2);

    if letter == 82 {
        // 'R'
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
    }
    if letter == 69 {
        // 'E'
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
    }
    if letter == 65 {
        // 'A'
        vdp_set_write_address(addr);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
    }
    if letter == 68 {
        // 'D'
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
    }
    if letter == 89 {
        // 'Y'
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
    }
    if letter == 63 {
        // '?'
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
    }
    if letter == 80 {
        // 'P'
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
    }
    if letter == 85 {
        // 'U'
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
    }
    if letter == 83 {
        // 'S'
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
    }
    if letter == 72 {
        // 'H'
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
    }
    if letter == 84 {
        // 'T'
        vdp_set_write_address(addr);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        write_vdp_data(0x0007);
        vdp_set_write_address(addr + 128);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 256);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 384);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
        vdp_set_write_address(addr + 512);
        write_vdp_data(0x0000);
        write_vdp_data(0x0007);
        write_vdp_data(0x0000);
    }
}

fn clear_text_area(x: i32, y: i32, width: i32) {
    let mut row: i32 = 0;
    while row < 5 {
        let addr: i32 = 0xC000 + ((y + row) * 128) + (x * 2);
        vdp_set_write_address(addr);
        let mut col: i32 = 0;
        while col < width {
            write_vdp_data(0x0000);
            col += 1;
        }
        row += 1;
    }
}

fn draw_ready_text() {
    // "READY?" centered on screen
    draw_letter(8, 10, 82); // R
    draw_letter(12, 10, 69); // E
    draw_letter(16, 10, 65); // A
    draw_letter(20, 10, 68); // D
    draw_letter(24, 10, 89); // Y
    draw_letter(28, 10, 63); // ?
}

fn draw_push_start_text() {
    // "PUSH START" centered on screen
    draw_letter(2, 17, 80); // P
    draw_letter(6, 17, 85); // U
    draw_letter(10, 17, 83); // S
    draw_letter(14, 17, 72); // H
    // space gap
    draw_letter(20, 17, 83); // S
    draw_letter(24, 17, 84); // T
    draw_letter(28, 17, 65); // A
    draw_letter(32, 17, 82); // R
    draw_letter(36, 17, 84); // T
}

fn clear_ready_screen() {
    clear_text_area(8, 10, 24); // READY?
    clear_text_area(2, 17, 38); // PUSH START
}

fn wait_for_start() {
    draw_ready_text();
    draw_push_start_text();

    // Wait for START to be released first
    let mut buttons: i32 = read_controller();
    while (buttons & 0x80) == 0 {
        vdp_wait_vblank();
        buttons = read_controller();
    }

    // Wait for START to be pressed
    while (buttons & 0x80) != 0 {
        vdp_wait_vblank();
        buttons = read_controller();
    }

    // Clear the ready screen
    clear_ready_screen();
}

// ============================================================================
// Sprite Functions
// ============================================================================

fn update_sprite(index: i32, x: i32, y: i32, size: i32, tile: i32) {
    let addr: i32 = 0xF000 + (index * 8);
    vdp_set_write_address(addr);
    write_vdp_data(y + 128);
    write_vdp_data((size << 8) | (index + 1));
    write_vdp_data(tile);
    write_vdp_data(x + 128);
}

fn clear_sprite(index: i32) {
    let addr: i32 = 0xF000 + (index * 8);
    vdp_set_write_address(addr);
    write_vdp_data(0);
    write_vdp_data(0);
    write_vdp_data(0);
    write_vdp_data(0);
}

// ============================================================================
// Game Logic
// ============================================================================

fn reset_ball() {
    unsafe {
        BALL_X = 156;
        BALL_Y = 108;
        if (FRAME_COUNT & 1) != 0 {
            BALL_DX = BALL_SPEED;
        } else {
            BALL_DX = 0 - BALL_SPEED;
        }
        BALL_DY = 2;
    }
}

fn update_ball() {
    unsafe {
        let paddle_width: i32 = 8;

        BALL_X += BALL_DX;
        BALL_Y += BALL_DY;

        // Bounce off top/bottom
        if BALL_Y < TOP_MARGIN {
            BALL_Y = TOP_MARGIN;
            BALL_DY = 0 - BALL_DY;
            sound_wall_bounce();
        }
        if BALL_Y > BOTTOM_MARGIN - BALL_SIZE {
            BALL_Y = BOTTOM_MARGIN - BALL_SIZE;
            BALL_DY = 0 - BALL_DY;
            sound_wall_bounce();
        }

        // Player 1 paddle collision
        if BALL_X < PLAYER1_X + paddle_width
            && BALL_X > PLAYER1_X - BALL_SIZE
            && BALL_Y + BALL_SIZE > PLAYER1_Y
            && BALL_Y < PLAYER1_Y + PADDLE_HEIGHT
        {
            BALL_X = PLAYER1_X + paddle_width;
            BALL_DX = 0 - BALL_DX;
            sound_paddle_hit();
        }

        // Player 2 paddle collision
        if BALL_X + BALL_SIZE > PLAYER2_X
            && BALL_X < PLAYER2_X + paddle_width
            && BALL_Y + BALL_SIZE > PLAYER2_Y
            && BALL_Y < PLAYER2_Y + PADDLE_HEIGHT
        {
            BALL_X = PLAYER2_X - BALL_SIZE;
            BALL_DX = 0 - BALL_DX;
            sound_paddle_hit();
        }

        // Scoring
        if BALL_X < LEFT_MARGIN {
            PLAYER2_SCORE += 1;
            sound_score();
            reset_ball();
        }
        if BALL_X > RIGHT_MARGIN {
            PLAYER1_SCORE += 1;
            sound_score();
            reset_ball();
        }
    }
}

fn update_paddles(buttons: i32) {
    unsafe {
        // Player 1: Up/Down buttons
        if (buttons & 0x01) == 0 {
            PLAYER1_Y -= PADDLE_SPEED;
        }
        if (buttons & 0x02) == 0 {
            PLAYER1_Y += PADDLE_SPEED;
        }

        // Clamp player 1
        if PLAYER1_Y < TOP_MARGIN {
            PLAYER1_Y = TOP_MARGIN;
        }
        if PLAYER1_Y > BOTTOM_MARGIN - PADDLE_HEIGHT {
            PLAYER1_Y = BOTTOM_MARGIN - PADDLE_HEIGHT;
        }

        // AI for player 2
        if BALL_X > 160 {
            if PLAYER2_Y + 16 < BALL_Y {
                PLAYER2_Y += 3;
            }
            if PLAYER2_Y + 16 > BALL_Y {
                PLAYER2_Y -= 3;
            }
        }

        // Clamp player 2
        if PLAYER2_Y < TOP_MARGIN {
            PLAYER2_Y = TOP_MARGIN;
        }
        if PLAYER2_Y > BOTTOM_MARGIN - PADDLE_HEIGHT {
            PLAYER2_Y = BOTTOM_MARGIN - PADDLE_HEIGHT;
        }
    }
}

// ============================================================================
// Rendering
// ============================================================================

fn render() {
    unsafe {
        // Paddle sprites: size 0x03 = 1 wide x 4 tall (8x32 pixels)
        update_sprite(0, PLAYER1_X, PLAYER1_Y, 0x03, 1);
        update_sprite(1, PLAYER2_X, PLAYER2_Y, 0x03, 1);
        // Ball sprite: size 0x00 = 1x1 (8x8 pixels)
        update_sprite(2, BALL_X, BALL_Y, 0x00, 6);
        // End sprite list
        clear_sprite(3);
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    unsafe {
        vdp_init();
        psg_init();
        init_controller();
        setup_palette();
        load_tiles();

        // Clear sprites
        let mut i: i32 = 0;
        while i < 80 {
            clear_sprite(i);
            i += 1;
        }

        // Wait for START button before starting game
        wait_for_start();

        // Draw static elements after ready screen is cleared
        draw_center_line();
        draw_scores();

        let mut last_p1_score: i32 = 0;
        let mut last_p2_score: i32 = 0;

        while GAME_RUNNING != 0 {
            vdp_wait_vblank();

            let buttons: i32 = read_controller();
            update_paddles(buttons);
            update_ball();
            render();
            sound_update();

            // Update score display when score changes
            if PLAYER1_SCORE != last_p1_score || PLAYER2_SCORE != last_p2_score {
                draw_scores();
                last_p1_score = PLAYER1_SCORE;
                last_p2_score = PLAYER2_SCORE;
            }

            FRAME_COUNT += 1;

            if PLAYER1_SCORE >= 10 || PLAYER2_SCORE >= 10 {
                GAME_RUNNING = 0;
            }
        }

        // Game over - infinite loop
        loop {
            vdp_wait_vblank();
        }
    }
}
