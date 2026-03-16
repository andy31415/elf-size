use clap::Parser;
use eyre::Result;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::util::SubscriberInitExt;

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

    /// Output format
    #[arg(short, long, value_parser = ["table", "csv"], default_value = "table")]
    output: String,

    /// Whether to include a total row in the output
    #[arg(long, value_parser = ["yes", "no"], default_value = "yes")]
    include_total: String,

    /// Disable C++ symbol demangling
    #[arg(long, action)]
    no_demangle: bool,

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

    let symbols1 = elf_parser::get_symbol_sizes(&args.file1, !args.no_demangle)?;
    tracing::debug!("Symbols from file1: {:?}", symbols1.len());
    let symbols2 = elf_parser::get_symbol_sizes(&args.file2, !args.no_demangle)?;
    tracing::debug!("Symbols from file2: {:?}", symbols2.len());

    let map1: HashMap<&str, &Symbol> = symbols1.iter().map(|s| (s.name.as_str(), s)).collect();
    let map2: HashMap<&str, &Symbol> = symbols2.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut diffs = Vec::new();

    for (name, symbol1) in &map1 {
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
                    change_type: report::ChangeType::Removed,
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
                change_type: report::ChangeType::Added,
                size_diff: -(symbol2.size as i64),
            });
        }
    }
    tracing::info!("Generated {} diffs", diffs.len());

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let output_type = match args.output.as_str() {
        "table" => report::OutputType::Table,
        "csv" => report::OutputType::Csv,
        _ => unreachable!(), // Clap should prevent this
    };

    let include_total = args.include_total == "yes";

    let report_data = report::ReportData {
        diffs: &diffs,
        output_type,
        include_total,
    };

    report::generate_report(&mut handle, &report_data)
}
