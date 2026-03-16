use clap::{Parser, Subcommand};
use eyre::Result;
use object::{Architecture, Object};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::FmtSubscriber;

mod parsers;
mod report;

use crate::parsers::{
    create_parser,
    definitions::{Symbol, SymbolKind},
};
use regex::Regex;
use report::{generate_report, OutputType, ReportData, SymbolDiff};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, value_enum, default_value_t = LogLevel::Info)]
    log_level: LogLevel,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Compare symbols between two ELF files
    #[command(alias = "c")]
    Compare {
        /// The base ELF file
        #[arg(index = 1)]
        from: PathBuf,

        /// The target ELF file
        #[arg(index = 2)]
        to: PathBuf,

        /// Output format
        #[arg(short, long, value_enum, default_value_t = OutputType::Table)]
        output_type: OutputType,

        /// Hide read-only sections (e.g., .text, .rodata)
        #[arg(long, default_value_t = false)]
        hide_read_only: bool,

        /// Demangle symbol names
        #[arg(short, long, default_value_t = true)]
        demangle: bool,

        /// Parser to use
        #[arg(short, long, default_value = "native")]
        parser: String,

        /// Maximum width for the symbol column in table output (0 for no limit)
        #[arg(long, default_value_t = 100)]
        max_symbol_width: usize,
    },
    /// Show disassembly for symbols in an ELF file
    #[command(alias = "s")]
    Show {
        /// The ELF file to inspect
        #[arg(index = 1)]
        elf_file: PathBuf,

        /// Symbols to show disassembly for
        #[arg(index = 2, num_args = 1..)]
        symbols: Vec<String>,

        /// Demangle symbol names
        #[arg(short, long, default_value_t = true)]
        demangle: bool,

        /// Parser to use
        #[arg(short, long, default_value = "native")]
        parser: String,

        /// Path to the objdump binary to use
        #[arg(long, default_value = "objdump")]
        objdump: String,

        /// Interleave source code with disassembly (requires debug info)
        #[arg(long, default_value_t = false)]
        source: bool,
    },
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Level::ERROR,
            LogLevel::Warn => Level::WARN,
            LogLevel::Info => Level::INFO,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Trace => Level::TRACE,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::from(cli.log_level))
        .finish();

    subscriber.init();

    tracing::info!("Starting elf-diff with command: {:?}", cli.command);

    match cli.command {
        Commands::Compare {
            from,
            to,
            output_type,
            hide_read_only,
            demangle,
            parser,
            max_symbol_width,
        } => run_compare(
            from,
            to,
            output_type,
            hide_read_only,
            demangle,
            &parser,
            max_symbol_width,
        ),
        Commands::Show {
            elf_file,
            symbols,
            demangle,
            parser,
            objdump,
            source,
        } => run_show(elf_file, symbols, demangle, &parser, &objdump, source),
    }
}

fn run_compare(
    from: PathBuf,
    to: PathBuf,
    output_type: OutputType,
    hide_read_only: bool,
    demangle: bool,
    parser_name: &str,
    max_symbol_width: usize,
) -> Result<()> {
    let from_parser = create_parser(parser_name, &from)?;
    let to_parser = create_parser(parser_name, &to)?;

    tracing::info!("Using {} parser", parser_name);

    let from_path = from
        .to_str()
        .ok_or_else(|| eyre::eyre!("FROM path is not valid UTF-8: {}", from.display()))?;
    let mut from_symbols = from_parser
        .get_symbols(from_path)
        .map_err(|e| eyre::eyre!(e))?;
    if demangle {
        from_symbols.iter_mut().for_each(|s| s.demangle());
    }
    tracing::debug!("Symbols from FROM file: {:?}", from_symbols.len());

    let to_path = to
        .to_str()
        .ok_or_else(|| eyre::eyre!("TO path is not valid UTF-8: {}", to.display()))?;
    let mut to_symbols = to_parser.get_symbols(to_path).map_err(|e| eyre::eyre!(e))?;
    if demangle {
        to_symbols.iter_mut().for_each(|s| s.demangle());
    }
    tracing::debug!("Symbols from TO file: {:?}", to_symbols.len());

    let diffs = run_diff(from_symbols, to_symbols, hide_read_only);
    tracing::info!("Generated {} diffs", diffs.len());

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    generate_report(
        &mut handle,
        &ReportData {
            diffs: &diffs,
            output_type,
            include_total: true, // This was previously conditional, fixed to true
        },
        max_symbol_width,
    )?;

    Ok(())
}

fn run_diff(
    from_symbols: Vec<Symbol>,
    to_symbols: Vec<Symbol>,
    hide_read_only: bool,
) -> Vec<SymbolDiff> {
    let map1: HashMap<&str, &Symbol> = from_symbols.iter().map(|s| (s.name.as_str(), s)).collect();
    let map2: HashMap<&str, &Symbol> = to_symbols.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut diffs = Vec::new();

    for (name, symbol1) in &map1 {
        if hide_read_only
            && (symbol1.kind == SymbolKind::Code || symbol1.kind == SymbolKind::RoData)
        {
            continue;
        }

        match map2.get(name) {
            Some(symbol2) => {
                if symbol1.size != symbol2.size {
                    tracing::trace!(
                        "Symbol CHANGED: {} ({} -> {})",
                        name,
                        symbol1.size,
                        symbol2.size
                    );
                    diffs.push(SymbolDiff {
                        name: (*name).to_string(),
                        change_type: report::ChangeType::Changed,
                        size_diff: symbol2.size as i64 - symbol1.size as i64,
                        kind: symbol1.kind.clone(),
                    });
                } else {
                    tracing::trace!("Symbol UNCHANGED: {}", name);
                }
            }
            None => {
                tracing::trace!("Symbol REMOVED: {} ({})", name, symbol1.size);
                diffs.push(SymbolDiff {
                    name: (*name).to_string(),
                    change_type: report::ChangeType::Removed,
                    size_diff: -(symbol1.size as i64),
                    kind: symbol1.kind.clone(),
                });
            }
        }
    }

    for (name, symbol2) in &map2 {
        if hide_read_only
            && (symbol2.kind == SymbolKind::Code || symbol2.kind == SymbolKind::RoData)
        {
            continue;
        }
        if !map1.contains_key(name) {
            tracing::trace!("Symbol ADDED: {} ({})", name, symbol2.size);
            diffs.push(SymbolDiff {
                name: (*name).to_string(),
                change_type: report::ChangeType::Added,
                size_diff: symbol2.size as i64,
                kind: symbol2.kind.clone(),
            });
        }
    }

    // Sort diffs by symbol name for consistent output
    diffs.sort_by(|a, b| a.name.cmp(&b.name));
    diffs
}

