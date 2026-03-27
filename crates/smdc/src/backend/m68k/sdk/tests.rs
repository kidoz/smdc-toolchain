//! Unit tests for the SDK module

use super::*;
use crate::backend::m68k::m68k::*;
use std::collections::HashSet;

// ============================================================================
// Registry Tests
// ============================================================================

#[test]
fn registry_contains_all_vdp_functions() {
    let reg = SdkRegistry::new();
    let vdp_names = [
        "vdp_set_reg",
        "vdp_get_status",
        "vdp_set_write_addr",
        "vdp_set_cram_addr",
        "vdp_set_color",
        "vdp_set_background",
        "vdp_in_vblank",
        "vdp_init",
        "vdp_vsync",
        "vdp_wait_vblank_start",
        "vdp_wait_vblank_end",
        "vdp_wait_frame",
        "vdp_load_palette",
        "vdp_load_tiles",
        "vdp_set_tile_a",
        "vdp_set_tile_b",
        "vdp_clear_plane_a",
        "vdp_clear_plane_b",
        "vdp_set_hscroll_a",
        "vdp_set_hscroll_b",
        "vdp_set_vscroll_a",
        "vdp_set_vscroll_b",
        "vdp_get_frame_count",
        "vdp_reset_frame_count",
    ];
    for name in &vdp_names {
        assert!(
            reg.is_sdk_function(name),
            "VDP function '{name}' not found in registry"
        );
        let f = reg.lookup(name).unwrap();
        assert_eq!(f.category, SdkCategory::Vdp);
    }
}

#[test]
fn registry_contains_all_ym2612_functions() {
    let reg = SdkRegistry::new();
    let ym_names = [
        "ym_read_status",
        "ym_write0",
        "ym_write1",
        "ym_dac_enable",
        "ym_dac_disable",
        "ym_dac_write",
        "ym_init",
        "ym_reset",
        "ym_wait",
        "ym_write_ch",
        "ym_write_op",
        "ym_key_on",
        "ym_key_off",
        "ym_key_on_ops",
        "ym_set_freq",
        "ym_set_algo",
        "ym_set_pan",
        "ym_set_volume",
        "ym_set_lfo",
    ];
    for name in &ym_names {
        assert!(
            reg.is_sdk_function(name),
            "YM2612 function '{name}' not found in registry"
        );
        let f = reg.lookup(name).unwrap();
        assert_eq!(f.category, SdkCategory::Ym2612);
    }
}

#[test]
fn registry_contains_all_psg_functions() {
    let reg = SdkRegistry::new();
    let psg_names = [
        "psg_write",
        "psg_set_volume",
        "psg_set_noise",
        "psg_stop_channel",
        "psg_note_off",
        "psg_init",
        "psg_set_tone",
        "psg_set_freq",
        "psg_stop",
        "psg_beep",
        "psg_note_on",
    ];
    for name in &psg_names {
        assert!(
            reg.is_sdk_function(name),
            "PSG function '{name}' not found in registry"
        );
        let f = reg.lookup(name).unwrap();
        assert_eq!(f.category, SdkCategory::Psg);
    }
}

#[test]
fn registry_contains_all_sprite_functions() {
    let reg = SdkRegistry::new();
    let sprite_names = [
        "sprite_attr",
        "sprite_get_width",
        "sprite_get_height",
        "sprite_init",
        "sprite_set",
        "sprite_set_pos",
        "sprite_hide",
        "sprite_clear",
        "sprite_clear_all",
        "sprite_set_link",
    ];
    for name in &sprite_names {
        assert!(
            reg.is_sdk_function(name),
            "Sprite function '{name}' not found in registry"
        );
        let f = reg.lookup(name).unwrap();
        assert_eq!(f.category, SdkCategory::Sprite);
    }
}

#[test]
fn registry_contains_all_input_functions() {
    let reg = SdkRegistry::new();
    let input_names = [
        "joy1_read",
        "joy2_read",
        "input_init",
        "input_read",
        "input_update",
        "input_held",
        "input_pressed",
        "input_released",
        "input_is_6button",
    ];
    for name in &input_names {
        assert!(
            reg.is_sdk_function(name),
            "Input function '{name}' not found in registry"
        );
        let f = reg.lookup(name).unwrap();
        assert_eq!(f.category, SdkCategory::Input);
    }
}

#[test]
fn registry_lookup_nonexistent_returns_none() {
    let reg = SdkRegistry::new();
    assert!(reg.lookup("nonexistent_function").is_none());
    assert!(!reg.is_sdk_function("nonexistent_function"));
}

