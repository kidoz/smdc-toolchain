/*
 * smd/patches_retrowave.h - Retrowave/Synthwave FM patches for YM2612
 *
 * Pre-defined instrument patches optimized for retrowave music:
 * - Punchy synth bass
 * - Warm analog-style pads
 * - Bright cutting leads
 * - Plucky arpeggios
 * - FM drums
 */

#ifndef SMD_PATCHES_RETROWAVE_H
#define SMD_PATCHES_RETROWAVE_H

#include <smd/ym2612.h>

/* ========================================================================== */
/* Patch Macros (for readability)                                             */
/* ========================================================================== */

/* DT_MUL: detune (-3 to +3), multiply (0-15) */
#define P_DM(dt, mul)   ((((dt) & 0x07) << 4) | ((mul) & 0x0F))

/* TL: total level (0=loud, 127=silent) */
#define P_TL(tl)        ((tl) & 0x7F)

/* RS_AR: rate scale (0-3), attack rate (0-31) */
#define P_RA(rs, ar)    ((((rs) & 0x03) << 6) | ((ar) & 0x1F))

/* AM_D1R: AM enable (0-1), decay 1 rate (0-31) */
#define P_AD(am, d1r)   ((((am) & 0x01) << 7) | ((d1r) & 0x1F))

/* D2R: decay 2 rate (0-31) */
#define P_D2(d2r)       ((d2r) & 0x1F)

/* D1L_RR: sustain level (0-15), release rate (0-15) */
#define P_SR(d1l, rr)   ((((d1l) & 0x0F) << 4) | ((rr) & 0x0F))

/* ALGO_FB: algorithm (0-7), feedback (0-7) */
#define P_AF(algo, fb)  ((((fb) & 0x07) << 3) | ((algo) & 0x07))

/* PAN_AMS_PMS: L, R, AMS (0-3), PMS (0-7) */
#define P_PAP(l, r, ams, pms)  ((((l) & 1) << 7) | (((r) & 1) << 6) | (((ams) & 3) << 4) | ((pms) & 7))

/* ========================================================================== */
/* SYNTH BASS - Deep, punchy bass for driving basslines                       */
/* ========================================================================== */

static const struct YM_Patch patch_synth_bass = {
    P_AF(4, 5),             /* Algorithm 4 (two FM pairs), feedback 5 */
    P_PAP(1, 1, 0, 0),      /* Center pan, no LFO */
    {
        /* Op1 - Modulator for bass body */
        { P_DM(0, 1), P_TL(35), P_RA(0, 31), P_AD(0, 14), P_D2(8), P_SR(3, 7), 0 },
        /* Op2 - Carrier for bass fundamental */
        { P_DM(0, 2), P_TL(8),  P_RA(0, 31), P_AD(0, 10), P_D2(5), P_SR(2, 9), 0 },
        /* Op3 - Modulator for sub harmonics */
        { P_DM(0, 0), P_TL(30), P_RA(0, 31), P_AD(0, 12), P_D2(6), P_SR(2, 7), 0 },
        /* Op4 - Carrier for sub bass */
        { P_DM(0, 1), P_TL(5),  P_RA(0, 31), P_AD(0, 8),  P_D2(4), P_SR(1, 10), 0 },
    }
};

/* ========================================================================== */
/* SYNTH BASS 2 - Rounder, warmer bass                                        */
/* ========================================================================== */

static const struct YM_Patch patch_synth_bass2 = {
    P_AF(2, 4),             /* Algorithm 2, feedback 4 */
    P_PAP(1, 1, 0, 0),      /* Center pan */
    {
        { P_DM(0, 1), P_TL(40), P_RA(0, 31), P_AD(0, 12), P_D2(6), P_SR(2, 6), 0 },
        { P_DM(0, 2), P_TL(25), P_RA(0, 31), P_AD(0, 10), P_D2(5), P_SR(2, 7), 0 },
        { P_DM(0, 1), P_TL(20), P_RA(0, 31), P_AD(0, 8),  P_D2(4), P_SR(2, 8), 0 },
        { P_DM(0, 1), P_TL(6),  P_RA(0, 31), P_AD(0, 6),  P_D2(3), P_SR(1, 10), 0 },
    }
};

