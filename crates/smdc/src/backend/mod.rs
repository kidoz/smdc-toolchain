//! Backend trait and implementations
//!
//! Backends are responsible for:
//! 1. Converting IR to target-specific instructions
//! 2. Register allocation
//! 3. Emitting output in the requested format

pub mod m68k;
pub mod rom;

use crate::common::CompileResult;
use crate::ir::IrModule;
use std::path::Path;

pub use m68k::M68kBackend;
pub use rom::RomBackend;

/// Output format for backends
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Assembly text (.s)
    #[default]
    Assembly,
    /// Object file (.o)
    Object,
    /// Raw binary (.bin)
    Binary,
}

/// Configuration options for backends
#[derive(Debug, Clone, Default)]
pub struct BackendConfig {
    pub output_format: OutputFormat,
    pub optimize_level: u8,
    pub debug_info: bool,
    pub dump_ir: bool,
    pub verbose: bool,
}

/// ROM-specific configuration for Sega Genesis
#[derive(Debug, Clone)]
pub struct RomConfig {
    /// "SEGA MEGA DRIVE " or "SEGA GENESIS    " (16 chars)
    pub system_name: String,
    /// Copyright string "(C)XXXX YYYY.ZZZ" (16 chars)
    pub copyright: String,
    /// Japanese title (48 chars)
    pub domestic_name: String,
    /// Overseas title (48 chars)
    pub overseas_name: String,
    /// Serial number "GM XXXXXXXX-XX" (14 chars)
    pub serial_number: String,
    /// ROM start address (typically 0x000000)
    pub rom_start: u32,
    /// ROM end address
    pub rom_end: u32,
    /// RAM start (typically 0xFF0000)
    pub ram_start: u32,
    /// RAM end (typically 0xFFFFFF)
    pub ram_end: u32,
    /// Region code
    pub region: RomRegion,
    /// Entry point address (default: 0x200)
    pub entry_point: u32,
    /// Initial stack pointer (default: 0x00FFE000)
    pub stack_pointer: u32,
    /// Extra memory (SRAM/EEPROM) configuration
    pub extra_memory: Option<ExtraMemory>,
}

impl Default for RomConfig {
    fn default() -> Self {
        Self {
            system_name: "SEGA MEGA DRIVE ".to_string(),
            copyright: "(C)2024 SMD-SDK ".to_string(),
            domestic_name: "SMD GAME".to_string(),
            overseas_name: "SMD GAME".to_string(),
            serial_number: "GM 00000000-00".to_string(),
            rom_start: 0x00000000,
            rom_end: 0x003FFFFF,
            ram_start: 0x00FF0000,
            ram_end: 0x00FFFFFF,
            region: RomRegion::All,
            entry_point: 0x200,
            stack_pointer: 0x00FFE000,
            extra_memory: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum RomRegion {
    Japan,
    Americas,
    Europe,
    #[default]
    All,
}

impl RomRegion {
    pub fn code(&self) -> &'static str {
        match self {
            RomRegion::Japan => "J               ",
            RomRegion::Americas => "U               ",
            RomRegion::Europe => "E               ",
            RomRegion::All => "JUE             ",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtraMemory {
    pub memory_type: ExtraMemoryType,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum ExtraMemoryType {
    Sram,
    Eeprom,
}

/// Output from a backend
pub enum BackendOutput {
    /// Text output (assembly)
    Text(String),
    /// Binary output (object file or ROM)
    Binary(Vec<u8>),
}

impl BackendOutput {
    pub fn write_to(&self, path: &Path) -> std::io::Result<()> {
        match self {
            BackendOutput::Text(s) => std::fs::write(path, s),
            BackendOutput::Binary(b) => std::fs::write(path, b),
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            BackendOutput::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_binary(&self) -> Option<&[u8]> {
        match self {
            BackendOutput::Binary(b) => Some(b),
            _ => None,
        }
    }
}

/// Trait for code generation backends
///
/// A backend is responsible for converting IR to target-specific output.
pub trait Backend: Send + Sync {
    /// The name of this backend (e.g., "m68k", "rom")
    fn name(&self) -> &'static str;

    /// Target architecture description
    fn target(&self) -> &'static str;

    /// Supported output formats
    fn supported_formats(&self) -> &'static [OutputFormat];

    /// Generate output from IR
    fn generate(
        &self,
        module: &IrModule,
        config: &BackendConfig,
    ) -> CompileResult<BackendOutput>;
}

/// Registry of available backends
pub struct BackendRegistry {
    backends: Vec<Box<dyn Backend>>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self { backends: Vec::new() }
    }

    pub fn register(&mut self, backend: Box<dyn Backend>) {
        self.backends.push(backend);
    }

    pub fn find_by_name(&self, name: &str) -> Option<&dyn Backend> {
        self.backends.iter()
            .find(|b| b.name() == name)
            .map(|b| b.as_ref())
    }

    pub fn default_backend(&self) -> Option<&dyn Backend> {
        self.backends.first().map(|b| b.as_ref())
    }

    pub fn list(&self) -> impl Iterator<Item = &dyn Backend> {
        self.backends.iter().map(|b| b.as_ref())
    }
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}