#[test]
fn registry_inline_vs_library_classification() {
    let reg = SdkRegistry::new();

    // Inline functions
    let inline_funcs = [
        "vdp_set_reg",
        "vdp_get_status",
        "ym_write0",
        "ym_write1",
        "psg_write",
        "sprite_attr",
        "joy1_read",
        "joy2_read",
    ];
    for name in &inline_funcs {
        let f = reg.lookup(name).unwrap();
        assert_eq!(
            f.kind,
            SdkFunctionKind::Inline,
            "'{name}' should be Inline"
        );
    }

    // Library functions
    let library_funcs = [
        "vdp_init",
        "vdp_load_palette",
        "ym_init",
        "ym_set_freq",
        "psg_set_tone",
        "sprite_init",
        "input_init",
    ];
    for name in &library_funcs {
        let f = reg.lookup(name).unwrap();
        assert_eq!(
            f.kind,
            SdkFunctionKind::Library,
            "'{name}' should be Library"
        );
    }
}

#[test]
fn registry_param_counts() {
    let reg = SdkRegistry::new();

    assert_eq!(reg.lookup("vdp_set_reg").unwrap().param_count, 2);
    assert_eq!(reg.lookup("vdp_get_status").unwrap().param_count, 0);
    assert_eq!(reg.lookup("vdp_load_palette").unwrap().param_count, 3);
    assert_eq!(reg.lookup("sprite_set").unwrap().param_count, 5);
    assert_eq!(reg.lookup("ym_write_op").unwrap().param_count, 4);
    assert_eq!(reg.lookup("psg_write").unwrap().param_count, 1);
}

#[test]
fn registry_return_values() {
    let reg = SdkRegistry::new();

    // Functions that return values
    assert!(reg.lookup("vdp_get_status").unwrap().has_return);
    assert!(reg.lookup("vdp_in_vblank").unwrap().has_return);
    assert!(reg.lookup("joy1_read").unwrap().has_return);
    assert!(reg.lookup("ym_read_status").unwrap().has_return);
    assert!(reg.lookup("vdp_get_frame_count").unwrap().has_return);

    // Functions that don't return
    assert!(!reg.lookup("vdp_init").unwrap().has_return);
    assert!(!reg.lookup("psg_write").unwrap().has_return);
    assert!(!reg.lookup("sprite_init").unwrap().has_return);
}

#[test]
fn registry_ym_patch_functions() {
    let reg = SdkRegistry::new();
    let patches = [
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
    ];
    for name in &patches {
        let f = reg.lookup(name).unwrap();
        assert_eq!(f.category, SdkCategory::Ym2612);
        assert_eq!(f.kind, SdkFunctionKind::Library);
        assert_eq!(f.param_count, 1);
        assert!(!f.has_return);
    }
}

#[test]
fn registry_default_trait() {
    let reg = SdkRegistry::default();
    assert!(reg.is_sdk_function("vdp_init"));
}

// ============================================================================
// Inline Generator Tests
// ============================================================================

#[test]
fn inline_generate_vdp_set_reg() {
    let insts = SdkInlineGenerator::generate("vdp_set_reg").unwrap();
    assert!(!insts.is_empty());
    // Should contain a write to VDP_CTRL
    assert!(insts.iter().any(|i| matches!(
        i,
        M68kInst::Move(Size::Word, _, Operand::AbsLong(VDP_CTRL))
    )));
}

#[test]
fn inline_generate_vdp_get_status() {
    let insts = SdkInlineGenerator::generate("vdp_get_status").unwrap();
    assert_eq!(insts.len(), 1);
    // Should read from VDP_CTRL into D0
    assert!(matches!(
        &insts[0],
        M68kInst::Move(Size::Word, Operand::AbsLong(VDP_CTRL), Operand::DataReg(DataReg::D0))
    ));
}

#[test]
fn inline_generate_ym_write0_has_delay() {
    let insts = SdkInlineGenerator::generate("ym_write0").unwrap();
    // ym_write0 should have NOPs for timing delay
    let nop_count = insts.iter().filter(|i| matches!(i, M68kInst::Nop)).count();
    assert_eq!(nop_count, 24, "ym_write0 should have 24 NOPs (2x12 delay)");
}

#[test]
fn inline_generate_ym_write1_has_delay() {
    let insts = SdkInlineGenerator::generate("ym_write1").unwrap();
    let nop_count = insts.iter().filter(|i| matches!(i, M68kInst::Nop)).count();
    assert_eq!(nop_count, 24, "ym_write1 should have 24 NOPs (2x12 delay)");
}

