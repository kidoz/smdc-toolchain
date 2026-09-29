//! SMD Compiler - C and Rust compiler for Sega Megadrive/Genesis
//!
//! Usage: smdc [OPTIONS] <input> -o <output>

use clap::{Parser as ClapParser, ValueEnum};
use smd_compiler::backend::m68k::{decompile_rom, disassemble_listing, parse_sym_file};
use smd_compiler::backend::rom::verify_checksum;
use smd_compiler::backend::{BackendConfig, M68kBackend, OutputFormat, RomBackend, RomConfig};
use smd_compiler::common::DiagnosticReporter;
use smd_compiler::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig, RustFrontend};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
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
    /// Input source file (.c or .rs), or a ROM binary with --disasm
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

    /// Optimization level (0-3).
    ///
    /// NOTE: accepted but currently inert — no optimization passes consume it
    /// yet. The codegen always emits straightforward stack-based code.
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

    /// Include paths for #include directives
    #[arg(short = 'I', long = "include", action = clap::ArgAction::Append)]
    include_paths: Vec<PathBuf>,

    /// Disassemble a ROM binary (.bin) instead of compiling.
    ///
    /// Prints header/vector info and an annotated listing. Symbols are
    /// loaded from --sym or from <input>.sym when present. Writes to
    /// --output if given, otherwise to stdout.
    #[arg(long)]
    disasm: bool,

    /// Decompile a ROM binary (.bin) to C-like pseudo-source instead of
    /// compiling. Functions, parameters and locals are recovered from the
    /// codegen's frame layout; if/else and loop structure is reconstructed
    /// where recognizable. Writes to --output if given, otherwise to stdout.
    #[arg(long)]
    decompile: bool,

    /// Symbol map file (.sym) to annotate the disassembly/decompilation with
    #[arg(long)]
    sym: Option<PathBuf>,

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
        eprintln!("error: {e}");
        process::exit(1);
    }
}

fn detect_language(path: &Path, explicit: Language) -> Language {
    match explicit {
        Language::Auto => match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => Language::Rust,
            Some("c" | "h") => Language::C,
            _ => {
                eprintln!("warning: cannot detect language, defaulting to C");
                Language::C
            }
        },
        other => other,
    }
}

fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    if args.disasm {
        return run_disasm(args);
    }
    if args.decompile {
        return run_decompile(args);
    }

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
        eprintln!(
            "Compiling {} ({}) -> {} ({})",
            args.input.display(),
            lang_str,
            output_path.display(),
            output_str
        );
    }

    // Select frontend
    let frontend: Box<dyn Frontend> = match language {
        Language::C => Box::new(CFrontend::new()),
        Language::Rust => Box::new(RustFrontend::new()),
        Language::Auto => unreachable!(),
    };

    // Build include paths
    let mut include_paths = args.include_paths.clone();

    // Auto-detect SDK include path relative to current directory
    let sdk_path = PathBuf::from("sdk/c/include");
    if sdk_path.exists() && !include_paths.contains(&sdk_path) {
        include_paths.push(sdk_path);
    }

    // Also check relative to input file
    if let Some(parent) = args.input.parent() {
        let relative_sdk = parent.join("../include");
        if relative_sdk.exists() && !include_paths.contains(&relative_sdk) {
            include_paths.push(relative_sdk);
        }
    }

    if args.verbose && !include_paths.is_empty() {
        eprintln!("Include paths:");
        for path in &include_paths {
            eprintln!("  {}", path.display());
        }
    }

    // Configure frontend
    let frontend_config = FrontendConfig {
        dump_tokens: args.dump_tokens,
        dump_ast: args.dump_ast,
        dump_mir: args.dump_mir,
        verbose: args.verbose,
        include_paths,
    };

    // Create compile context
    let ctx = CompileContext::new(filename.clone(), file_id, &reporter);

    // Compile to IR
    let ir_module = frontend.compile(&source, &ctx, &frontend_config)?;

    if args.dump_ir {
        eprintln!("=== IR ===");
        eprintln!("{ir_module}");
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

    let mut registry = smd_compiler::backend::BackendRegistry::new();
    registry.register(Box::new(M68kBackend::new()));

    let rom_config = RomConfig {
        domestic_name: args.domestic_name.clone(),
        overseas_name: args.overseas_name.clone(),
        ..Default::default()
    };
    registry.register(Box::new(RomBackend::with_config(rom_config)));

    let backend_name = match args.output_type {
        OutputType::Asm => "m68k",
        OutputType::Rom => "rom",
    };

    let backend = registry
        .find_by_name(backend_name)
        .ok_or_else(|| format!("Backend '{backend_name}' not found"))?;

    let output = backend.generate(&ir_module, &ctx, &backend_config)?;

    // Write output
    output.write_to(&output_path)?;

    if args.verbose {
        eprintln!("Successfully compiled to {}", output_path.display());
    }

    Ok(())
}

