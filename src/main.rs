use clap::Parser;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::util::SubscriberInitExt;
use eyre::Result;

mod elf_parser;
mod report;

use elf_parser::Symbol;
use report::SymbolDiff;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// First ELF file to compare
    #[arg(value_name = "FILE1")]
    file1: PathBuf,

    /// Second ELF file to compare
    #[arg(value_name = "FILE2")]
    file2: PathBuf,

    #[arg(short, long, value_enum, default_value_t = report::OutputType::Table)]
    output: report::OutputType,

    #[arg(long, action)]
    demangle: bool,

    #[arg(short, long, value_enum, default_value_t = LogLevel::Info)]
    log_level: LogLevel,
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
    let args = Args::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::from(args.log_level))
        .finish();

    subscriber.init();

    tracing::info!("Starting elf-diff with args: {:?}", args);

    let symbols1 = elf_parser::get_symbol_sizes(&args.file1, args.demangle)?;
    tracing::debug!("Symbols from file1: {:?}", symbols1.len());
    let symbols2 = elf_parser::get_symbol_sizes(&args.file2, args.demangle)?;
    tracing::debug!("Symbols from file2: {:?}", symbols2.len());

    let map1: HashMap<&str, &Symbol> = symbols1.iter().map(|s| (s.name.as_str(), s)).collect();
    let map2: HashMap<&str, &Symbol> = symbols2.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut diffs = Vec::new();

    for (name, symbol1) in &map1 {
        match map2.get(name) {
            Some(symbol2) => {
                if symbol1.size != symbol2.size {
                    tracing::trace!("Symbol CHANGED: {} ({} -> {})", name, symbol1.size, symbol2.size);
                    diffs.push(SymbolDiff {
                        name: (*name).to_string(),
                        change_type: "CHANGED".to_string(),
                        size_diff: symbol1.size as i64 - symbol2.size as i64,
                    });
                } else {
                    tracing::trace!("Symbol UNCHANGED: {}", name);
                }
            }
            None => {
                tracing::trace!("Symbol REMOVED: {} ({})", name, symbol1.size);
                diffs.push(SymbolDiff {
                    name: (*name).to_string(),
                    change_type: "REMOVED".to_string(),
                    size_diff: symbol1.size as i64,
                });
            }
        }
    }

    for (name, symbol2) in &map2 {
        if !map1.contains_key(name) {
            tracing::trace!("Symbol ADDED: {} ({})", name, symbol2.size);
            diffs.push(SymbolDiff {
                name: (*name).to_string(),
                change_type: "ADDED".to_string(),
                size_diff: -(symbol2.size as i64),
            });
        }
    }

    tracing::info!("Generated {} diffs", diffs.len());

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    report::generate_report(&mut handle, diffs, args.output)?;

    tracing::info!("Report generated successfully");

    Ok(())
}

// Add this to allow clap to parse OutputType
impl std::str::FromStr for report::OutputType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(report::OutputType::Table),
            "csv" => Ok(report::OutputType::Csv),
            _ => Err(format!("Invalid output type: {}", s)),
        }
    }
}

impl clap::ValueEnum for report::OutputType {
    fn value_variants<'a>() -> &'a [Self] {
        &[report::OutputType::Table, report::OutputType::Csv]
    }

    fn to_possible_value<'a>(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            report::OutputType::Table => clap::builder::PossibleValue::new("table"),
            report::OutputType::Csv => clap::builder::PossibleValue::new("csv"),
        })
    }
}