/* ========================================================================== */
/* WARM PAD - Lush, sustained pad for chords                                  */
/* ========================================================================== */

static const struct YM_Patch patch_warm_pad = {
    P_AF(7, 2),             /* Algorithm 7 (all carriers), feedback 2 */
    P_PAP(1, 1, 1, 3),      /* Center, light AMS, medium PMS for movement */
    {
        /* Op1 - Fundamental with slight detune */
        { P_DM(3, 1), P_TL(18), P_RA(0, 18), P_AD(0, 2), P_D2(1), P_SR(2, 3), 0 },
        /* Op2 - Octave with opposite detune */
        { P_DM(4, 2), P_TL(20), P_RA(0, 18), P_AD(0, 2), P_D2(1), P_SR(2, 3), 0 },
        /* Op3 - Fifth */
        { P_DM(0, 3), P_TL(24), P_RA(0, 18), P_AD(0, 2), P_D2(1), P_SR(2, 3), 0 },
        /* Op4 - Sub octave */
        { P_DM(7, 1), P_TL(22), P_RA(0, 18), P_AD(0, 2), P_D2(1), P_SR(2, 3), 0 },
    }
};

/* ========================================================================== */
/* STRING PAD - Orchestral-style sustained strings                            */
/* ========================================================================== */

static const struct YM_Patch patch_string_pad = {
    P_AF(5, 3),             /* Algorithm 5 */
    P_PAP(1, 1, 1, 4),      /* Moderate vibrato */
    {
        { P_DM(3, 2), P_TL(32), P_RA(0, 20), P_AD(1, 4), P_D2(2), P_SR(3, 4), 0 },
        { P_DM(0, 1), P_TL(14), P_RA(0, 22), P_AD(0, 3), P_D2(2), P_SR(2, 5), 0 },
        { P_DM(4, 4), P_TL(16), P_RA(0, 22), P_AD(0, 3), P_D2(2), P_SR(2, 5), 0 },
        { P_DM(0, 1), P_TL(12), P_RA(0, 22), P_AD(0, 3), P_D2(2), P_SR(2, 5), 0 },
    }
};

/* ========================================================================== */
/* BRIGHT LEAD - Cutting, bright lead synth                                   */
/* ========================================================================== */

static const struct YM_Patch patch_bright_lead = {
    P_AF(5, 6),             /* Algorithm 5, high feedback for brightness */
    P_PAP(1, 1, 0, 5),      /* Vibrato on PMS */
    {
        /* Op1 - Modulator with high harmonics */
        { P_DM(3, 3), P_TL(38), P_RA(1, 31), P_AD(0, 10), P_D2(5), P_SR(3, 6), 0 },
        /* Op2 - Carrier */
        { P_DM(0, 1), P_TL(10), P_RA(1, 31), P_AD(0, 6),  P_D2(3), P_SR(2, 8), 0 },
        /* Op3 - Second carrier for fullness */
        { P_DM(0, 2), P_TL(14), P_RA(1, 31), P_AD(0, 7),  P_D2(4), P_SR(2, 8), 0 },
        /* Op4 - Third carrier */
        { P_DM(0, 4), P_TL(16), P_RA(1, 31), P_AD(0, 8),  P_D2(4), P_SR(3, 8), 0 },
    }
};

/* ========================================================================== */
/* SOFT LEAD - Warmer, rounder lead                                           */
/* ========================================================================== */

