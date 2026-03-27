//! SDK dependency resolution and static data generation

use crate::backend::m68k::m68k::M68kInst;
use std::collections::HashSet;

/// Get the set of SDK functions that a given function depends on
pub fn get_sdk_dependencies(func_name: &str) -> &'static [&'static str] {
    match func_name {
        // VDP dependencies
        "vdp_vsync" => &["vdp_wait_vblank_start"],
        "vdp_wait_frame" => &["vdp_wait_vblank_start", "vdp_wait_vblank_end"],

        // YM2612 dependencies
        "ym_reset" => &["ym_init"],
        "ym_init" => &["ym_write0", "ym_key_off", "ym_write_op"],
        "ym_write_ch" => &["ym_write0", "ym_write1"],
        "ym_write_op" => &["ym_write0", "ym_write1"],
        "ym_set_freq" => &["ym_write_ch"],
        "ym_set_algo" => &["ym_write_ch"],
        "ym_set_pan" => &["ym_write_ch"],
        "ym_set_volume" => &["ym_write_op"],
        "ym_key_on" => &["ym_write0"],
        "ym_key_off" => &["ym_write0"],
        "ym_key_on_ops" => &["ym_write0"],
        "ym_set_lfo" => &["ym_write0"],

        // PSG dependencies
        "psg_init" => &["psg_stop"],
        "psg_set_freq" => &["psg_set_tone"],
        "psg_beep" => &["psg_set_tone"],
        "psg_note_on" => &["psg_beep", "psg_set_tone"],

        // Sprite dependencies
        "sprite_clear" => &["sprite_hide"],
        "sprite_clear_all" => &["sprite_init"],

        // Input dependencies
        "input_held" => &["input_read"],
        "input_pressed" => &["input_read"],

        _ => &[],
    }
}

/// Resolve all transitive dependencies for a set of SDK functions
pub fn resolve_dependencies(functions: &HashSet<String>) -> HashSet<String> {
    let mut all = functions.clone();
    let mut changed = true;

    while changed {
        changed = false;
        let current: Vec<_> = all.iter().cloned().collect();
        for func in current {
            for dep in get_sdk_dependencies(&func) {
                if all.insert(dep.to_string()) {
                    changed = true;
                }
            }
        }
    }

    all
}

/// Check if any functions in the set need the frame counter
pub fn needs_frame_counter(functions: &HashSet<String>) -> bool {
    functions.iter().any(|f| {
        matches!(
            f.as_str(),
            "vdp_wait_vblank_start" | "vdp_get_frame_count" | "vdp_reset_frame_count" | "vdp_init"
        )
    })
}

/// Check if any functions need the random state variable
pub fn needs_rand_state(functions: &HashSet<String>) -> bool {
    functions
        .iter()
        .any(|f| matches!(f.as_str(), "rand_next" | "rand_seed"))
}

/// Check if any functions need the operator offset table
pub fn needs_op_offsets(functions: &HashSet<String>) -> bool {
    functions
        .iter()
        .any(|f| matches!(f.as_str(), "ym_write_op" | "ym_load_operator" | "ym_init"))
}

/// Generate SDK static data section
pub fn generate_static_data(functions: &HashSet<String>) -> Vec<M68kInst> {
    let mut insts = Vec::new();

    if needs_frame_counter(functions) || needs_op_offsets(functions) || needs_rand_state(functions)
    {
        insts.push(M68kInst::Directive(".section .bss".to_string()));
        insts.push(M68kInst::Directive(".align 4".to_string()));
    }

    if needs_frame_counter(functions) {
        insts.push(M68kInst::Label("__sdk_frame_count".to_string()));
        insts.push(M68kInst::Directive(".space 4".to_string()));
    }

    if needs_rand_state(functions) {
        insts.push(M68kInst::Label("__sdk_rand_state".to_string()));
        insts.push(M68kInst::Directive(".space 4".to_string()));
    }

    if needs_op_offsets(functions) {
        insts.push(M68kInst::Directive(".section .rodata".to_string()));
        insts.push(M68kInst::Directive(".align 4".to_string()));
        insts.push(M68kInst::Label("__sdk_op_offsets".to_string()));
        insts.push(M68kInst::Directive(".long 0, 8, 4, 12".to_string()));
    }

    insts
}
