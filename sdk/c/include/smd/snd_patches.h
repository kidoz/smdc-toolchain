/*
 * smd/snd_patches.h - Preset FM Patches for SMD Sound Driver
 *
 * Ready-to-use FM instrument patches organized by category.
 * All patches use byte arrays for compiler compatibility.
 * Cast to (struct SndFmPatch *) when using with snd_fm_set_patch().
 */

#ifndef SND_PATCHES_H
#define SND_PATCHES_H

/*
 * Patch format (30 bytes):
 * [0]  algo_fb      - Algorithm (0-7) | Feedback (0-7) << 3
 * [1]  pan_ams_pms  - L/R/AMS/PMS (0xC0 = center)
 * [2-8]   Op1: dt_mul, tl, rs_ar, am_d1r, d2r, d1l_rr, ssg_eg
 * [9-15]  Op2: dt_mul, tl, rs_ar, am_d1r, d2r, d1l_rr, ssg_eg
 * [16-22] Op3: dt_mul, tl, rs_ar, am_d1r, d2r, d1l_rr, ssg_eg
 * [23-29] Op4: dt_mul, tl, rs_ar, am_d1r, d2r, d1l_rr, ssg_eg
 */

/* ============================================================================
 * BASS Patches
 * ============================================================================ */

/* Punchy synth bass - great for electronic music */
static const unsigned char SND_PATCH_BASS_SYNTH[30] = {
    0x2C, 0xC0,  /* algo=4, fb=5, pan=LR */
    0x01, 32, 0x1F, 0x0E, 8,  0x37, 0,  /* Op1 */
    0x02, 8,  0x1F, 0x0A, 5,  0x29, 0,  /* Op2 */
    0x00, 28, 0x1F, 0x0C, 6,  0x27, 0,  /* Op3 */
    0x01, 5,  0x1F, 0x08, 4,  0x1A, 0   /* Op4 */
};

/* Electric bass - warm, round tone */
static const unsigned char SND_PATCH_BASS_ELECTRIC[30] = {
    0x18, 0xC0,  /* algo=0, fb=3 */
    0x01, 40, 0x1C, 0x08, 4,  0x26, 0,
    0x01, 18, 0x1F, 0x0A, 6,  0x38, 0,
    0x02, 24, 0x1A, 0x08, 5,  0x27, 0,
    0x01, 8,  0x1F, 0x06, 4,  0x19, 0
};

/* Slap bass - percussive attack */
static const unsigned char SND_PATCH_BASS_SLAP[30] = {
    0x34, 0xC0,  /* algo=4, fb=6 */
    0x02, 38, 0x3F, 0x12, 12, 0x5A, 0,
    0x01, 10, 0x3F, 0x0E, 8,  0x3B, 0,
    0x03, 42, 0x3F, 0x14, 14, 0x69, 0,
    0x01, 6,  0x3F, 0x0C, 6,  0x2C, 0
};

/* ============================================================================
 * PAD Patches (Sustained sounds)
 * ============================================================================ */

/* Warm string pad */
static const unsigned char SND_PATCH_PAD_STRINGS[30] = {
    0x17, 0xC3,  /* algo=7, fb=2, pms=3 */
    0x31, 18, 0x10, 0x02, 1,  0x23, 0,
    0x42, 20, 0x10, 0x02, 1,  0x23, 0,
    0x03, 22, 0x10, 0x02, 1,  0x23, 0,
    0x71, 20, 0x10, 0x02, 1,  0x23, 0
};

/* Soft synth pad */
static const unsigned char SND_PATCH_PAD_SOFT[30] = {
    0x0F, 0xC2,  /* algo=7, fb=1, pms=2 */
    0x01, 22, 0x0E, 0x03, 2,  0x34, 0,
    0x02, 24, 0x0E, 0x03, 2,  0x34, 0,
    0x04, 28, 0x0E, 0x03, 2,  0x34, 0,
    0x01, 20, 0x0E, 0x03, 2,  0x34, 0
};