static const struct YM_Patch patch_soft_lead = {
    P_AF(4, 4),             /* Algorithm 4 */
    P_PAP(1, 1, 0, 4),      /* Medium vibrato */
    {
        { P_DM(3, 2), P_TL(35), P_RA(0, 28), P_AD(0, 8), P_D2(4), P_SR(3, 6), 0 },
        { P_DM(0, 1), P_TL(12), P_RA(0, 31), P_AD(0, 5), P_D2(3), P_SR(2, 8), 0 },
        { P_DM(4, 2), P_TL(38), P_RA(0, 28), P_AD(0, 8), P_D2(4), P_SR(3, 6), 0 },
        { P_DM(0, 1), P_TL(10), P_RA(0, 31), P_AD(0, 5), P_D2(3), P_SR(2, 8), 0 },
    }
};

/* ========================================================================== */
/* PLUCK SYNTH - Short, plucky sound for arpeggios                            */
/* ========================================================================== */

static const struct YM_Patch patch_pluck_arp = {
    P_AF(4, 4),             /* Algorithm 4 */
    P_PAP(1, 1, 0, 0),      /* No LFO needed for plucks */
    {
        { P_DM(0, 2), P_TL(42), P_RA(2, 31), P_AD(0, 22), P_D2(18), P_SR(6, 12), 0 },
        { P_DM(0, 1), P_TL(16), P_RA(2, 31), P_AD(0, 20), P_D2(15), P_SR(5, 14), 0 },
        { P_DM(0, 3), P_TL(45), P_RA(2, 31), P_AD(0, 24), P_D2(18), P_SR(7, 12), 0 },
        { P_DM(0, 1), P_TL(14), P_RA(2, 31), P_AD(0, 18), P_D2(14), P_SR(4, 14), 0 },
    }
};

/* ========================================================================== */
/* BELL/CHIME - Bell-like tone for accents                                    */
/* ========================================================================== */

static const struct YM_Patch patch_bell = {
    P_AF(4, 0),             /* Algorithm 4, no feedback */
    P_PAP(1, 1, 0, 0),
    {
        { P_DM(0, 14), P_TL(50), P_RA(3, 31), P_AD(0, 10), P_D2(4), P_SR(4, 6), 0 },
        { P_DM(0, 1),  P_TL(12), P_RA(2, 31), P_AD(0, 6),  P_D2(2), P_SR(2, 8), 0 },
        { P_DM(0, 7),  P_TL(55), P_RA(3, 31), P_AD(0, 12), P_D2(5), P_SR(5, 6), 0 },
        { P_DM(0, 1),  P_TL(10), P_RA(2, 31), P_AD(0, 5),  P_D2(2), P_SR(2, 8), 0 },
    }
};

/* ========================================================================== */
/* FM KICK DRUM - Punchy low kick                                             */
/* ========================================================================== */

static const struct YM_Patch patch_fm_kick = {
    P_AF(4, 7),             /* Algorithm 4, max feedback for punch */
    P_PAP(1, 1, 0, 0),
    {
        /* High feedback modulator for initial attack */
        { P_DM(0, 1), P_TL(20), P_RA(0, 31), P_AD(0, 31), P_D2(31), P_SR(15, 15), 0 },
        /* Low carrier for boom */
        { P_DM(0, 0), P_TL(0),  P_RA(0, 31), P_AD(0, 18), P_D2(10), P_SR(8, 10), 0 },
        { P_DM(0, 1), P_TL(25), P_RA(0, 31), P_AD(0, 28), P_D2(20), P_SR(12, 12), 0 },
        { P_DM(0, 0), P_TL(5),  P_RA(0, 31), P_AD(0, 15), P_D2(8),  P_SR(6, 10), 0 },
    }
};

/* ========================================================================== */
/* FM SNARE DRUM - Crisp snare with noise                                     */
/* ========================================================================== */

