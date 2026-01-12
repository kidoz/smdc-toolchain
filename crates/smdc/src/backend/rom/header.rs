//! Sega Megadrive/Genesis ROM header generation
//!
//! The ROM header occupies bytes 0x100-0x1FF and contains metadata
//! about the game including title, region, and memory addresses.

/// Sega Megadrive/Genesis ROM header (bytes 0x100-0x1FF)
#[derive(Debug, Clone)]
pub struct RomHeader {
    /// "SEGA MEGA DRIVE " or "SEGA GENESIS    " (16 bytes)
    pub system_name: [u8; 16],
    /// Copyright string "(C)XXXX YYYY.ZZZ" (16 bytes)
    pub copyright: [u8; 16],
    /// Domestic (Japanese) game name (48 bytes)
    pub domestic_name: [u8; 48],
    /// Overseas game name (48 bytes)
    pub overseas_name: [u8; 48],
    /// Serial number "GM XXXXXXXX-XX" (14 bytes)
    pub serial_number: [u8; 14],
    /// Checksum (2 bytes) - calculated from ROM data
    pub checksum: u16,
    /// I/O support "J" for 3-button, "6" for 6-button, etc (16 bytes)
    pub io_support: [u8; 16],
    /// ROM start address (4 bytes)
    pub rom_start: u32,
    /// ROM end address (4 bytes)
    pub rom_end: u32,
    /// RAM start address (4 bytes)
    pub ram_start: u32,
    /// RAM end address (4 bytes)
    pub ram_end: u32,
    /// SRAM info (12 bytes)
    pub sram_info: [u8; 12],
    /// Modem info (12 bytes)
    pub modem_info: [u8; 12],
    /// Reserved (40 bytes)
    pub reserved: [u8; 40],
    /// Region codes (16 bytes) "JUE" for all regions
    pub region: [u8; 16],
}

impl RomHeader {
    pub fn new() -> Self {
        Self {
            system_name: *b"SEGA MEGA DRIVE ",
            copyright: *b"(C)2024 SMD-SDK ",
            domestic_name: Self::pad_to_48(b"SMD GAME"),
            overseas_name: Self::pad_to_48(b"SMD GAME"),
            serial_number: *b"GM 00000000-00",
            checksum: 0,
            io_support: *b"J               ",
            rom_start: 0x00000000,
            rom_end: 0x003FFFFF,
            ram_start: 0x00FF0000,
            ram_end: 0x00FFFFFF,
            sram_info: *b"            ",
            modem_info: *b"            ",
            reserved: [0x20; 40],
            region: *b"JUE             ",
        }
    }

    /// Set the system name
    pub fn set_system_name(&mut self, name: &str) {
        self.system_name = Self::pad_to_16(name.as_bytes());
    }

    /// Set game names (domestic/Japanese and overseas)
    pub fn set_game_name(&mut self, domestic: &str, overseas: &str) {
        self.domestic_name = Self::pad_to_48(domestic.as_bytes());
        self.overseas_name = Self::pad_to_48(overseas.as_bytes());
    }

    /// Set ROM size (updates rom_end based on start + size)
    pub fn set_rom_size(&mut self, size: u32) {
        self.rom_end = self.rom_start + size - 1;
    }

    /// Set region codes
    pub fn set_region(&mut self, region: &str) {
        self.region = Self::pad_to_16(region.as_bytes());
    }

    /// Convert header to bytes (256 bytes total)
    pub fn to_bytes(&self) -> [u8; 256] {
        let mut bytes = [0x20u8; 256]; // Fill with spaces
        let mut offset = 0;

        // System name (16 bytes)
        bytes[offset..offset+16].copy_from_slice(&self.system_name);
        offset += 16;

        // Copyright (16 bytes)
        bytes[offset..offset+16].copy_from_slice(&self.copyright);
        offset += 16;

        // Domestic name (48 bytes)
        bytes[offset..offset+48].copy_from_slice(&self.domestic_name);
        offset += 48;

        // Overseas name (48 bytes)
        bytes[offset..offset+48].copy_from_slice(&self.overseas_name);
        offset += 48;

        // Serial number (14 bytes)
        bytes[offset..offset+14].copy_from_slice(&self.serial_number);
        offset += 14;

        // Checksum (2 bytes, big-endian)
        bytes[offset..offset+2].copy_from_slice(&self.checksum.to_be_bytes());
        offset += 2;

        // IO support (16 bytes)
        bytes[offset..offset+16].copy_from_slice(&self.io_support);
        offset += 16;

        // ROM addresses (big-endian)
        bytes[offset..offset+4].copy_from_slice(&self.rom_start.to_be_bytes());
        offset += 4;
        bytes[offset..offset+4].copy_from_slice(&self.rom_end.to_be_bytes());
        offset += 4;

        // RAM addresses
        bytes[offset..offset+4].copy_from_slice(&self.ram_start.to_be_bytes());
        offset += 4;
        bytes[offset..offset+4].copy_from_slice(&self.ram_end.to_be_bytes());
        offset += 4;

        // SRAM info (12 bytes)
        bytes[offset..offset+12].copy_from_slice(&self.sram_info);
        offset += 12;

        // Modem info (12 bytes)
        bytes[offset..offset+12].copy_from_slice(&self.modem_info);
        offset += 12;

        // Reserved (40 bytes)
        bytes[offset..offset+40].copy_from_slice(&self.reserved);
        offset += 40;

        // Region (16 bytes)
        bytes[offset..offset+16].copy_from_slice(&self.region);

        bytes
    }

    fn pad_to_16(input: &[u8]) -> [u8; 16] {
        let mut result = [0x20u8; 16];
        let len = input.len().min(16);
        result[..len].copy_from_slice(&input[..len]);
        result
    }

    fn pad_to_48(input: &[u8]) -> [u8; 48] {
        let mut result = [0x20u8; 48];
        let len = input.len().min(48);
        result[..len].copy_from_slice(&input[..len]);
        result
    }
}

impl Default for RomHeader {
    fn default() -> Self {
        Self::new()
    }
}