/* Brass pad */
static const unsigned char SND_PATCH_PAD_BRASS[30] = {
    0x24, 0xC0,  /* algo=4, fb=4 */
    0x01, 35, 0x14, 0x06, 4,  0x45, 0,
    0x01, 12, 0x18, 0x05, 3,  0x36, 0,
    0x01, 38, 0x14, 0x06, 4,  0x45, 0,
    0x01, 10, 0x18, 0x05, 3,  0x36, 0
};

/* ============================================================================
 * LEAD Patches
 * ============================================================================ */

/* Bright synth lead */
static const unsigned char SND_PATCH_LEAD_BRIGHT[30] = {
    0x35, 0xC5,  /* algo=5, fb=6, pms=5 */
    0x33, 36, 0x5F, 0x0A, 5,  0x36, 0,
    0x01, 10, 0x5F, 0x06, 3,  0x28, 0,
    0x02, 14, 0x5F, 0x07, 4,  0x28, 0,
    0x04, 14, 0x5F, 0x08, 4,  0x38, 0
};

/* Square wave lead */
static const unsigned char SND_PATCH_LEAD_SQUARE[30] = {
    0x07, 0xC0,  /* algo=7, fb=0 */
    0x01, 16, 0x1F, 0x04, 2,  0x28, 0,
    0x02, 16, 0x1F, 0x04, 2,  0x28, 0,
    0x04, 20, 0x1F, 0x04, 2,  0x28, 0,
    0x08, 24, 0x1F, 0x04, 2,  0x28, 0
};

/* Distortion lead */
static const unsigned char SND_PATCH_LEAD_DIST[30] = {
    0x3D, 0xC0,  /* algo=5, fb=7 */
    0x01, 28, 0x9F, 0x0C, 6,  0x48, 0,
    0x32, 8,  0x9F, 0x08, 4,  0x2A, 0,
    0x03, 10, 0x9F, 0x09, 5,  0x39, 0,
    0x01, 12, 0x9F, 0x0A, 5,  0x39, 0
};

/* ============================================================================
 * ARPEGGIO / PLUCK Patches
 * ============================================================================ */

/* Pluck synth - good for arpeggios */
static const unsigned char SND_PATCH_PLUCK[30] = {
    0x24, 0xC0,  /* algo=4, fb=4 */
    0x02, 40, 0x9F, 0x16, 18, 0x6C, 0,
    0x01, 14, 0x9F, 0x14, 15, 0x5E, 0,
    0x03, 44, 0x9F, 0x18, 18, 0x7C, 0,
    0x01, 12, 0x9F, 0x12, 14, 0x4E, 0
};

/* Bell / Marimba */
static const unsigned char SND_PATCH_BELL[30] = {
    0x1C, 0xC0,  /* algo=4, fb=3 */
    0x09, 48, 0xFF, 0x0A, 4,  0x48, 0,
    0x01, 18, 0xFF, 0x08, 3,  0x3A, 0,
    0x0D, 52, 0xFF, 0x0C, 5,  0x57, 0,
    0x01, 16, 0xFF, 0x07, 3,  0x3B, 0
};

/* Piano-like */
static const unsigned char SND_PATCH_PIANO[30] = {
    0x2C, 0xC0,  /* algo=4, fb=5 */
    0x01, 38, 0x5F, 0x0C, 6,  0x48, 0,
    0x04, 16, 0x5F, 0x08, 4,  0x3A, 0,
    0x01, 40, 0x5F, 0x0E, 7,  0x57, 0,
    0x01, 12, 0x5F, 0x06, 3,  0x2B, 0
};

/* ============================================================================
 * DRUM Patches
 * ============================================================================ */

