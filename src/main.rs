use clap::Parser;
use eyre::Result;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ParserType {
    Nm,
    Native,
    Goblin,
}

mod parsers;
mod report;

use parsers::definitions::{ElfParser, Symbol};
use parsers::goblin::GoblinParser;
use parsers::native::NativeParser;
use parsers::nm::NmParser;
use report::SymbolDiff;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The base ELF file (before changes)
    #[arg(value_name = "FROM_FILE")]
    from: PathBuf,

    /// The new ELF file (after changes)
    #[arg(value_name = "TO_FILE")]
    to: PathBuf,

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

    /// Parser type to use
    #[arg(long, value_enum, default_value_t = ParserType::Native)]
    parser: ParserType,
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

    let parser: Box<dyn ElfParser> = match args.parser {
        ParserType::Nm => Box::new(NmParser::default()),
        ParserType::Native => Box::new(NativeParser),
        ParserType::Goblin => Box::new(GoblinParser),
    };

    tracing::info!("Using {:?} parser", args.parser);

    let mut symbols1 =
        parser
            .get_symbols(args.from.to_str().ok_or_else(|| {
                eyre::eyre!("FROM path is not valid UTF-8: {}", args.from.display())
            })?)
            .map_err(|e| eyre::eyre!(e))?;
    tracing::debug!("Symbols from FROM file: {:?}", symbols1.len());
    if !args.no_demangle {
        for s in &mut symbols1 {
            s.demangle();
        }
    }

    let mut symbols2 = parser
        .get_symbols(
            args.to
                .to_str()
                .ok_or_else(|| eyre::eyre!("TO path is not valid UTF-8: {}", args.to.display()))?,
        )
        .map_err(|e| eyre::eyre!(e))?;
    tracing::debug!("Symbols from TO file: {:?}", symbols2.len());
    if !args.no_demangle {
        for s in &mut symbols2 {
            s.demangle();
        }
    }

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
    tracing::info!("Generated {} diffs", diffs.len());

    // Sort diffs by symbol name for consistent output
    diffs.sort_by(|a, b| a.name.cmp(&b.name));

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
