//! ROM builder that assembles all components into a complete ROM

use super::{RomHeader, VectorTable, update_checksum};
use crate::backend::RomConfig;
use crate::common::CompileResult;

/// ROM builder that assembles all components into a complete ROM
pub struct RomBuilder {
    #[allow(dead_code)]
    config: RomConfig,
    header: RomHeader,
    vectors: VectorTable,
    code: Vec<u8>,
    data: Vec<u8>,
}

impl RomBuilder {
    /// Create a new ROM builder with the given configuration
    pub fn new(config: RomConfig) -> Self {
        let mut header = RomHeader::new();
        header.set_game_name(&config.domestic_name, &config.overseas_name);
        header.set_system_name(&config.system_name);

        let vectors = VectorTable::new(config.entry_point, config.stack_pointer);

        Self {
            config,
            header,
            vectors,
            code: Vec::new(),
            data: Vec::new(),
        }
    }

    /// Set the code section (compiled program)
    pub fn set_code(&mut self, code: Vec<u8>) {
        self.code = code;
    }

    /// Set the data section
    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data = data;
    }

    /// Set the entry point address
    pub fn set_entry_point(&mut self, addr: u32) {
        self.vectors.initial_pc = addr;
    }

    /// Set the initial stack pointer
    pub fn set_stack_pointer(&mut self, addr: u32) {
        self.vectors.initial_sp = addr;
    }

    /// Set the VBlank handler address
    pub fn set_vblank_handler(&mut self, addr: u32) {
        self.vectors.set_vblank_handler(addr);
    }

    /// Set the HBlank handler address
    pub fn set_hblank_handler(&mut self, addr: u32) {
        self.vectors.set_hblank_handler(addr);
    }

    /// Build the complete ROM
    pub fn build(mut self) -> CompileResult<Vec<u8>> {
        // Calculate total content size
        let content_size = self.code.len() + self.data.len();

        // Calculate ROM size (round up to power of 2, minimum 64KB)
        let rom_size = Self::round_to_power_of_two(0x200 + content_size).max(0x10000);

        // Update header with ROM size
        self.header.set_rom_size(rom_size as u32);

        // Allocate ROM buffer
        let mut rom = vec![0xFFu8; rom_size];

        // Write vector table (0x000-0x0FF)
        rom[0x000..0x100].copy_from_slice(&self.vectors.to_bytes());

        // Write header (0x100-0x1FF)
        rom[0x100..0x200].copy_from_slice(&self.header.to_bytes());

        // Write code (starting at 0x200)
        let code_end = (0x200 + self.code.len()).min(rom_size);
        if !self.code.is_empty() {
            rom[0x200..code_end].copy_from_slice(&self.code[..code_end - 0x200]);
        }

        // Write data (after code)
        let data_start = 0x200 + self.code.len();
        let data_end = (data_start + self.data.len()).min(rom_size);
        if !self.data.is_empty() && data_start < rom_size {
            rom[data_start..data_end].copy_from_slice(&self.data[..data_end - data_start]);
        }

        // Calculate and write checksum
        update_checksum(&mut rom);

        Ok(rom)
    }

    /// Round up to the next power of 2
    fn round_to_power_of_two(n: usize) -> usize {
        if n == 0 {
            return 1;
        }
        let mut power = 1;
        while power < n {
            power *= 2;
        }
        power
    }
}

impl Default for RomBuilder {
    fn default() -> Self {
        Self::new(RomConfig::default())
    }
}