#[test]
fn inline_generate_psg_write() {
    let insts = SdkInlineGenerator::generate("psg_write").unwrap();
    assert_eq!(insts.len(), 1);
    assert!(matches!(
        &insts[0],
        M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(PSG_PORT))
    ));
}

#[test]
fn inline_generate_joy1_read() {
    let insts = SdkInlineGenerator::generate("joy1_read").unwrap();
    // Should read from joystick 1 port
    assert!(insts
        .iter()
        .any(|i| matches!(i, M68kInst::Move(Size::Byte, Operand::AbsLong(0xA10003), _))));
    // Should invert bits
    assert!(insts
        .iter()
        .any(|i| matches!(i, M68kInst::Not(Size::Byte, _))));
}

#[test]
fn inline_generate_joy2_read() {
    let insts = SdkInlineGenerator::generate("joy2_read").unwrap();
    // Should read from joystick 2 port
    assert!(insts
        .iter()
        .any(|i| matches!(i, M68kInst::Move(Size::Byte, Operand::AbsLong(0xA10005), _))));
}

#[test]
fn inline_generate_sprite_get_width() {
    let insts = SdkInlineGenerator::generate("sprite_get_width").unwrap();
    assert_eq!(insts.len(), 3);
    // Should shift right by 2, mask, and add 1
    assert!(matches!(
        &insts[0],
        M68kInst::Lsr(Size::Word, Operand::Imm(2), DataReg::D0)
    ));
}

#[test]
fn inline_generate_sprite_get_height() {
    let insts = SdkInlineGenerator::generate("sprite_get_height").unwrap();
    assert_eq!(insts.len(), 2);
    // Should mask lower 2 bits and add 1
    assert!(matches!(&insts[1], M68kInst::Addq(Size::Word, 1, _)));
}

#[test]
fn inline_generate_unknown_returns_error() {
    let result = SdkInlineGenerator::generate("not_a_function");
    assert!(result.is_err());
}

#[test]
fn inline_all_registered_inline_functions_generate() {
    let reg = SdkRegistry::new();
    let inline_names = [
        "vdp_set_reg",
        "vdp_get_status",
        "vdp_set_write_addr",
        "vdp_set_cram_addr",
        "vdp_set_color",
        "vdp_set_background",
        "vdp_in_vblank",
        "ym_read_status",
        "ym_write0",
        "ym_write1",
        "ym_dac_enable",
        "ym_dac_disable",
        "ym_dac_write",
        "psg_write",
        "psg_set_volume",
        "psg_set_noise",
        "psg_stop_channel",
        "psg_note_off",
        "sprite_attr",
        "sprite_get_width",
        "sprite_get_height",
        "joy1_read",
        "joy2_read",
    ];
    for name in &inline_names {
        let f = reg.lookup(name).unwrap();
        assert_eq!(f.kind, SdkFunctionKind::Inline, "'{name}' should be inline");
        let result = SdkInlineGenerator::generate(name);
        assert!(
            result.is_ok(),
            "inline generation failed for '{name}': {:?}",
            result.err()
        );
        assert!(
            !result.unwrap().is_empty(),
            "inline generation for '{name}' returned empty"
        );
    }
}

// ============================================================================
// Library Generator Tests
// ============================================================================

#[test]
fn library_generate_vdp_init_starts_with_label() {
    let mut libgen = SdkLibraryGenerator::new();
    let insts = libgen.generate("vdp_init");
    assert!(matches!(
        &insts[0],
        M68kInst::Label(name) if name == "vdp_init"
    ));
    // Should end with RTS
    assert!(matches!(insts.last(), Some(M68kInst::Rts)));
}

#[test]
fn library_generate_vdp_vsync_tail_calls_vblank_start() {
    let mut libgen = SdkLibraryGenerator::new();
    let insts = libgen.generate("vdp_vsync");
    assert_eq!(insts.len(), 2);
    assert!(matches!(
        &insts[1],
        M68kInst::Bra(target) if target == "vdp_wait_vblank_start"
    ));
}

#[test]
fn library_generate_psg_stop_silences_all_channels() {
    let mut libgen = SdkLibraryGenerator::new();
    let insts = libgen.generate("psg_stop");
    // Should write 4 silence bytes (0x9F, 0xBF, 0xDF, 0xFF)
    let psg_writes: Vec<_> = insts
        .iter()
        .filter(|i| matches!(i, M68kInst::Move(Size::Byte, Operand::Imm(_), Operand::AbsLong(PSG_PORT))))
        .collect();
    assert_eq!(psg_writes.len(), 4);
}

