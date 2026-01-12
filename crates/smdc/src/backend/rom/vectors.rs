//! M68k exception vector table for Sega Megadrive/Genesis
//!
//! The vector table occupies bytes 0x000-0x0FF and contains addresses
//! for exception handlers and the initial stack pointer/program counter.

/// M68k exception vector table (bytes 0x000-0x0FF)
///
/// This table contains 64 32-bit addresses (256 bytes total).
#[derive(Debug, Clone)]
pub struct VectorTable {
    /// Initial stack pointer (SP) - typically 0x00FFE000 or similar
    pub initial_sp: u32,
    /// Initial program counter (PC) - entry point
    pub initial_pc: u32,
    /// Bus error handler
    pub bus_error: u32,
    /// Address error handler
    pub address_error: u32,
    /// Illegal instruction handler
    pub illegal_instruction: u32,
    /// Division by zero handler
    pub divide_by_zero: u32,
    /// CHK instruction handler
    pub chk: u32,
    /// TRAPV instruction handler
    pub trapv: u32,
    /// Privilege violation handler
    pub privilege_violation: u32,
    /// Trace handler
    pub trace: u32,
    /// Line A emulator
    pub line_a: u32,
    /// Line F emulator
    pub line_f: u32,
    /// Reserved vectors (12 entries, indices 12-23)
    pub reserved1: [u32; 12],
    /// Spurious interrupt (index 24)
    pub spurious: u32,
    /// Level 1-7 auto-vectored interrupts (indices 25-31)
    pub auto_vectors: [u32; 7],
    /// TRAP #0-15 vectors (indices 32-47)
    pub trap_vectors: [u32; 16],
    /// Reserved vectors (16 entries, indices 48-63)
    pub reserved2: [u32; 16],
}

impl VectorTable {
    /// Create a new vector table with default handlers
    ///
    /// # Arguments
    /// * `entry_point` - Address of the program entry point (initial PC)
    /// * `stack_pointer` - Initial stack pointer address
    pub fn new(entry_point: u32, stack_pointer: u32) -> Self {
        // Use entry point as default handler for all exceptions
        let default_handler = entry_point;

        Self {
            initial_sp: stack_pointer,
            initial_pc: entry_point,
            bus_error: default_handler,
            address_error: default_handler,
            illegal_instruction: default_handler,
            divide_by_zero: default_handler,
            chk: default_handler,
            trapv: default_handler,
            privilege_violation: default_handler,
            trace: default_handler,
            line_a: default_handler,
            line_f: default_handler,
            reserved1: [default_handler; 12],
            spurious: default_handler,
            auto_vectors: [default_handler; 7],
            trap_vectors: [default_handler; 16],
            reserved2: [default_handler; 16],
        }
    }

    /// Set the HBlank interrupt handler (Level 4 auto-vector)
    pub fn set_hblank_handler(&mut self, addr: u32) {
        // Level 4 is index 3 in auto_vectors (Level 1 is index 0)
        self.auto_vectors[3] = addr;
    }

    /// Set the VBlank interrupt handler (Level 6 auto-vector)
    pub fn set_vblank_handler(&mut self, addr: u32) {
        // Level 6 is index 5 in auto_vectors
        self.auto_vectors[5] = addr;
    }

    /// Set the external interrupt handler (Level 2 auto-vector)
    pub fn set_external_handler(&mut self, addr: u32) {
        // Level 2 is index 1 in auto_vectors
        self.auto_vectors[1] = addr;
    }

    /// Convert vector table to bytes (256 bytes total)
    pub fn to_bytes(&self) -> [u8; 256] {
        let mut bytes = [0u8; 256];
        let mut offset = 0;

        // Helper closure to write u32 big-endian
        let mut write_u32 = |val: u32| {
            bytes[offset..offset+4].copy_from_slice(&val.to_be_bytes());
            offset += 4;
        };

        // Initial SP and PC
        write_u32(self.initial_sp);
        write_u32(self.initial_pc);

        // Exception vectors (indices 2-11)
        write_u32(self.bus_error);
        write_u32(self.address_error);
        write_u32(self.illegal_instruction);
        write_u32(self.divide_by_zero);
        write_u32(self.chk);
        write_u32(self.trapv);
        write_u32(self.privilege_violation);
        write_u32(self.trace);
        write_u32(self.line_a);
        write_u32(self.line_f);

        // Reserved (indices 12-23)
        for &v in &self.reserved1 {
            write_u32(v);
        }

        // Spurious interrupt (index 24)
        write_u32(self.spurious);

        // Auto-vectors (indices 25-31, Levels 1-7)
        for &v in &self.auto_vectors {
            write_u32(v);
        }

        // TRAP vectors (indices 32-47)
        for &v in &self.trap_vectors {
            write_u32(v);
        }

        // Reserved (indices 48-63)
        for &v in &self.reserved2 {
            write_u32(v);
        }

        bytes
    }
}

impl Default for VectorTable {
    fn default() -> Self {
        // Default entry point at 0x200 (after header), stack at 0xFFE000
        Self::new(0x200, 0x00FFE000)
    }
}
