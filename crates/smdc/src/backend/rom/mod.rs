//! ROM builder backend for Sega Megadrive/Genesis
//!
//! This backend generates complete ROM files including:
//! - Vector table (256 bytes at 0x000-0x0FF)
//! - ROM header (256 bytes at 0x100-0x1FF)
//! - Code and data sections
//! - Calculated checksum

mod builder;
mod checksum;
mod header;
mod vectors;

pub use builder::RomBuilder;
pub use checksum::{calculate_checksum, update_checksum, verify_checksum};
pub use header::RomHeader;
pub use vectors::VectorTable;

use crate::backend::m68k::{Assembler, CodeGenerator, generate_sym_file};
use crate::backend::{Backend, BackendConfig, BackendOutput, OutputFormat, RomConfig};
use crate::common::{CompileError, CompileResult};
use crate::ir::IrModule;
use std::collections::HashMap;

/// ROM builder backend
///
/// This backend generates a complete Sega Megadrive/Genesis ROM file.
/// It uses the M68k backend internally to generate code, then wraps it
/// with the vector table, header, and checksum.
pub struct RomBackend {
    rom_config: RomConfig,
}

impl RomBackend {
    pub fn new() -> Self {
        Self {
            rom_config: RomConfig::default(),
        }
    }

    pub fn with_config(config: RomConfig) -> Self {
        Self { rom_config: config }
    }

    /// Set the ROM configuration
    pub fn set_config(&mut self, config: RomConfig) {
        self.rom_config = config;
    }

    /// Build a ROM from the given IR module.
    ///
    /// Returns the ROM binary and optionally the assembler symbol table
    /// (when `config.debug_info` is true, for `.sym` file generation).
    pub fn build_rom(
        &self,
        module: &IrModule,
        config: &BackendConfig,
    ) -> CompileResult<(Vec<u8>, Option<HashMap<String, u32>>)> {
        // 1. Generate M68k instructions from IR
        let mut codegen = CodeGenerator::new();
        if config.debug_info {
            if let Some(di) = &module.debug_info {
                codegen.set_debug_info(di.filename.clone(), di.source.clone());
            }
        }
        let instructions = codegen.generate_instructions(module)?;

        // 2. Assemble to binary (code starts at 0x200 after header)
        let mut assembler = Assembler::new(self.rom_config.entry_point);
        let code_binary = assembler
            .assemble(&instructions)
            .map_err(|e| CompileError::backend(format!("assembly error: {e}")))?;

        let symbols = if config.debug_info {
            Some(assembler.symbols().clone())
        } else {
            None
        };

        // 3. Build ROM with actual code
        let mut builder = RomBuilder::new(self.rom_config.clone());
        builder.set_code(code_binary);
        let rom = builder.build()?;

        Ok((rom, symbols))
    }
}

impl Default for RomBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for RomBackend {
    fn name(&self) -> &'static str {
        "rom"
    }

    fn target(&self) -> &'static str {
        "Sega Megadrive/Genesis ROM"
    }

    fn supported_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::Binary]
    }

    fn generate(
        &self,
        module: &IrModule,
        _ctx: &crate::frontend::CompileContext,
        config: &BackendConfig,
    ) -> CompileResult<BackendOutput> {
        if config.output_format != OutputFormat::Binary {
            return Err(CompileError::backend(
                "ROM backend only supports binary output format",
            ));
        }

        if config.verbose {
            eprintln!("Building Sega Megadrive/Genesis ROM...");
        }

        let (rom, symbols) = self.build_rom(module, config)?;

        if config.verbose {
            eprintln!("ROM size: {} bytes ({} KB)", rom.len(), rom.len() / 1024);
        }

        let mut output = BackendOutput::binary(rom);

        // Generate .sym file when debug info is enabled
        if let Some(sym_table) = symbols {
            let sym_content = generate_sym_file(&sym_table);
            output.side_artifacts.push(("sym".to_string(), sym_content));
        }

        Ok(output)
    }
}
