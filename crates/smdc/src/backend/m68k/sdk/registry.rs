//! SDK function registry

use super::{SdkCategory, SdkFunction, SdkFunctionKind};
use std::collections::HashMap;

/// Registry of all SDK functions
pub struct SdkRegistry {
    functions: HashMap<&'static str, SdkFunction>,
}

impl SdkRegistry {
    pub fn new() -> Self {
        let mut functions = HashMap::new();

        // VDP Functions
        Self::register_vdp_functions(&mut functions);

        // Sprite Functions
        Self::register_sprite_functions(&mut functions);

        // Input Functions
        Self::register_input_functions(&mut functions);

        // YM2612 Functions
        Self::register_ym2612_functions(&mut functions);

        // PSG Functions
        Self::register_psg_functions(&mut functions);

        Self { functions }
    }

    pub fn lookup(&self, name: &str) -> Option<&SdkFunction> {
        self.functions.get(name)
    }

    pub fn is_sdk_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    fn register_vdp_functions(map: &mut HashMap<&'static str, SdkFunction>) {
        use SdkCategory::Vdp;
        use SdkFunctionKind::{Inline, Library};

        // Inline VDP functions
        map.insert(
            "vdp_set_reg",
            SdkFunction {
                name: "vdp_set_reg",
                kind: Inline,
                category: Vdp,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "vdp_get_status",
            SdkFunction {
                name: "vdp_get_status",
                kind: Inline,
                category: Vdp,
                param_count: 0,
                has_return: true,
            },
        );
        map.insert(
            "vdp_set_write_addr",
            SdkFunction {
                name: "vdp_set_write_addr",
                kind: Inline,
                category: Vdp,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "vdp_set_cram_addr",
            SdkFunction {
                name: "vdp_set_cram_addr",
                kind: Inline,
                category: Vdp,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "vdp_set_color",
            SdkFunction {
                name: "vdp_set_color",
                kind: Inline,
                category: Vdp,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "vdp_set_background",
            SdkFunction {
                name: "vdp_set_background",
                kind: Inline,
                category: Vdp,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "vdp_in_vblank",
            SdkFunction {
                name: "vdp_in_vblank",
                kind: Inline,
                category: Vdp,
                param_count: 0,
                has_return: true,
            },
        );

        // Library VDP functions
        map.insert(
            "vdp_init",
            SdkFunction {
                name: "vdp_init",
                kind: Library,
                category: Vdp,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "vdp_vsync",
            SdkFunction {
                name: "vdp_vsync",
                kind: Library,
                category: Vdp,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "vdp_wait_vblank_start",
            SdkFunction {
                name: "vdp_wait_vblank_start",
                kind: Library,
                category: Vdp,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "vdp_wait_vblank_end",
            SdkFunction {
                name: "vdp_wait_vblank_end",
                kind: Library,
                category: Vdp,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "vdp_wait_frame",
            SdkFunction {
                name: "vdp_wait_frame",
                kind: Library,
                category: Vdp,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "vdp_load_palette",
            SdkFunction {
                name: "vdp_load_palette",
                kind: Library,
                category: Vdp,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "vdp_load_tiles",
            SdkFunction {
                name: "vdp_load_tiles",
                kind: Library,
                category: Vdp,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "vdp_set_tile_a",
            SdkFunction {
                name: "vdp_set_tile_a",
                kind: Library,
                category: Vdp,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "vdp_set_tile_b",
            SdkFunction {
                name: "vdp_set_tile_b",
                kind: Library,
                category: Vdp,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "vdp_clear_plane_a",
            SdkFunction {
                name: "vdp_clear_plane_a",
                kind: Library,
                category: Vdp,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "vdp_clear_plane_b",
            SdkFunction {
                name: "vdp_clear_plane_b",
                kind: Library,
                category: Vdp,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "vdp_set_hscroll_a",
            SdkFunction {
                name: "vdp_set_hscroll_a",
                kind: Library,
                category: Vdp,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "vdp_set_hscroll_b",
            SdkFunction {
                name: "vdp_set_hscroll_b",
                kind: Library,
                category: Vdp,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "vdp_set_vscroll_a",
            SdkFunction {
                name: "vdp_set_vscroll_a",
                kind: Library,
                category: Vdp,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "vdp_set_vscroll_b",
            SdkFunction {
                name: "vdp_set_vscroll_b",
                kind: Library,
                category: Vdp,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "vdp_get_frame_count",
            SdkFunction {
                name: "vdp_get_frame_count",
                kind: Library,
                category: Vdp,
                param_count: 0,
                has_return: true,
            },
        );
        map.insert(
            "vdp_reset_frame_count",
            SdkFunction {
                name: "vdp_reset_frame_count",
                kind: Library,
                category: Vdp,
                param_count: 0,
                has_return: false,
            },
        );
    }

    fn register_sprite_functions(map: &mut HashMap<&'static str, SdkFunction>) {
        use SdkCategory::Sprite;
        use SdkFunctionKind::{Inline, Library};

        // Inline Sprite functions
        map.insert(
            "sprite_attr",
            SdkFunction {
                name: "sprite_attr",
                kind: Inline,
                category: Sprite,
                param_count: 5,
                has_return: true,
            },
        );
        map.insert(
            "sprite_get_width",
            SdkFunction {
                name: "sprite_get_width",
                kind: Inline,
                category: Sprite,
                param_count: 1,
                has_return: true,
            },
        );
        map.insert(
            "sprite_get_height",
            SdkFunction {
                name: "sprite_get_height",
                kind: Inline,
                category: Sprite,
                param_count: 1,
                has_return: true,
            },
        );

        // Library Sprite functions
        map.insert(
            "sprite_init",
            SdkFunction {
                name: "sprite_init",
                kind: Library,
                category: Sprite,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "sprite_set",
            SdkFunction {
                name: "sprite_set",
                kind: Library,
                category: Sprite,
                param_count: 5,
                has_return: false,
            },
        );
        map.insert(
            "sprite_set_pos",
            SdkFunction {
                name: "sprite_set_pos",
                kind: Library,
                category: Sprite,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "sprite_hide",
            SdkFunction {
                name: "sprite_hide",
                kind: Library,
                category: Sprite,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "sprite_clear",
            SdkFunction {
                name: "sprite_clear",
                kind: Library,
                category: Sprite,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "sprite_clear_all",
            SdkFunction {
                name: "sprite_clear_all",
                kind: Library,
                category: Sprite,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "sprite_set_link",
            SdkFunction {
                name: "sprite_set_link",
                kind: Library,
                category: Sprite,
                param_count: 2,
                has_return: false,
            },
        );
    }

    fn register_input_functions(map: &mut HashMap<&'static str, SdkFunction>) {
        use SdkCategory::Input;
        use SdkFunctionKind::{Inline, Library};

        // Inline Input functions
        map.insert(
            "joy1_read",
            SdkFunction {
                name: "joy1_read",
                kind: Inline,
                category: Input,
                param_count: 0,
                has_return: true,
            },
        );
        map.insert(
            "joy2_read",
            SdkFunction {
                name: "joy2_read",
                kind: Inline,
                category: Input,
                param_count: 0,
                has_return: true,
            },
        );

        // Library Input functions
        map.insert(
            "input_init",
            SdkFunction {
                name: "input_init",
                kind: Library,
                category: Input,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "input_read",
            SdkFunction {
                name: "input_read",
                kind: Library,
                category: Input,
                param_count: 1,
                has_return: true,
            },
        );
        map.insert(
            "input_update",
            SdkFunction {
                name: "input_update",
                kind: Library,
                category: Input,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "input_held",
            SdkFunction {
                name: "input_held",
                kind: Library,
                category: Input,
                param_count: 1,
                has_return: true,
            },
        );
        map.insert(
            "input_pressed",
            SdkFunction {
                name: "input_pressed",
                kind: Library,
                category: Input,
                param_count: 1,
                has_return: true,
            },
        );
        map.insert(
            "input_released",
            SdkFunction {
                name: "input_released",
                kind: Library,
                category: Input,
                param_count: 1,
                has_return: true,
            },
        );
        map.insert(
            "input_is_6button",
            SdkFunction {
                name: "input_is_6button",
                kind: Library,
                category: Input,
                param_count: 1,
                has_return: true,
            },
        );
    }

    fn register_ym2612_functions(map: &mut HashMap<&'static str, SdkFunction>) {
        use SdkCategory::Ym2612;
        use SdkFunctionKind::{Inline, Library};

        // Inline YM2612 functions
        map.insert(
            "ym_read_status",
            SdkFunction {
                name: "ym_read_status",
                kind: Inline,
                category: Ym2612,
                param_count: 0,
                has_return: true,
            },
        );
        map.insert(
            "ym_write0",
            SdkFunction {
                name: "ym_write0",
                kind: Inline,
                category: Ym2612,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "ym_write1",
            SdkFunction {
                name: "ym_write1",
                kind: Inline,
                category: Ym2612,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "ym_dac_enable",
            SdkFunction {
                name: "ym_dac_enable",
                kind: Inline,
                category: Ym2612,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "ym_dac_disable",
            SdkFunction {
                name: "ym_dac_disable",
                kind: Inline,
                category: Ym2612,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "ym_dac_write",
            SdkFunction {
                name: "ym_dac_write",
                kind: Inline,
                category: Ym2612,
                param_count: 1,
                has_return: false,
            },
        );

        // Library YM2612 functions
        map.insert(
            "ym_init",
            SdkFunction {
                name: "ym_init",
                kind: Library,
                category: Ym2612,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "ym_reset",
            SdkFunction {
                name: "ym_reset",
                kind: Library,
                category: Ym2612,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "ym_wait",
            SdkFunction {
                name: "ym_wait",
                kind: Library,
                category: Ym2612,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "ym_write_ch",
            SdkFunction {
                name: "ym_write_ch",
                kind: Library,
                category: Ym2612,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "ym_write_op",
            SdkFunction {
                name: "ym_write_op",
                kind: Library,
                category: Ym2612,
                param_count: 4,
                has_return: false,
            },
        );
        map.insert(
            "ym_key_on",
            SdkFunction {
                name: "ym_key_on",
                kind: Library,
                category: Ym2612,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "ym_key_off",
            SdkFunction {
                name: "ym_key_off",
                kind: Library,
                category: Ym2612,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "ym_key_on_ops",
            SdkFunction {
                name: "ym_key_on_ops",
                kind: Library,
                category: Ym2612,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "ym_set_freq",
            SdkFunction {
                name: "ym_set_freq",
                kind: Library,
                category: Ym2612,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "ym_set_freq_detune",
            SdkFunction {
                name: "ym_set_freq_detune",
                kind: Library,
                category: Ym2612,
                param_count: 4,
                has_return: false,
            },
        );
        map.insert(
            "ym_set_algo",
            SdkFunction {
                name: "ym_set_algo",
                kind: Library,
                category: Ym2612,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "ym_set_pan",
            SdkFunction {
                name: "ym_set_pan",
                kind: Library,
                category: Ym2612,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "ym_set_volume",
            SdkFunction {
                name: "ym_set_volume",
                kind: Library,
                category: Ym2612,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "ym_set_lfo",
            SdkFunction {
                name: "ym_set_lfo",
                kind: Library,
                category: Ym2612,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "ym_set_lfo_sensitivity",
            SdkFunction {
                name: "ym_set_lfo_sensitivity",
                kind: Library,
                category: Ym2612,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "ym_load_patch",
            SdkFunction {
                name: "ym_load_patch",
                kind: Library,
                category: Ym2612,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "ym_load_operator",
            SdkFunction {
                name: "ym_load_operator",
                kind: Library,
                category: Ym2612,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "ym_dac_play",
            SdkFunction {
                name: "ym_dac_play",
                kind: Library,
                category: Ym2612,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "ym_set_timer_a",
            SdkFunction {
                name: "ym_set_timer_a",
                kind: Library,
                category: Ym2612,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "ym_set_timer_b",
            SdkFunction {
                name: "ym_set_timer_b",
                kind: Library,
                category: Ym2612,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "ym_start_timers",
            SdkFunction {
                name: "ym_start_timers",
                kind: Library,
                category: Ym2612,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "ym_stop_timers",
            SdkFunction {
                name: "ym_stop_timers",
                kind: Library,
                category: Ym2612,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "ym_timer_a_overflow",
            SdkFunction {
                name: "ym_timer_a_overflow",
                kind: Library,
                category: Ym2612,
                param_count: 0,
                has_return: true,
            },
        );
        map.insert(
            "ym_timer_b_overflow",
            SdkFunction {
                name: "ym_timer_b_overflow",
                kind: Library,
                category: Ym2612,
                param_count: 0,
                has_return: true,
            },
        );

        // YM2612 Patch functions
        for patch in &[
            "ym_patch_dist_guitar",
            "ym_patch_palm_mute",
            "ym_patch_clean_guitar",
            "ym_patch_lead_guitar",
            "ym_patch_synth_bass",
            "ym_patch_elec_bass",
            "ym_patch_epiano",
            "ym_patch_strings",
            "ym_patch_brass",
            "ym_patch_organ",
            "ym_patch_synth_lead",
            "ym_patch_kick",
            "ym_patch_snare",
            "ym_patch_tom",
            "ym_patch_hihat",
        ] {
            map.insert(
                patch,
                SdkFunction {
                    name: patch,
                    kind: Library,
                    category: Ym2612,
                    param_count: 1,
                    has_return: false,
                },
            );
        }

        // Vibrato functions
        map.insert(
            "ym_vibrato_init",
            SdkFunction {
                name: "ym_vibrato_init",
                kind: Library,
                category: Ym2612,
                param_count: 5,
                has_return: false,
            },
        );
        map.insert(
            "ym_vibrato_update",
            SdkFunction {
                name: "ym_vibrato_update",
                kind: Library,
                category: Ym2612,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "ym_pitch_bend",
            SdkFunction {
                name: "ym_pitch_bend",
                kind: Library,
                category: Ym2612,
                param_count: 4,
                has_return: true,
            },
        );
    }

    fn register_psg_functions(map: &mut HashMap<&'static str, SdkFunction>) {
        use SdkCategory::Psg;
        use SdkFunctionKind::{Inline, Library};

        // Inline PSG functions
        map.insert(
            "psg_write",
            SdkFunction {
                name: "psg_write",
                kind: Inline,
                category: Psg,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "psg_set_volume",
            SdkFunction {
                name: "psg_set_volume",
                kind: Inline,
                category: Psg,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "psg_set_noise",
            SdkFunction {
                name: "psg_set_noise",
                kind: Inline,
                category: Psg,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "psg_stop_channel",
            SdkFunction {
                name: "psg_stop_channel",
                kind: Inline,
                category: Psg,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "psg_note_off",
            SdkFunction {
                name: "psg_note_off",
                kind: Inline,
                category: Psg,
                param_count: 1,
                has_return: false,
            },
        );

        // Library PSG functions
        map.insert(
            "psg_init",
            SdkFunction {
                name: "psg_init",
                kind: Library,
                category: Psg,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "psg_set_tone",
            SdkFunction {
                name: "psg_set_tone",
                kind: Library,
                category: Psg,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "psg_set_freq",
            SdkFunction {
                name: "psg_set_freq",
                kind: Library,
                category: Psg,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "psg_stop",
            SdkFunction {
                name: "psg_stop",
                kind: Library,
                category: Psg,
                param_count: 0,
                has_return: false,
            },
        );
        map.insert(
            "psg_beep",
            SdkFunction {
                name: "psg_beep",
                kind: Library,
                category: Psg,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "psg_note_on",
            SdkFunction {
                name: "psg_note_on",
                kind: Library,
                category: Psg,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "psg_hihat",
            SdkFunction {
                name: "psg_hihat",
                kind: Library,
                category: Psg,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "psg_snare_noise",
            SdkFunction {
                name: "psg_snare_noise",
                kind: Library,
                category: Psg,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "psg_kick",
            SdkFunction {
                name: "psg_kick",
                kind: Library,
                category: Psg,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "psg_cymbal",
            SdkFunction {
                name: "psg_cymbal",
                kind: Library,
                category: Psg,
                param_count: 1,
                has_return: false,
            },
        );
        map.insert(
            "psg_env_init",
            SdkFunction {
                name: "psg_env_init",
                kind: Library,
                category: Psg,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "psg_env_attack",
            SdkFunction {
                name: "psg_env_attack",
                kind: Library,
                category: Psg,
                param_count: 3,
                has_return: false,
            },
        );
        map.insert(
            "psg_env_release",
            SdkFunction {
                name: "psg_env_release",
                kind: Library,
                category: Psg,
                param_count: 2,
                has_return: false,
            },
        );
        map.insert(
            "psg_env_update",
            SdkFunction {
                name: "psg_env_update",
                kind: Library,
                category: Psg,
                param_count: 1,
                has_return: true,
            },
        );
    }
}

impl Default for SdkRegistry {
    fn default() -> Self {
        Self::new()
    }
}