/* Kick drum */
static const unsigned char SND_PATCH_DRUM_KICK[30] = {
    0x3C, 0xC0,  /* algo=4, fb=7 */
    0x01, 18, 0x1F, 0x1F, 31, 0xFF, 0,
    0x00, 0,  0x1F, 0x12, 10, 0x8A, 0,
    0x01, 22, 0x1F, 0x1C, 20, 0xCC, 0,
    0x00, 4,  0x1F, 0x0F, 8,  0x6A, 0
};

/* Snare drum */
static const unsigned char SND_PATCH_DRUM_SNARE[30] = {
    0x3F, 0xC0,  /* algo=7, fb=7 */
    0x77, 26, 0x1F, 0x14, 15, 0x8C, 0,
    0x31, 16, 0x1F, 0x12, 12, 0x7B, 0,
    0x59, 28, 0x1F, 0x16, 16, 0x9C, 0,
    0x01, 14, 0x1F, 0x10, 10, 0x6A, 0
};

/* Hi-hat (closed) */
static const unsigned char SND_PATCH_DRUM_HIHAT[30] = {
    0x3F, 0xC0,  /* algo=7, fb=7 */
    0x7E, 32, 0xFF, 0x1C, 24, 0xCF, 0,
    0x59, 28, 0xFF, 0x1A, 22, 0xBF, 0,
    0x3B, 30, 0xFF, 0x1E, 26, 0xDF, 0,
    0x01, 20, 0xFF, 0x18, 20, 0xAF, 0
};

/* Tom drum */
static const unsigned char SND_PATCH_DRUM_TOM[30] = {
    0x2C, 0xC0,  /* algo=4, fb=5 */
    0x01, 24, 0x1F, 0x1A, 18, 0xAC, 0,
    0x01, 8,  0x1F, 0x14, 14, 0x8B, 0,
    0x02, 28, 0x1F, 0x18, 16, 0x9B, 0,
    0x01, 6,  0x1F, 0x12, 12, 0x7A, 0
};

/* ============================================================================
 * SPECIAL Patches
 * ============================================================================ */

/* Organ */
static const unsigned char SND_PATCH_ORGAN[30] = {
    0x07, 0xC0,  /* algo=7, fb=0 */
    0x01, 20, 0x1F, 0x00, 0,  0x08, 0,
    0x02, 22, 0x1F, 0x00, 0,  0x08, 0,
    0x04, 26, 0x1F, 0x00, 0,  0x08, 0,
    0x08, 30, 0x1F, 0x00, 0,  0x08, 0
};

/* Flute */
static const unsigned char SND_PATCH_FLUTE[30] = {
    0x15, 0xC3,  /* algo=5, fb=2, pms=3 */
    0x01, 45, 0x14, 0x04, 2,  0x26, 0,
    0x01, 18, 0x18, 0x03, 2,  0x27, 0,
    0x02, 22, 0x16, 0x03, 2,  0x27, 0,
    0x01, 16, 0x1A, 0x02, 1,  0x18, 0
};

/* Laser SFX */
static const unsigned char SND_PATCH_SFX_LASER[30] = {
    0x3F, 0xC0,  /* algo=7, fb=7 */
    0x01, 20, 0xFF, 0x00, 8,  0x48, 0,
    0x72, 18, 0xFF, 0x00, 10, 0x57, 0,
    0x04, 22, 0xFF, 0x00, 12, 0x66, 0,
    0x38, 24, 0xFF, 0x00, 14, 0x75, 0
};

/* Explosion SFX */
static const unsigned char SND_PATCH_SFX_EXPLOSION[30] = {
    0x3F, 0xC0,  /* algo=7, fb=7 */
    0x7F, 16, 0x1F, 0x08, 4,  0x26, 0,
    0x5D, 18, 0x1F, 0x0A, 5,  0x35, 0,
    0x3B, 20, 0x1F, 0x0C, 6,  0x44, 0,
    0x00, 8,  0x1F, 0x06, 3,  0x27, 0
};

#endif /* SND_PATCHES_H */
