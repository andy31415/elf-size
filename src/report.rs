use prettytable::{Cell, Row, Table, row};
use std::io::Write;
use eyre::{Context, Result};

#[derive(Clone, Debug)]
pub enum OutputType {
    Table,
    Csv,
}

pub fn generate_report<W: Write>(
    writer: &mut W,
    mut diffs: Vec<SymbolDiff>,
    output_type: OutputType,
) -> Result<()> {
    diffs.sort_by_key(|d| d.size_diff);

    tracing::debug!("Generating report with type: {:?}", output_type);

    match output_type {
        OutputType::Table => {
            let mut table = Table::new();
            table.add_row(row!["Type", "Size Diff", "Symbol"]);

            for diff in diffs {
                table.add_row(Row::new(vec![
                    Cell::new(&diff.change_type),
                    Cell::new(&diff.size_diff.to_string()),
                    Cell::new(&diff.name),
                ]));
            }
            table.print(writer).context("Failed to print table")?;
        }
        OutputType::Csv => {
            let mut wtr = csv::Writer::from_writer(writer);
            wtr.write_record(["Type", "Size Diff", "Symbol"]).context("Failed to write CSV header")?;
            for diff in diffs {
                wtr.write_record(&[
                    diff.change_type,
                    diff.size_diff.to_string(),
                    diff.name,
                ]).context("Failed to write CSV record")?;
            }
            wtr.flush().context("Failed to flush CSV writer")?;
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct SymbolDiff {
    pub name: String,
    pub change_type: String,
    pub size_diff: i64,
}