#[test]
fn library_generate_unknown_produces_stub() {
    let mut libgen = SdkLibraryGenerator::new();
    let insts = libgen.generate("unknown_function");
    assert!(matches!(
        &insts[0],
        M68kInst::Label(name) if name == "unknown_function"
    ));
    assert!(matches!(insts.last(), Some(M68kInst::Rts)));
}

#[test]
fn library_generate_unique_labels() {
    let mut libgen = SdkLibraryGenerator::new();
    // Generate two functions that both use labels
    let insts1 = libgen.generate("vdp_wait_vblank_start");
    let insts2 = libgen.generate("vdp_wait_vblank_end");

    // Collect all internal labels (not function-name labels)
    let labels1: Vec<_> = insts1
        .iter()
        .filter_map(|i| match i {
            M68kInst::Label(l) if l.starts_with(".sdk_") => Some(l.clone()),
            _ => None,
        })
        .collect();
    let labels2: Vec<_> = insts2
        .iter()
        .filter_map(|i| match i {
            M68kInst::Label(l) if l.starts_with(".sdk_") => Some(l.clone()),
            _ => None,
        })
        .collect();

    // No label should appear in both sets
    for l in &labels1 {
        assert!(
            !labels2.contains(l),
            "Label '{l}' duplicated across functions"
        );
    }
}

#[test]
fn library_all_registered_library_functions_generate() {
    let reg = SdkRegistry::new();
    let mut libgen = SdkLibraryGenerator::new();
    let library_names = [
        "vdp_init",
        "vdp_vsync",
        "vdp_wait_vblank_start",
        "vdp_wait_vblank_end",
        "vdp_wait_frame",
        "vdp_load_palette",
        "vdp_load_tiles",
        "vdp_set_tile_a",
        "vdp_set_tile_b",
        "vdp_clear_plane_a",
        "vdp_clear_plane_b",
        "vdp_set_hscroll_a",
        "vdp_set_hscroll_b",
        "vdp_set_vscroll_a",
        "vdp_set_vscroll_b",
        "vdp_get_frame_count",
        "vdp_reset_frame_count",
        "ym_init",
        "ym_reset",
        "ym_wait",
        "ym_write_ch",
        "ym_write_op",
        "ym_key_on",
        "ym_key_off",
        "ym_key_on_ops",
        "ym_set_freq",
        "ym_set_algo",
        "ym_set_pan",
        "ym_set_volume",
        "ym_set_lfo",
        "psg_init",
        "psg_set_tone",
        "psg_set_freq",
        "psg_stop",
        "psg_beep",
        "psg_note_on",
        "sprite_init",
        "sprite_set",
        "sprite_set_pos",
        "sprite_hide",
        "sprite_clear",
        "sprite_clear_all",
        "sprite_set_link",
        "input_init",
        "input_read",
        "input_update",
        "input_held",
        "input_pressed",
        "input_released",
        "input_is_6button",
    ];
    for name in &library_names {
        let f = reg.lookup(name).unwrap();
        assert_eq!(
            f.kind,
            SdkFunctionKind::Library,
            "'{name}' should be Library"
        );
        let insts = libgen.generate(name);
        assert!(
            !insts.is_empty(),
            "library generation for '{name}' returned empty"
        );
        // First instruction should be a label
        assert!(
            matches!(&insts[0], M68kInst::Label(l) if l == name),
            "library function '{name}' should start with its label"
        );
    }
}

#[test]
fn library_default_trait() {
    let mut libgen = SdkLibraryGenerator::default();
    let insts = libgen.generate("psg_stop");
    assert!(!insts.is_empty());
}

// ============================================================================
// Dependency Resolution Tests
// ============================================================================

#[test]
fn deps_vdp_vsync_depends_on_vblank_start() {
    let deps = get_sdk_dependencies("vdp_vsync");
    assert!(deps.contains(&"vdp_wait_vblank_start"));
}

#[test]
fn deps_vdp_wait_frame_depends_on_both_vblank() {
    let deps = get_sdk_dependencies("vdp_wait_frame");
    assert!(deps.contains(&"vdp_wait_vblank_start"));
    assert!(deps.contains(&"vdp_wait_vblank_end"));
}

#[test]
fn deps_ym_init_depends_on_write0_keyoff_writeop() {
    let deps = get_sdk_dependencies("ym_init");
    assert!(deps.contains(&"ym_write0"));
    assert!(deps.contains(&"ym_key_off"));
    assert!(deps.contains(&"ym_write_op"));
}

