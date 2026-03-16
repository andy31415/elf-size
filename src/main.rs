use clap::{Parser, Subcommand};
use eyre::Result;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::util::SubscriberInitExt;

mod parsers;
mod report;

use crate::parsers::{create_parser, definitions::{Symbol, SymbolKind}};
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
    Compare {
        /// The base ELF file
        #[arg(short, long)]
        from: PathBuf,

        /// The target ELF file
        #[arg(short, long)]
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
    Show {
        /// The ELF file to inspect
        #[arg(index = 1)]
        elf_file: PathBuf,

        /// Symbols to show disassembly for
        #[arg(index = 2, num_args = 1..)]
        symbols: Vec<String>,
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
        Commands::Compare { from, to, output_type, hide_read_only, demangle, parser, max_symbol_width } => {
            run_compare(from, to, output_type, hide_read_only, demangle, &parser, max_symbol_width)
        }
        Commands::Show { elf_file, symbols } => {
            run_show(elf_file, symbols)
        }
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

    let from_path = from.to_str().ok_or_else(|| eyre::eyre!("FROM path is not valid UTF-8: {}", from.display()))?;
    let mut from_symbols = from_parser.get_symbols(from_path).map_err(|e| eyre::eyre!(e))?;
    if demangle {
        from_symbols.iter_mut().for_each(|s| s.demangle());
    }
    tracing::debug!("Symbols from FROM file: {:?}", from_symbols.len());

    let to_path = to.to_str().ok_or_else(|| eyre::eyre!("TO path is not valid UTF-8: {}", to.display()))?;
    let mut to_symbols = to_parser.get_symbols(to_path).map_err(|e| eyre::eyre!(e))?;
    if demangle {
        to_symbols.iter_mut().for_each(|s| s.demangle());
    }
    tracing::debug!("Symbols from TO file: {:?}", to_symbols.len());

    let diffs = run_diff(from_symbols, to_symbols, hide_read_only);
    tracing::info!("Generated {} diffs", diffs.len());

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    generate_report(&mut handle, &ReportData {
        diffs: &diffs,
        output_type,
        include_total: true, // This was previously conditional, fixed to true
    }, max_symbol_width)?;

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
        if hide_read_only && (symbol1.kind == SymbolKind::Code || symbol1.kind == SymbolKind::RoData) {
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
        if hide_read_only && (symbol2.kind == SymbolKind::Code || symbol2.kind == SymbolKind::RoData) {
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

fn run_show(elf_file: PathBuf, symbols: Vec<String>) -> Result<()> {
    println!("Showing disassembly for {:?} in file {:?}", symbols, elf_file);
    // TODO: Implement disassembly logic
    Ok(())
}
