//! M68k code generation backend
//!
//! This backend generates Motorola 68000 assembly code from the shared IR,
//! targeting the Sega Megadrive/Genesis console.

mod assembler;
mod emit;
mod encoder;
mod m68k;
pub mod sdk;

pub use assembler::Assembler;
pub use emit::CodeGenerator;
pub use encoder::{EncodeError, InstructionEncoder};
pub use m68k::*;
pub use sdk::{SdkFunction, SdkFunctionKind, SdkRegistry};

use crate::backend::{Backend, BackendConfig, BackendOutput, OutputFormat};
use crate::common::CompileResult;
use crate::ir::IrModule;

/// M68k assembly backend
pub struct M68kBackend;

impl M68kBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for M68kBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for M68kBackend {
    fn name(&self) -> &'static str {
        "m68k"
    }

    fn target(&self) -> &'static str {
        "Motorola 68000 (Sega Megadrive/Genesis)"
    }

    fn supported_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::Assembly]
    }

    fn generate(
        &self,
        module: &IrModule,
        _ctx: &crate::frontend::CompileContext,
        config: &BackendConfig,
    ) -> CompileResult<BackendOutput> {
        if config.verbose {
            eprintln!("Generating M68k assembly...");
        }

        let mut codegen = CodeGenerator::new();
        let asm = codegen.generate(module)?;

        Ok(BackendOutput::Text(asm))
    }
}