#[test]
fn deps_unknown_has_no_deps() {
    let deps = get_sdk_dependencies("unknown_function");
    assert!(deps.is_empty());
}

#[test]
fn deps_resolve_transitive() {
    // ym_reset -> ym_init -> [ym_write0, ym_key_off, ym_write_op]
    // ym_write_op -> [ym_write0, ym_write1]
    // ym_key_off -> [ym_write0]
    let mut funcs = HashSet::new();
    funcs.insert("ym_reset".to_string());

    let resolved = resolve_dependencies(&funcs);

    assert!(resolved.contains("ym_reset"));
    assert!(resolved.contains("ym_init"));
    assert!(resolved.contains("ym_write0"));
    assert!(resolved.contains("ym_key_off"));
    assert!(resolved.contains("ym_write_op"));
    assert!(resolved.contains("ym_write1"));
}

#[test]
fn deps_resolve_no_deps() {
    let mut funcs = HashSet::new();
    funcs.insert("psg_write".to_string());
    let resolved = resolve_dependencies(&funcs);
    assert_eq!(resolved.len(), 1);
    assert!(resolved.contains("psg_write"));
}

#[test]
fn deps_resolve_psg_chain() {
    // psg_note_on -> [psg_beep, psg_set_tone]
    // psg_beep -> [psg_set_tone]
    let mut funcs = HashSet::new();
    funcs.insert("psg_note_on".to_string());
    let resolved = resolve_dependencies(&funcs);
    assert!(resolved.contains("psg_note_on"));
    assert!(resolved.contains("psg_beep"));
    assert!(resolved.contains("psg_set_tone"));
}

// ============================================================================
// Static Data Tests
// ============================================================================

#[test]
fn static_data_frame_counter_needed() {
    let mut funcs = HashSet::new();
    funcs.insert("vdp_wait_vblank_start".to_string());
    assert!(deps::needs_frame_counter(&funcs));
}

#[test]
fn static_data_frame_counter_not_needed() {
    let mut funcs = HashSet::new();
    funcs.insert("psg_write".to_string());
    assert!(!deps::needs_frame_counter(&funcs));
}

#[test]
fn static_data_op_offsets_needed() {
    let mut funcs = HashSet::new();
    funcs.insert("ym_write_op".to_string());
    assert!(deps::needs_op_offsets(&funcs));
}

#[test]
fn static_data_op_offsets_not_needed() {
    let mut funcs = HashSet::new();
    funcs.insert("ym_write0".to_string());
    assert!(!deps::needs_op_offsets(&funcs));
}

#[test]
fn static_data_empty_for_no_special_functions() {
    let mut funcs = HashSet::new();
    funcs.insert("psg_write".to_string());
    let data = generate_static_data(&funcs);
    assert!(data.is_empty());
}

#[test]
fn static_data_contains_frame_counter() {
    let mut funcs = HashSet::new();
    funcs.insert("vdp_init".to_string());
    let data = generate_static_data(&funcs);
    assert!(data.iter().any(|i| matches!(
        i,
        M68kInst::Label(l) if l == "__sdk_frame_count"
    )));
}

#[test]
fn static_data_contains_op_offsets() {
    let mut funcs = HashSet::new();
    funcs.insert("ym_write_op".to_string());
    let data = generate_static_data(&funcs);
    assert!(data.iter().any(|i| matches!(
        i,
        M68kInst::Label(l) if l == "__sdk_op_offsets"
    )));
}

#[test]
fn static_data_both_frame_counter_and_op_offsets() {
    let mut funcs = HashSet::new();
    funcs.insert("vdp_init".to_string());
    funcs.insert("ym_init".to_string());
    let data = generate_static_data(&funcs);
    let has_frame_counter = data
        .iter()
        .any(|i| matches!(i, M68kInst::Label(l) if l == "__sdk_frame_count"));
    let has_op_offsets = data
        .iter()
        .any(|i| matches!(i, M68kInst::Label(l) if l == "__sdk_op_offsets"));
    assert!(has_frame_counter);
    assert!(has_op_offsets);
}

// ============================================================================
// Hardware Address Constant Tests
// ============================================================================

#[test]
fn hardware_addresses_correct() {
    assert_eq!(VDP_DATA, 0xC00000);
    assert_eq!(VDP_CTRL, 0xC00004);
    assert_eq!(PSG_PORT, 0xC00011);
    assert_eq!(YM_ADDR0, 0xA04000);
    assert_eq!(YM_DATA0, 0xA04001);
    assert_eq!(YM_ADDR1, 0xA04002);
    assert_eq!(YM_DATA1, 0xA04003);
}
