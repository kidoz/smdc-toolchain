/*
 * sfx_test.c - SMD sound driver SFX system test
 *
 * Cycles three sound effects while no music is playing:
 * - laser: descending PSG sweep
 * - blip:  short PSG chirp
 * - boom:  decaying noise burst
 *
 * Each effect is 2 bytes per frame: { note, volume }.
 *
 * Compile:
 *   smdc sdk/c/examples/sfx_test.c -o sfx_test.bin -t rom -I sdk/c/include
 */

#include <smd/vdp.h>

#define SND_IMPLEMENTATION
#include <smd/snd.h>
#include <smd/snd_impl.h>

/* Descending laser sweep on a PSG tone channel (12 frames) */
static unsigned char laser_data[24] = {
    SND_NOTE(6, SND_NOTE_B),  127,
    SND_NOTE(6, SND_NOTE_G),  120,
    SND_NOTE(6, SND_NOTE_E),  112,
    SND_NOTE(6, SND_NOTE_C),  104,
    SND_NOTE(5, SND_NOTE_A),  96,
    SND_NOTE(5, SND_NOTE_F),  88,
    SND_NOTE(5, SND_NOTE_D),  72,
    SND_NOTE(4, SND_NOTE_B),  56,
    SND_NOTE(4, SND_NOTE_G),  40,
    SND_NOTE(4, SND_NOTE_E),  24,
    SND_NOTE(4, SND_NOTE_C),  12,
    SND_NOTE_OFF,             0
};

/* Short UI blip (4 frames) */
static unsigned char blip_data[8] = {
    SND_NOTE(5, SND_NOTE_E),  100,
    SND_NOTE(6, SND_NOTE_E),  100,
    SND_NOTE_NONE,            60,
    SND_NOTE_OFF,             0
};

/* Explosion: white noise with decaying volume (24 frames) */
static unsigned char boom_data[48] = {
    6, 127,  SND_NOTE_NONE, 127, SND_NOTE_NONE, 120, SND_NOTE_NONE, 116,
    SND_NOTE_NONE, 110, SND_NOTE_NONE, 104, SND_NOTE_NONE, 98,  SND_NOTE_NONE, 92,
    SND_NOTE_NONE, 84,  SND_NOTE_NONE, 76,  SND_NOTE_NONE, 68,  SND_NOTE_NONE, 60,
    SND_NOTE_NONE, 54,  SND_NOTE_NONE, 48,  SND_NOTE_NONE, 42,  SND_NOTE_NONE, 36,
    SND_NOTE_NONE, 30,  SND_NOTE_NONE, 24,  SND_NOTE_NONE, 18,  SND_NOTE_NONE, 12,
    SND_NOTE_NONE, 8,   SND_NOTE_NONE, 4,   SND_NOTE_NONE, 2,   SND_NOTE_OFF,  0
};

static struct SndSfx laser_sfx;
static struct SndSfx blip_sfx;
static struct SndSfx boom_sfx;

static void init_sfx(void) {
    laser_sfx.channel = SND_CH_PSG1;
    laser_sfx.priority = SND_PRIORITY_NORMAL;
    laser_sfx.length = 12;
    laser_sfx.data = laser_data;

    blip_sfx.channel = SND_CH_ANY;
    blip_sfx.priority = SND_PRIORITY_LOW;
    blip_sfx.length = 4;
    blip_sfx.data = blip_data;

    boom_sfx.channel = SND_CH_NOISE;
    boom_sfx.priority = SND_PRIORITY_HIGH;
    boom_sfx.length = 24;
    boom_sfx.data = boom_data;
}

void main(void) {
    int frame;
    int which;

    vdp_init();
    vdp_set_color(0, 0x0000);

    snd_init();
    init_sfx();

    frame = 0;
    which = 0;

    while (1) {
        vdp_vsync();
        snd_update();

        /* Trigger the next effect every second */
        frame++;
        if (frame >= 60) {
            frame = 0;
            if (which == 0) {
                snd_play_sfx(&laser_sfx);
            } else if (which == 1) {
                snd_play_sfx(&blip_sfx);
            } else {
                snd_play_sfx(&boom_sfx);
            }
            which++;
            if (which >= 3) which = 0;
        }
    }
}
