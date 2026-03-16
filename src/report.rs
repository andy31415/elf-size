use comfy_table::{
    Attribute, Cell, CellAlignment, ColumnConstraint, Table, Width, presets::UTF8_FULL_CONDENSED,
};
use eyre::{Context, Result};
use std::io::Write;
use terminal_size::{Width as TermWidth, terminal_size};

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
    include_total: bool,
) -> Result<()> {
    diffs.sort_by_key(|d| d.size_diff);

    tracing::debug!("Generating report with type: {:?}", output_type);

    match output_type {
        OutputType::Table => {
            let mut table = Table::new();
            let terminal_width = terminal_size().map(|(TermWidth(w), _)| w).unwrap_or(120);
            const TYPE_WIDTH: u16 = 10;
            const DELTA_WIDTH: u16 = 8;
            const SEPARATORS: u16 = 4; // Approximately for the 3 columns

            let symbol_width = terminal_width.saturating_sub(TYPE_WIDTH + DELTA_WIDTH + SEPARATORS);

            table
                .load_preset(UTF8_FULL_CONDENSED)
                .set_header(vec![
                    Cell::new("Type").add_attribute(Attribute::Bold),
                    Cell::new("Delta").add_attribute(Attribute::Bold),
                    Cell::new("Symbol").add_attribute(Attribute::Bold),
                ])
                .set_width(terminal_width);

            // Add constraints to columns to control width and truncation
            if let Some(column) = table.column_mut(0) {
                // Type
                column.set_constraint(ColumnConstraint::UpperBoundary(Width::Fixed(TYPE_WIDTH)));
            }
            if let Some(column) = table.column_mut(1) {
                // Delta
                column.set_constraint(ColumnConstraint::UpperBoundary(Width::Fixed(DELTA_WIDTH)));
            }
            // Note: No constraint on Symbol column, width is handled by manual truncation.

            let total_diff: i64 = diffs.iter().map(|d| d.size_diff).sum();

            for diff in diffs {
                let symbol_name = if diff.name.len() > symbol_width as usize {
                    if symbol_width > 3 {
                        format!("{}...", &diff.name[..symbol_width as usize - 3])
                    } else {
                        diff.name.clone()
                    }
                } else {
                    diff.name.clone()
                };
                table.add_row(vec![
                    Cell::new(diff.change_type.to_string()),
                    Cell::new(diff.size_diff.to_string()).set_alignment(CellAlignment::Right),
                    Cell::new(&symbol_name),
                ]);
            }

            if include_total {
                table.add_row(vec![
                    Cell::new("TOTAL").add_attribute(Attribute::Bold),
                    Cell::new(total_diff.to_string())
                        .set_alignment(CellAlignment::Right)
                        .add_attribute(Attribute::Bold),
                    Cell::new(""),
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
