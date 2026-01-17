/**
 * Channel Test - Verify all YM2612 and PSG channels work
 * Plays each channel one by one with a label
 */

int YM_ADDR_PORT0 = 0xA04000;
int YM_DATA_PORT0 = 0xA04001;
int YM_ADDR_PORT1 = 0xA04002;
int YM_DATA_PORT1 = 0xA04003;
int PSG_PORT = 0xC00011;

void ym_delay(void) {
    int i;
    i = 0;
_d:
    i = i + 1;
    if (i < 20) { goto _d; }
}

void ym_write0(int reg, int val) {
    unsigned char *a;
    unsigned char *d;
    a = (unsigned char *)YM_ADDR_PORT0;
    d = (unsigned char *)YM_DATA_PORT0;
    *a = reg;
    ym_delay();
    *d = val;
    ym_delay();
}

void ym_write1(int reg, int val) {
    unsigned char *a;
    unsigned char *d;
    a = (unsigned char *)YM_ADDR_PORT1;
    d = (unsigned char *)YM_DATA_PORT1;
    *a = reg;
    ym_delay();
    *d = val;
    ym_delay();
}

void ym_op(int ch, int op, int reg, int val) {
    int off;
    int r;

    if (op < 1) { off = 0; }
    if (op > 0) { if (op < 2) { off = 8; } }
    if (op > 1) { if (op < 3) { off = 4; } }
    if (op > 2) { off = 12; }

    r = ch;
    if (ch > 2) {
        r = ch - 3;
        ym_write1(reg + r + off, val);
    } else {
        ym_write0(reg + r + off, val);
    }
}

void ym_on(int ch) {
    int s;
    if (ch < 3) { s = 0xF0 | ch; }
    else { s = 0xF0 | (ch - 3) | 4; }
    ym_write0(0x28, s);
}

void ym_off(int ch) {
    int s;
    if (ch < 3) { s = ch; }
    else { s = (ch - 3) | 4; }
    ym_write0(0x28, s);
}

void ym_freq(int ch, int block, int fnum) {
    int r;
    int fh;
    r = ch;
    if (ch > 2) { r = ch - 3; }
    fh = ((block & 7) << 3) | ((fnum >> 8) & 7);
    if (ch < 3) {
        ym_write0(0xA4 + r, fh);
        ym_write0(0xA0 + r, fnum & 0xFF);
    } else {
        ym_write1(0xA4 + r, fh);
        ym_write1(0xA0 + r, fnum & 0xFF);
    }
}

/* Simple sine-like patch for testing */
void ym_patch_test(int ch) {
    int p;
    int r;
    p = 0;
    r = ch;
    if (ch > 2) { p = 1; r = ch - 3; }

    /* Algorithm 0, Feedback 0 */
    if (p < 1) { ym_write0(0xB0 + r, 0x00); }
    else { ym_write1(0xB0 + r, 0x00); }

    /* Stereo L+R */
    if (p < 1) { ym_write0(0xB4 + r, 0xC0); }
    else { ym_write1(0xB4 + r, 0xC0); }

    /* Op 1-3: Off */
    ym_op(ch, 0, 0x40, 0x7F);
    ym_op(ch, 1, 0x40, 0x7F);
    ym_op(ch, 2, 0x40, 0x7F);

    /* Op 4: Carrier only */
    ym_op(ch, 3, 0x30, 0x01);  /* MUL=1 */
    ym_op(ch, 3, 0x40, 0x10);  /* TL = loud */
    ym_op(ch, 3, 0x50, 0x1F);  /* AR = 31 */
    ym_op(ch, 3, 0x60, 0x00);  /* D1R = 0 */
    ym_op(ch, 3, 0x70, 0x00);  /* D2R = 0 */
    ym_op(ch, 3, 0x80, 0x0F);  /* RR = 15 */
}

void psg_w(int val) {
    unsigned char *p;
    p = (unsigned char *)PSG_PORT;
    *p = val;
}

void psg_vol(int ch, int vol) {
    psg_w(0x90 | ((ch & 3) << 5) | (vol & 15));
}

void psg_tone(int ch, int div) {
    psg_w(0x80 | ((ch & 3) << 5) | (div & 15));
    psg_w((div >> 4) & 63);
}

void psg_noise(int mode) {
    psg_w(0xE0 | (mode & 7));
}