static const struct YM_Patch patch_fm_snare = {
    P_AF(7, 7),             /* Algorithm 7 (all carriers), max feedback for noise */
    P_PAP(1, 1, 0, 0),
    {
        { P_DM(7, 7), P_TL(28), P_RA(0, 31), P_AD(0, 20), P_D2(15), P_SR(8, 12), 0 },
        { P_DM(3, 1), P_TL(18), P_RA(0, 31), P_AD(0, 18), P_D2(12), P_SR(7, 11), 0 },
        { P_DM(5, 9), P_TL(30), P_RA(0, 31), P_AD(0, 22), P_D2(16), P_SR(9, 12), 0 },
        { P_DM(0, 1), P_TL(15), P_RA(0, 31), P_AD(0, 16), P_D2(10), P_SR(6, 10), 0 },
    }
};

/* ========================================================================== */
/* FM HI-HAT - Metallic hi-hat                                                */
/* ========================================================================== */

static const struct YM_Patch patch_fm_hihat = {
    P_AF(7, 7),             /* All carriers, max feedback */
    P_PAP(1, 1, 0, 0),
    {
        { P_DM(7, 14), P_TL(35), P_RA(0, 31), P_AD(0, 28), P_D2(20), P_SR(12, 14), 0 },
        { P_DM(3, 9),  P_TL(38), P_RA(0, 31), P_AD(0, 26), P_D2(18), P_SR(11, 14), 0 },
        { P_DM(5, 13), P_TL(40), P_RA(0, 31), P_AD(0, 30), P_D2(22), P_SR(13, 14), 0 },
        { P_DM(0, 7),  P_TL(32), P_RA(0, 31), P_AD(0, 24), P_D2(16), P_SR(10, 14), 0 },
    }
};

/* ========================================================================== */
/* FM TOM - Tuned tom drum                                                    */
/* ========================================================================== */

static const struct YM_Patch patch_fm_tom = {
    P_AF(4, 5),
    P_PAP(1, 1, 0, 0),
    {
        { P_DM(0, 2), P_TL(30), P_RA(0, 31), P_AD(0, 20), P_D2(12), P_SR(8, 10), 0 },
        { P_DM(0, 1), P_TL(8),  P_RA(0, 31), P_AD(0, 14), P_D2(8),  P_SR(5, 10), 0 },
        { P_DM(0, 3), P_TL(35), P_RA(0, 31), P_AD(0, 22), P_D2(14), P_SR(9, 10), 0 },
        { P_DM(0, 1), P_TL(6),  P_RA(0, 31), P_AD(0, 12), P_D2(6),  P_SR(4, 10), 0 },
    }
};

/* ========================================================================== */
/* Instrument Array - Index for songs                                         */
/* ========================================================================== */

#define INST_BASS       0
#define INST_BASS2      1
#define INST_PAD        2
#define INST_STRINGS    3
#define INST_LEAD       4
#define INST_LEAD_SOFT  5
#define INST_ARP        6
#define INST_BELL       7
#define INST_KICK       8
#define INST_SNARE      9
#define INST_HIHAT      10
#define INST_TOM        11

#define RETROWAVE_INSTRUMENT_COUNT 12