/// Header/vector offsets in a Megadrive ROM image
const ROM_HEADER_START: usize = 0x100;
const ROM_CODE_START: usize = 0x200;

fn run_disasm(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let rom = fs::read(&args.input)?;

    // Load symbols from --sym, or <input>.sym if it exists
    let sym_path = args
        .sym
        .clone()
        .unwrap_or_else(|| args.input.with_extension("sym"));
    let symbols = if sym_path.exists() {
        if args.verbose {
            eprintln!("Loading symbols from {}", sym_path.display());
        }
        parse_sym_file(&fs::read_to_string(&sym_path)?)
    } else {
        if args.sym.is_some() {
            return Err(format!("symbol file not found: {}", sym_path.display()).into());
        }
        HashMap::new()
    };

    let listing = rom_listing(&rom, &args.input.display().to_string(), &symbols);

    match &args.output {
        Some(path) => {
            fs::write(path, listing)?;
            if args.verbose {
                eprintln!("Disassembly written to {}", path.display());
            }
        }
        None => print!("{listing}"),
    }
    Ok(())
}

fn run_decompile(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let rom = fs::read(&args.input)?;

    // Load symbols from --sym, or <input>.sym if it exists
    let sym_path = args
        .sym
        .clone()
        .unwrap_or_else(|| args.input.with_extension("sym"));
    let symbols = if sym_path.exists() {
        if args.verbose {
            eprintln!("Loading symbols from {}", sym_path.display());
        }
        parse_sym_file(&fs::read_to_string(&sym_path)?)
    } else {
        if args.sym.is_some() {
            return Err(format!("symbol file not found: {}", sym_path.display()).into());
        }
        HashMap::new()
    };

    let source = decompile_rom(&rom, &symbols, None);

    match &args.output {
        Some(path) => {
            fs::write(path, source)?;
            if args.verbose {
                eprintln!("Decompiled source written to {}", path.display());
            }
        }
        None => print!("{source}"),
    }
    Ok(())
}

/// Build a full disassembly listing for a ROM image: header summary
/// followed by the decoded code section.
fn rom_listing(rom: &[u8], name: &str, symbols: &HashMap<String, u32>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "; Disassembly of {} ({} bytes)", name, rom.len());

    if rom.len() >= ROM_CODE_START {
        let sp = u32::from_be_bytes([rom[0], rom[1], rom[2], rom[3]]);
        let pc = u32::from_be_bytes([rom[4], rom[5], rom[6], rom[7]]);
        let system = header_string(rom, ROM_HEADER_START, 16);
        let overseas = header_string(rom, 0x150, 48);
        let checksum = u16::from_be_bytes([rom[0x18E], rom[0x18F]]);
        let checksum_status = if verify_checksum(rom) { "OK" } else { "BAD" };

        let _ = writeln!(out, "; System:   {system}");
        let _ = writeln!(out, "; Title:    {overseas}");
        let _ = writeln!(out, "; Checksum: ${checksum:04X} ({checksum_status})");
        let _ = writeln!(out, "; Entry:    ${pc:06X}  SP: ${sp:08X}");
        out.push('\n');

        // Code follows the header; trim the 0xFF fill padding at the end.
        // Any inline data section will decode as garbage instructions --
        // compile with -g and use the generated .lst for an exact listing.
        let code_start = (pc as usize).clamp(ROM_CODE_START, rom.len());
        let end = rom
            .iter()
            .rposition(|&b| b != 0xFF)
            .map_or(rom.len(), |i| i + 1)
            .max(code_start);
        out.push_str(&disassemble_listing(
            &rom[code_start..end],
            code_start as u32,
            symbols,
            None,
        ));
    } else {
        // Too small for a ROM header: treat as a raw code blob
        let _ = writeln!(out, "; No ROM header, disassembling as raw code\n");
        out.push_str(&disassemble_listing(rom, 0, symbols, None));
    }

    out
}

/// Read a fixed-size ASCII field from the ROM header.
fn header_string(rom: &[u8], offset: usize, len: usize) -> String {
    String::from_utf8_lossy(&rom[offset..offset + len])
        .trim()
        .to_string()
}
