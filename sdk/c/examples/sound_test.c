/*
 * sound_test.c - Simple YM2612 sound test
 *
 * Tests basic FM sound output with a simple beep pattern
 */

#include <smd/vdp.h>

/* YM2612 ports */
#define YM_ADDR0  (*(volatile unsigned char *)0xA04000)
#define YM_DATA0  (*(volatile unsigned char *)0xA04001)
#define YM_ADDR1  (*(volatile unsigned char *)0xA04002)
#define YM_DATA1  (*(volatile unsigned char *)0xA04003)

/* Small delay */
static void small_delay(void) {
    volatile int i;
    for (i = 0; i < 10; i++);
}

/* Wait for YM2612 to be ready */
static void ym_wait(void) {
    volatile unsigned char status;
    int timeout = 1000;
    do {
        status = YM_ADDR0;
        timeout--;
    } while ((status & 0x80) && timeout > 0);
    small_delay();
}

/* Write to port 0 */
static void ym_write0(int reg, int val) {
    ym_wait();
    YM_ADDR0 = reg;
    ym_wait();
    YM_DATA0 = val;
}

/* Initialize YM2612 */
static void ym_init(void) {
    int ch;

    /* Disable LFO */
    ym_write0(0x22, 0x00);

    /* Disable timers */
    ym_write0(0x27, 0x00);

    /* Key off all channels */
    for (ch = 0; ch < 3; ch++) {
        ym_write0(0x28, ch);       /* Ch 1-3 */
        ym_write0(0x28, ch + 4);   /* Ch 4-6 */
    }
}

/* Set up a simple instrument on channel 1 (index 0) */
static void setup_simple_instrument(void) {
    /* Algorithm 7 (all carriers) with feedback 0 */
    ym_write0(0xB0, 0x07);

    /* Stereo: both L and R */
    ym_write0(0xB4, 0xC0);

    /* Operator 1 (offset +0) */
    ym_write0(0x30, 0x01);  /* DT=0, MUL=1 */
    ym_write0(0x40, 0x10);  /* TL=16 (volume) */
    ym_write0(0x50, 0x1F);  /* RS=0, AR=31 (fast attack) */
    ym_write0(0x60, 0x00);  /* AM=0, D1R=0 */
    ym_write0(0x70, 0x00);  /* D2R=0 */
    ym_write0(0x80, 0x0F);  /* D1L=0, RR=15 */
    ym_write0(0x90, 0x00);  /* SSG-EG=0 */

    /* Operator 2 (offset +8) - silence */
    ym_write0(0x38, 0x01);
    ym_write0(0x48, 0x7F);  /* TL=127 (silent) */
    ym_write0(0x58, 0x1F);
    ym_write0(0x68, 0x00);
    ym_write0(0x78, 0x00);
    ym_write0(0x88, 0x0F);
    ym_write0(0x98, 0x00);

    /* Operator 3 (offset +4) - silence */
    ym_write0(0x34, 0x01);
    ym_write0(0x44, 0x7F);
    ym_write0(0x54, 0x1F);
    ym_write0(0x64, 0x00);
    ym_write0(0x74, 0x00);
    ym_write0(0x84, 0x0F);
    ym_write0(0x94, 0x00);

    /* Operator 4 (offset +12) - silence */
    ym_write0(0x3C, 0x01);
    ym_write0(0x4C, 0x7F);
    ym_write0(0x5C, 0x1F);
    ym_write0(0x6C, 0x00);
    ym_write0(0x7C, 0x00);
    ym_write0(0x8C, 0x0F);
    ym_write0(0x9C, 0x00);
}

/* Set frequency on channel 1 */
static void set_frequency(int block, int fnum) {
    /* Write high byte first (with block) */
    ym_write0(0xA4, ((block & 7) << 3) | ((fnum >> 8) & 7));
    /* Then low byte */
    ym_write0(0xA0, fnum & 0xFF);
}

/* Key on channel 1 (all operators) */
static void key_on(void) {
    ym_write0(0x28, 0xF0);  /* F0 = all 4 ops on, channel 0 */
}

/* Key off channel 1 */
static void key_off(void) {
    ym_write0(0x28, 0x00);  /* Channel 0, all ops off */
}

/* Delay loop */
static void delay(int frames) {
    int i;
    for (i = 0; i < frames; i++) {
        vdp_vsync();
    }
}

/* Test with very different pitches - same note (A) at different octaves */
static int notes[8] = {
    1081,  /* A */
    1081,  /* A */
    1081,  /* A */
    1081,  /* A */
    1081,  /* A */
    1081,  /* A */
    1081,  /* A */
    1081   /* A */
};
/* Blocks 2-6 give very different octaves */
static int blocks[8] = { 2, 3, 4, 5, 6, 5, 4, 3 };

void main(void) {
    int i;

    /* Initialize */
    vdp_init();
    vdp_set_color(0, 0x000E);  /* Blue background */

    ym_init();
    setup_simple_instrument();

    /* Play scale repeatedly */
    while (1) {
        for (i = 0; i < 8; i++) {
            set_frequency(blocks[i], notes[i]);
            key_on();
            delay(15);  /* ~0.25 seconds */
            key_off();
            delay(5);   /* Short gap */
        }
        delay(30);  /* Pause between scales */
    }
}
