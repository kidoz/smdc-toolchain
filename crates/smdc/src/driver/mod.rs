//! Compilation driver and pipeline orchestration

use crate::common::{CompileResult, DiagnosticReporter};
use crate::frontend::{Frontend, FrontendConfig, FrontendRegistry, CompileContext};
use crate::backend::{Backend, BackendConfig, BackendRegistry, BackendOutput};
use crate::ir::IrModule;
use std::path::Path;

/// Compilation pipeline that coordinates frontends and backends
pub struct Pipeline {
    frontends: FrontendRegistry,
    backends: BackendRegistry,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            frontends: FrontendRegistry::new(),
            backends: BackendRegistry::new(),
        }
    }

    pub fn register_frontend(&mut self, frontend: Box<dyn Frontend>) {
        self.frontends.register(frontend);
    }

    pub fn register_backend(&mut self, backend: Box<dyn Backend>) {
        self.backends.register(backend);
    }

    pub fn frontends(&self) -> &FrontendRegistry {
        &self.frontends
    }

    pub fn backends(&self) -> &BackendRegistry {
        &self.backends
    }

    /// Compile source code using the appropriate frontend
    pub fn compile_source(
        &self,
        source: &str,
        filename: &str,
        frontend_name: Option<&str>,
        config: &FrontendConfig,
        reporter: &DiagnosticReporter,
        file_id: usize,
    ) -> CompileResult<IrModule> {
        let frontend = if let Some(name) = frontend_name {
            self.frontends.find_by_name(name)
        } else {
            // Auto-detect from file extension
            let ext = Path::new(filename)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e))
                .unwrap_or_default();
            self.frontends.find_by_extension(&ext)
        };

        let frontend = frontend.ok_or_else(|| {
            crate::common::CompileError::codegen(format!(
                "no frontend found for file: {}",
                filename
            ))
        })?;

        let ctx = CompileContext::new(filename.to_string(), file_id, reporter);
        frontend.compile(source, &ctx, config)
    }

    /// Generate output using the specified backend
    pub fn generate_output(
        &self,
        module: &IrModule,
        backend_name: &str,
        config: &BackendConfig,
    ) -> CompileResult<BackendOutput> {
        let backend = self.backends.find_by_name(backend_name).ok_or_else(|| {
            crate::common::CompileError::backend(format!(
                "backend not found: {}",
                backend_name
            ))
        })?;

        backend.generate(module, config)
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