fn get_arch_default_objdump(elf_file: &PathBuf) -> Result<String> {
    let bin_data = fs::read(elf_file)?;
    let obj_file = object::File::parse(&*bin_data)
        .map_err(|e| eyre::eyre!("Failed to parse ELF file {}: {}", elf_file.display(), e))?;
    let arch = obj_file.architecture();
    let default_objdump = match arch {
        Architecture::Arm | Architecture::Aarch64 => "arm-none-eabi-objdump".to_string(),
        Architecture::X86_64 | Architecture::I386 => "objdump".to_string(),
        // Add more architectures as needed
        _ => {
            tracing::warn!(
                "Unsupported architecture {:?} for objdump auto-detection, defaulting to 'objdump'",
                arch
            );
            "objdump".to_string()
        }
    };
    Ok(default_objdump)
}

fn run_show(
    elf_file: PathBuf,
    symbols: Vec<String>,
    demangle: bool,
    parser_name: &str,
    objdump_path: &str,
    show_source: bool,
) -> Result<()> {
    let chosen_objdump = if objdump_path == "objdump" {
        match get_arch_default_objdump(&elf_file) {
            Ok(default_cmd) => default_cmd,
            Err(e) => {
                tracing::warn!(
                    "Failed to auto-detect objdump, falling back to 'objdump': {}",
                    e
                );
                "objdump".to_string()
            }
        }
    } else {
        objdump_path.to_string()
    };

    let parser = create_parser(parser_name, &elf_file)?;
    let elf_path_str = elf_file
        .to_str()
        .ok_or_else(|| eyre::eyre!("ELF file path is not valid UTF-8: {}", elf_file.display()))?;
    let mut elf_symbols = parser
        .get_symbols(elf_path_str)
        .map_err(|e| eyre::eyre!(e))?;
    if demangle {
        elf_symbols.iter_mut().for_each(|s| s.demangle());
    }
    tracing::debug!("Loaded {} symbols from {}", elf_symbols.len(), elf_path_str);

    for pattern in symbols {
        println!("\n--- Matching symbols for pattern: {} ---", pattern);
        let regex = match Regex::new(&pattern) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Invalid regex '{}': {}", pattern, e);
                eyre::bail!("Invalid regex '{}': {}", pattern, e)
            }
        };

        let matches: Vec<&Symbol> = elf_symbols
            .iter()
            .filter(|s| regex.is_match(&s.name))
            .collect();

        match matches.len() {
            0 => println!("No symbols found matching pattern '{}'.", pattern),
            1 => {
                let symbol = matches[0];
                println!("Found unique match: {}", symbol.name);
                if symbol.kind != SymbolKind::Code {
                    tracing::warn!(
                        "Symbol '{}' is not a code symbol (kind: {:?}), skipping disassembly.",
                        symbol.name,
                        symbol.kind
                    );
                    continue;
                }
                if symbol.size == 0 {
                    tracing::warn!(
                        "Symbol '{}' has zero size, skipping disassembly.",
                        symbol.name
                    );
                    continue;
                }
                let start_addr = symbol.address;
                let end_addr = symbol.address + symbol.size as u64;
                println!("  Address: 0x{:x}, Size: {}", start_addr, symbol.size);

                let mut command = std::process::Command::new(&chosen_objdump);
                if show_source {
                    command.arg("-S");
                } else {
                    command.arg("-d");
                }
                command.arg(format!("--start-address={}", start_addr));
                command.arg(format!("--stop-address={}", end_addr));
                command.arg(&elf_file);

                tracing::info!("Running command: {:?}", command);

                let objdump_child = command.stdout(std::process::Stdio::piped()).spawn()?;
                let objdump_stdout = objdump_child
                    .stdout
                    .ok_or_else(|| eyre::eyre!("Failed to get objdump stdout"))?;

                let cxxfilt_child = std::process::Command::new("c++filt")
                    .stdin(objdump_stdout)
                    .stdout(std::process::Stdio::piped())
                    .spawn()?;

                let output = cxxfilt_child.wait_with_output()?;

                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.trim().is_empty() {
                        eyre::bail!("No disassembly output for this range");
                    }
                    println!("{}", stdout);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eyre::bail!("c++filt failed:\n{}", stderr)
                }
            }
            _ => {
                let mut match_list = String::new();
                for symbol in matches {
                    match_list.push_str(&format!(
                        "  - {} (Address: 0x{:x}, Size: {})
",
                        symbol.name, symbol.address, symbol.size
                    ));
                }
                eyre::bail!(
                    "Found multiple matches for pattern '{}':\n{}Please refine your pattern to match a single symbol.",
                    pattern,
                    match_list
                )
            }
        }
    }
    Ok(())
}
