//! SMD Compiler - C and Rust compiler for Sega Megadrive/Genesis
//!
//! Usage: smdc [OPTIONS] <input> -o <output>

use clap::{Parser as ClapParser, ValueEnum};
use smd_compiler::common::DiagnosticReporter;
use smd_compiler::frontend::{CFrontend, RustFrontend, Frontend, FrontendConfig, CompileContext};
use smd_compiler::backend::{M68kBackend, RomBackend, Backend, BackendConfig, OutputFormat, RomConfig};
use std::fs;
use std::path::PathBuf;
use std::process;

/// Source language
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Default)]
enum Language {
    /// C language
    C,
    /// Rust language
    Rust,
    /// Auto-detect from file extension
    #[default]
    Auto,
}

/// Output type
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Default)]
enum OutputType {
    /// Assembly text (.s)
    #[default]
    Asm,
    /// Raw binary ROM (.bin)
    Rom,
}

#[derive(ClapParser, Debug)]
#[command(name = "smdc")]
#[command(author = "SMD-SDK Team")]
#[command(version = "0.2.0")]
#[command(about = "C and Rust compiler for Sega Megadrive/Genesis (M68000)", long_about = None)]
struct Args {
    /// Input source file (.c or .rs)
    #[arg(required = true)]
    input: PathBuf,

    /// Output file (assembly or ROM)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Source language (c, rust, or auto)
    #[arg(short, long, value_enum, default_value = "auto")]
    lang: Language,

    /// Output type (asm or rom)
    #[arg(short = 't', long, value_enum, default_value = "asm")]
    output_type: OutputType,

    /// Optimization level (0-3)
    #[arg(short = 'O', long, default_value = "0")]
    optimize: u8,

    /// Generate debug information
    #[arg(short = 'g', long)]
    debug: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Dump IR (for debugging)
    #[arg(long)]
    dump_ir: bool,

    /// Dump AST (for debugging)
    #[arg(long)]
    dump_ast: bool,

    /// Dump tokens (for debugging)
    #[arg(long)]
    dump_tokens: bool,

    /// Dump MIR (for Rust, for debugging)
    #[arg(long)]
    dump_mir: bool,

    // ROM-specific options
    /// Domestic (Japanese) game name for ROM
    #[arg(long, default_value = "SMD GAME")]
    domestic_name: String,

    /// Overseas game name for ROM
    #[arg(long, default_value = "SMD GAME")]
    overseas_name: String,
}

fn main() {
    let args = Args::parse();

    if let Err(e) = run(&args) {
        eprintln!("error: {}", e);
        process::exit(1);
    }
}

fn detect_language(path: &PathBuf, explicit: Language) -> Language {
    match explicit {
        Language::Auto => {
            match path.extension().and_then(|e| e.to_str()) {
                Some("rs") => Language::Rust,
                Some("c") | Some("h") => Language::C,
                _ => {
                    eprintln!("warning: cannot detect language, defaulting to C");
                    Language::C
                }
            }
        }
        other => other,
    }
}

fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    // Read input file
    let source = fs::read_to_string(&args.input)?;
    let filename = args.input.display().to_string();

    // Set up diagnostic reporter
    let mut reporter = DiagnosticReporter::new();
    let file_id = reporter.add_file(&filename, &source);

    // Detect language
    let language = detect_language(&args.input, args.lang);

    // Determine output extension based on output type
    let default_ext = match args.output_type {
        OutputType::Asm => "s",
        OutputType::Rom => "bin",
    };

    // Determine output path
    let output_path = args.output.clone().unwrap_or_else(|| {
        let mut path = args.input.clone();
        path.set_extension(default_ext);
        path
    });

    if args.verbose {
        let lang_str = match language {
            Language::C => "C",
            Language::Rust => "Rust",
            Language::Auto => "auto",
        };
        let output_str = match args.output_type {
            OutputType::Asm => "assembly",
            OutputType::Rom => "ROM",
        };
        eprintln!("Compiling {} ({}) -> {} ({})",
            args.input.display(), lang_str, output_path.display(), output_str);
    }

    // Select frontend
    let frontend: Box<dyn Frontend> = match language {
        Language::C => Box::new(CFrontend::new()),
        Language::Rust => Box::new(RustFrontend::new()),
        Language::Auto => unreachable!(),
    };

    // Configure frontend
    let frontend_config = FrontendConfig {
        dump_tokens: args.dump_tokens,
        dump_ast: args.dump_ast,
        dump_mir: args.dump_mir,
        verbose: args.verbose,
    };

    // Create compile context
    let ctx = CompileContext::new(filename.clone(), file_id, &reporter);

    // Compile to IR
    let ir_module = frontend.compile(&source, &ctx, &frontend_config)?;

    if args.dump_ir {
        eprintln!("=== IR ===");
        eprintln!("{}", ir_module);
        eprintln!("=== End IR ===\n");
    }

    // Select backend and generate output
    let backend_config = BackendConfig {
        output_format: match args.output_type {
            OutputType::Asm => OutputFormat::Assembly,
            OutputType::Rom => OutputFormat::Binary,
        },
        optimize_level: args.optimize,
        debug_info: args.debug,
        dump_ir: args.dump_ir,
        verbose: args.verbose,
    };

    let output = match args.output_type {
        OutputType::Asm => {
            let backend = M68kBackend::new();
            backend.generate(&ir_module, &backend_config)?
        }
        OutputType::Rom => {
            let rom_config = RomConfig {
                domestic_name: args.domestic_name.clone(),
                overseas_name: args.overseas_name.clone(),
                ..Default::default()
            };
            let backend = RomBackend::with_config(rom_config);
            backend.generate(&ir_module, &backend_config)?
        }
    };

    // Write output
    output.write_to(&output_path)?;

    if args.verbose {
        eprintln!("Successfully compiled to {}", output_path.display());
    }

    Ok(())
}
