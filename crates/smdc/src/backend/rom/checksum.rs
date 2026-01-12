//! Sega Megadrive ROM checksum calculation
//!
//! The checksum is calculated as the 16-bit sum of all words from
//! offset 0x200 to the end of the ROM, stored at offset 0x18E.

/// Calculate the Sega Megadrive ROM checksum
///
/// The checksum is calculated as the sum of all 16-bit words
/// from offset 0x200 to the end of the ROM.
///
/// # Arguments
/// * `rom_data` - The complete ROM data
///
/// # Returns
/// The 16-bit checksum value
pub fn calculate_checksum(rom_data: &[u8]) -> u16 {
    // Checksum is calculated from offset 0x200 onwards
    if rom_data.len() <= 0x200 {
        return 0;
    }

    let data = &rom_data[0x200..];
    let mut sum: u32 = 0;

    // Sum 16-bit words (big-endian)
    for chunk in data.chunks(2) {
        let word = if chunk.len() == 2 {
            ((chunk[0] as u32) << 8) | (chunk[1] as u32)
        } else {
            // Odd byte at end, pad with 0
            (chunk[0] as u32) << 8
        };
        sum = sum.wrapping_add(word);
    }

    (sum & 0xFFFF) as u16
}

/// Update the checksum in a ROM image
///
/// Calculates the checksum and writes it to offset 0x18E (big-endian).
///
/// # Arguments
/// * `rom_data` - Mutable reference to the ROM data
pub fn update_checksum(rom_data: &mut [u8]) {
    if rom_data.len() < 0x190 {
        return; // ROM too small for checksum
    }

    let checksum = calculate_checksum(rom_data);

    // Store checksum at offset 0x18E (big-endian)
    rom_data[0x18E] = (checksum >> 8) as u8;
    rom_data[0x18F] = (checksum & 0xFF) as u8;
}

/// Verify the checksum in a ROM image
///
/// # Arguments
/// * `rom_data` - The ROM data to verify
///
/// # Returns
/// `true` if the checksum is valid, `false` otherwise
pub fn verify_checksum(rom_data: &[u8]) -> bool {
    if rom_data.len() < 0x190 {
        return false;
    }

    let stored = ((rom_data[0x18E] as u16) << 8) | (rom_data[0x18F] as u16);
    let calculated = calculate_checksum(rom_data);

    stored == calculated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_empty() {
        let data = vec![0u8; 0x200];
        assert_eq!(calculate_checksum(&data), 0);
    }

    #[test]
    fn test_checksum_simple() {
        let mut data = vec![0u8; 0x204];
        // Add some data at 0x200
        data[0x200] = 0x12;
        data[0x201] = 0x34;
        data[0x202] = 0x56;
        data[0x203] = 0x78;
        // Checksum should be 0x1234 + 0x5678 = 0x68AC
        assert_eq!(calculate_checksum(&data), 0x68AC);
    }

    #[test]
    fn test_update_and_verify() {
        let mut data = vec![0u8; 0x400];
        data[0x200] = 0xAB;
        data[0x201] = 0xCD;

        update_checksum(&mut data);
        assert!(verify_checksum(&data));
    }
}
