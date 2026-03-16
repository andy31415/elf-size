use comfy_table::{Attribute, Cell, CellAlignment, ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use eyre::{Context, Result};
use std::io::Write;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum ChangeType {
    Added,
    Removed,
    Changed,
}

impl std::fmt::Display for ChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeType::Added => write!(f, "ADDED"),
            ChangeType::Removed => write!(f, "REMOVED"),
            ChangeType::Changed => write!(f, "CHANGED"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum OutputType {
    Table,
    Csv,
}

pub fn generate_report<W: Write>(
    writer: &mut W,
    mut diffs: Vec<SymbolDiff>,
    output_type: OutputType,
    max_symbol_width: Option<usize>,
) -> Result<()> {
    diffs.sort_by_key(|d| d.size_diff);

    tracing::debug!("Generating report with type: {:?}", output_type);

    match output_type {
        OutputType::Table => {
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL_CONDENSED)
                .set_header(vec![
                    Cell::new("Type").add_attribute(Attribute::Bold),
                    Cell::new("Size Diff").add_attribute(Attribute::Bold),
                    Cell::new("Symbol").add_attribute(Attribute::Bold),
                ])
                .set_content_arrangement(ContentArrangement::Dynamic);

            for diff in diffs {
                let symbol_name = match max_symbol_width {
                    Some(max_width) if diff.name.len() > max_width => {
                        if max_width > 3 {
                            format!("{}...", &diff.name[..max_width - 3])
                        } else {
                            diff.name.clone()
                        }
                    }
                    _ => diff.name.clone(),
                };
                table.add_row(vec![
                    Cell::new(&diff.change_type.to_string()),
                    Cell::new(&diff.size_diff.to_string()).set_alignment(CellAlignment::Right),
                    Cell::new(&symbol_name),
                ]);
            }
            writeln!(writer, "{}", table).context("Failed to print table")?;
        }
        OutputType::Csv => {
            let mut wtr = csv::Writer::from_writer(writer);
            wtr.write_record(["Type", "Size Diff", "Symbol"])
                .context("Failed to write CSV header")?;
            for diff in diffs {
                wtr.write_record(&[
                    diff.change_type.to_string(),
                    diff.size_diff.to_string(),
                    diff.name,
                ])
                .context("Failed to write CSV record")?;
            }
            wtr.flush().context("Failed to flush CSV writer")?;
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct SymbolDiff {
    pub name: String,
    pub change_type: ChangeType,
    pub size_diff: i64,
}
