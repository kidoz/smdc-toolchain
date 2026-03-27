//! SDK function definitions and code generation for Sega Genesis hardware
//!
//! This module provides built-in implementations for SDK functions,
//! eliminating the need for separate SDK source files.

mod deps;
mod inline;
mod library;
mod registry;

pub use deps::{generate_static_data, get_sdk_dependencies, resolve_dependencies};
pub use inline::SdkInlineGenerator;
pub use library::SdkLibraryGenerator;
pub use registry::SdkRegistry;

// ============================================================================
// Hardware Addresses
// ============================================================================

/// VDP (Video Display Processor) registers
pub const VDP_DATA: u32 = 0xC00000;
pub const VDP_CTRL: u32 = 0xC00004;

/// PSG (Programmable Sound Generator) port
pub const PSG_PORT: u32 = 0xC00011;

/// YM2612 FM synthesizer ports
pub const YM_ADDR0: u32 = 0xA04000;
pub const YM_DATA0: u32 = 0xA04001;
pub const YM_ADDR1: u32 = 0xA04002;
pub const YM_DATA1: u32 = 0xA04003;

/// SRAM (battery-backed save RAM)
pub const SRAM_CTRL: u32 = 0xA130F1;
pub const SRAM_BASE: u32 = 0x200001; // Odd-byte addressing

// ============================================================================
// SDK Function Classification
// ============================================================================

/// How an SDK function should be generated
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkFunctionKind {
    /// Simple function - inline at call site (1-20 instructions)
    Inline,
    /// Complex function - generate as library function (loops, state, etc.)
    Library,
}

/// SDK function category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkCategory {
    Vdp,
    Sprite,
    Input,
    Ym2612,
    Psg,
    Util,
    Sram,
}

/// SDK function definition
#[derive(Debug, Clone)]
pub struct SdkFunction {
    pub name: &'static str,
    pub kind: SdkFunctionKind,
    pub category: SdkCategory,
    pub param_count: usize,
    pub has_return: bool,
}

#[cfg(test)]
mod tests;