void wait_long(void) {
    int i;
    i = 0;
_w:
    i = i + 1;
    if (i < 50000) { goto _w; }
}

void wait_short(void) {
    int i;
    i = 0;
_ws:
    i = i + 1;
    if (i < 20000) { goto _ws; }
}

void all_off(void) {
    ym_off(0);
    ym_off(1);
    ym_off(2);
    ym_off(3);
    ym_off(4);
    ym_off(5);
    psg_vol(0, 15);
    psg_vol(1, 15);
    psg_vol(2, 15);
    psg_vol(3, 15);
}

void ym_init(void) {
    ym_write0(0x22, 0x00);
    ym_write0(0x27, 0x00);
    ym_write0(0x2B, 0x00);

    ym_write0(0x28, 0x00);
    ym_write0(0x28, 0x01);
    ym_write0(0x28, 0x02);
    ym_write0(0x28, 0x04);
    ym_write0(0x28, 0x05);
    ym_write0(0x28, 0x06);

    /* Setup test patch on all 6 FM channels */
    ym_patch_test(0);
    ym_patch_test(1);
    ym_patch_test(2);
    ym_patch_test(3);
    ym_patch_test(4);
    ym_patch_test(5);

    psg_vol(0, 15);
    psg_vol(1, 15);
    psg_vol(2, 15);
    psg_vol(3, 15);
}

int main(void) {
    int *step;

    ym_init();

    step = (int *)0xFF0010;
    *step = 0;

mainloop:
    all_off();
    wait_short();

    /* Test each channel in sequence */

    /* FM Channel 0 - C4 */
    if (*step < 1) {
        ym_freq(0, 4, 644);
        ym_on(0);
        wait_long();
    }

    /* FM Channel 1 - D4 */
    if (*step > 0) {
        if (*step < 2) {
            ym_freq(1, 4, 723);
            ym_on(1);
            wait_long();
        }
    }

    /* FM Channel 2 - E4 */
    if (*step > 1) {
        if (*step < 3) {
            ym_freq(2, 4, 811);
            ym_on(2);
            wait_long();
        }
    }

    /* FM Channel 3 - F4 */
    if (*step > 2) {
        if (*step < 4) {
            ym_freq(3, 4, 859);
            ym_on(3);
            wait_long();
        }
    }

    /* FM Channel 4 - G4 */
    if (*step > 3) {
        if (*step < 5) {
            ym_freq(4, 4, 964);
            ym_on(4);
            wait_long();
        }
    }

    /* FM Channel 5 - A4 */
    if (*step > 4) {
        if (*step < 6) {
            ym_freq(5, 4, 1081);
            ym_on(5);
            wait_long();
        }
    }

    /* PSG Channel 0 - B4 */
    if (*step > 5) {
        if (*step < 7) {
            psg_tone(0, 170);
            psg_vol(0, 0);
            wait_long();
        }
    }

    /* PSG Channel 1 - C5 */
    if (*step > 6) {
        if (*step < 8) {
            psg_tone(1, 127);
            psg_vol(1, 0);
            wait_long();
        }
    }

    /* PSG Channel 2 - D5 */
    if (*step > 7) {
        if (*step < 9) {
            psg_tone(2, 113);
            psg_vol(2, 0);
            wait_long();
        }
    }

    /* PSG Noise Channel 3 */
    if (*step > 8) {
        if (*step < 10) {
            psg_noise(6);
            psg_vol(3, 0);
            wait_long();
        }
    }

    /* All channels together! */
    if (*step > 9) {
        /* FM chord */
        ym_freq(0, 4, 811);  /* E */
        ym_freq(1, 4, 1021); /* G# */
        ym_freq(2, 4, 1214); /* B */
        ym_freq(3, 3, 811);  /* E low */
        ym_freq(4, 3, 1021); /* G# low */
        ym_freq(5, 3, 1214); /* B low */
        ym_on(0);
        ym_on(1);
        ym_on(2);
        ym_on(3);
        ym_on(4);
        ym_on(5);

        /* PSG */
        psg_tone(0, 127);
        psg_tone(1, 101);
        psg_tone(2, 85);
        psg_vol(0, 4);
        psg_vol(1, 4);
        psg_vol(2, 4);

        wait_long();
        wait_long();

        *step = 0;
        goto mainloop;
    }

    *step = *step + 1;
    goto mainloop;
}