static const struct YM_Patch retrowave_instruments[RETROWAVE_INSTRUMENT_COUNT] = {
    /* 0 */ { P_AF(4, 5), P_PAP(1, 1, 0, 0), {{ P_DM(0, 1), P_TL(35), P_RA(0, 31), P_AD(0, 14), P_D2(8), P_SR(3, 7), 0 }, { P_DM(0, 2), P_TL(8), P_RA(0, 31), P_AD(0, 10), P_D2(5), P_SR(2, 9), 0 }, { P_DM(0, 0), P_TL(30), P_RA(0, 31), P_AD(0, 12), P_D2(6), P_SR(2, 7), 0 }, { P_DM(0, 1), P_TL(5), P_RA(0, 31), P_AD(0, 8), P_D2(4), P_SR(1, 10), 0 }} },
    /* 1 */ { P_AF(2, 4), P_PAP(1, 1, 0, 0), {{ P_DM(0, 1), P_TL(40), P_RA(0, 31), P_AD(0, 12), P_D2(6), P_SR(2, 6), 0 }, { P_DM(0, 2), P_TL(25), P_RA(0, 31), P_AD(0, 10), P_D2(5), P_SR(2, 7), 0 }, { P_DM(0, 1), P_TL(20), P_RA(0, 31), P_AD(0, 8), P_D2(4), P_SR(2, 8), 0 }, { P_DM(0, 1), P_TL(6), P_RA(0, 31), P_AD(0, 6), P_D2(3), P_SR(1, 10), 0 }} },
    /* 2 */ { P_AF(7, 2), P_PAP(1, 1, 1, 3), {{ P_DM(3, 1), P_TL(18), P_RA(0, 18), P_AD(0, 2), P_D2(1), P_SR(2, 3), 0 }, { P_DM(4, 2), P_TL(20), P_RA(0, 18), P_AD(0, 2), P_D2(1), P_SR(2, 3), 0 }, { P_DM(0, 3), P_TL(24), P_RA(0, 18), P_AD(0, 2), P_D2(1), P_SR(2, 3), 0 }, { P_DM(7, 1), P_TL(22), P_RA(0, 18), P_AD(0, 2), P_D2(1), P_SR(2, 3), 0 }} },
    /* 3 */ { P_AF(5, 3), P_PAP(1, 1, 1, 4), {{ P_DM(3, 2), P_TL(32), P_RA(0, 20), P_AD(1, 4), P_D2(2), P_SR(3, 4), 0 }, { P_DM(0, 1), P_TL(14), P_RA(0, 22), P_AD(0, 3), P_D2(2), P_SR(2, 5), 0 }, { P_DM(4, 4), P_TL(16), P_RA(0, 22), P_AD(0, 3), P_D2(2), P_SR(2, 5), 0 }, { P_DM(0, 1), P_TL(12), P_RA(0, 22), P_AD(0, 3), P_D2(2), P_SR(2, 5), 0 }} },
    /* 4 */ { P_AF(5, 6), P_PAP(1, 1, 0, 5), {{ P_DM(3, 3), P_TL(38), P_RA(1, 31), P_AD(0, 10), P_D2(5), P_SR(3, 6), 0 }, { P_DM(0, 1), P_TL(10), P_RA(1, 31), P_AD(0, 6), P_D2(3), P_SR(2, 8), 0 }, { P_DM(0, 2), P_TL(14), P_RA(1, 31), P_AD(0, 7), P_D2(4), P_SR(2, 8), 0 }, { P_DM(0, 4), P_TL(16), P_RA(1, 31), P_AD(0, 8), P_D2(4), P_SR(3, 8), 0 }} },
    /* 5 */ { P_AF(4, 4), P_PAP(1, 1, 0, 4), {{ P_DM(3, 2), P_TL(35), P_RA(0, 28), P_AD(0, 8), P_D2(4), P_SR(3, 6), 0 }, { P_DM(0, 1), P_TL(12), P_RA(0, 31), P_AD(0, 5), P_D2(3), P_SR(2, 8), 0 }, { P_DM(4, 2), P_TL(38), P_RA(0, 28), P_AD(0, 8), P_D2(4), P_SR(3, 6), 0 }, { P_DM(0, 1), P_TL(10), P_RA(0, 31), P_AD(0, 5), P_D2(3), P_SR(2, 8), 0 }} },
    /* 6 */ { P_AF(4, 4), P_PAP(1, 1, 0, 0), {{ P_DM(0, 2), P_TL(42), P_RA(2, 31), P_AD(0, 22), P_D2(18), P_SR(6, 12), 0 }, { P_DM(0, 1), P_TL(16), P_RA(2, 31), P_AD(0, 20), P_D2(15), P_SR(5, 14), 0 }, { P_DM(0, 3), P_TL(45), P_RA(2, 31), P_AD(0, 24), P_D2(18), P_SR(7, 12), 0 }, { P_DM(0, 1), P_TL(14), P_RA(2, 31), P_AD(0, 18), P_D2(14), P_SR(4, 14), 0 }} },
    /* 7 */ { P_AF(4, 0), P_PAP(1, 1, 0, 0), {{ P_DM(0, 14), P_TL(50), P_RA(3, 31), P_AD(0, 10), P_D2(4), P_SR(4, 6), 0 }, { P_DM(0, 1), P_TL(12), P_RA(2, 31), P_AD(0, 6), P_D2(2), P_SR(2, 8), 0 }, { P_DM(0, 7), P_TL(55), P_RA(3, 31), P_AD(0, 12), P_D2(5), P_SR(5, 6), 0 }, { P_DM(0, 1), P_TL(10), P_RA(2, 31), P_AD(0, 5), P_D2(2), P_SR(2, 8), 0 }} },
    /* 8 */ { P_AF(4, 7), P_PAP(1, 1, 0, 0), {{ P_DM(0, 1), P_TL(20), P_RA(0, 31), P_AD(0, 31), P_D2(31), P_SR(15, 15), 0 }, { P_DM(0, 0), P_TL(0), P_RA(0, 31), P_AD(0, 18), P_D2(10), P_SR(8, 10), 0 }, { P_DM(0, 1), P_TL(25), P_RA(0, 31), P_AD(0, 28), P_D2(20), P_SR(12, 12), 0 }, { P_DM(0, 0), P_TL(5), P_RA(0, 31), P_AD(0, 15), P_D2(8), P_SR(6, 10), 0 }} },
    /* 9 */ { P_AF(7, 7), P_PAP(1, 1, 0, 0), {{ P_DM(7, 7), P_TL(28), P_RA(0, 31), P_AD(0, 20), P_D2(15), P_SR(8, 12), 0 }, { P_DM(3, 1), P_TL(18), P_RA(0, 31), P_AD(0, 18), P_D2(12), P_SR(7, 11), 0 }, { P_DM(5, 9), P_TL(30), P_RA(0, 31), P_AD(0, 22), P_D2(16), P_SR(9, 12), 0 }, { P_DM(0, 1), P_TL(15), P_RA(0, 31), P_AD(0, 16), P_D2(10), P_SR(6, 10), 0 }} },
    /*10 */ { P_AF(7, 7), P_PAP(1, 1, 0, 0), {{ P_DM(7, 14), P_TL(35), P_RA(0, 31), P_AD(0, 28), P_D2(20), P_SR(12, 14), 0 }, { P_DM(3, 9), P_TL(38), P_RA(0, 31), P_AD(0, 26), P_D2(18), P_SR(11, 14), 0 }, { P_DM(5, 13), P_TL(40), P_RA(0, 31), P_AD(0, 30), P_D2(22), P_SR(13, 14), 0 }, { P_DM(0, 7), P_TL(32), P_RA(0, 31), P_AD(0, 24), P_D2(16), P_SR(10, 14), 0 }} },
    /*11 */ { P_AF(4, 5), P_PAP(1, 1, 0, 0), {{ P_DM(0, 2), P_TL(30), P_RA(0, 31), P_AD(0, 20), P_D2(12), P_SR(8, 10), 0 }, { P_DM(0, 1), P_TL(8), P_RA(0, 31), P_AD(0, 14), P_D2(8), P_SR(5, 10), 0 }, { P_DM(0, 3), P_TL(35), P_RA(0, 31), P_AD(0, 22), P_D2(14), P_SR(9, 10), 0 }, { P_DM(0, 1), P_TL(6), P_RA(0, 31), P_AD(0, 12), P_D2(6), P_SR(4, 10), 0 }} },
};

#endif /* SMD_PATCHES_RETROWAVE_H */
