use crate::parsers::definitions::SymbolKind;

fn kind_sort_order(kind: &SymbolKind) -> usize {
    match kind {
        SymbolKind::Code => 0,
        SymbolKind::Data => 1,
        SymbolKind::RoData => 2,
        SymbolKind::Weak => 3,
        SymbolKind::Bss => 4,
        _ => 5, // All others
    }
}
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

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
pub enum OutputType {
    Table,
    Csv,
}

pub struct ReportData<'a> {
    pub diffs: &'a [SymbolDiff],
    pub output_type: OutputType,
    pub include_total: bool,
}

pub fn generate_report<W: Write>(writer: &mut W, data: &ReportData, max_symbol_width: usize) -> Result<()> {
    let mut sorted_diffs = data.diffs.to_vec();
    sorted_diffs.sort_by(|a, b| {
        kind_sort_order(&a.kind)
            .cmp(&kind_sort_order(&b.kind))
            .then_with(|| b.size_diff.cmp(&a.size_diff)) // Secondary sort by size descending
    });

    let flash_total: i64 = sorted_diffs
        .iter()
        .filter(|d| matches!(d.kind, SymbolKind::Code | SymbolKind::RoData))
        .map(|d| d.size_diff)
        .sum();

    let ram_total: i64 = sorted_diffs
        .iter()
        .filter(|d| d.kind == SymbolKind::Bss)
        .map(|d| d.size_diff)
        .sum();

    tracing::debug!(
        "Generating report with type: {:?}, include_total: {}",
        data.output_type,
        data.include_total
    );

    match data.output_type {
        OutputType::Table => {
            let mut table = Table::new();
            let terminal_width = terminal_size().map(|(TermWidth(w), _)| w).unwrap_or(120);
            const TYPE_WIDTH: u16 = 10;
            const KIND_WIDTH: u16 = 12;
            const DELTA_WIDTH: u16 = 8;
            const SEPARATORS: u16 = 5; // Approximately for the 4 columns

            let dynamic_symbol_width =
                terminal_width.saturating_sub(TYPE_WIDTH + KIND_WIDTH + DELTA_WIDTH + SEPARATORS);
            let symbol_width = if max_symbol_width == 0 {
                dynamic_symbol_width
            } else {
                std::cmp::min(dynamic_symbol_width, max_symbol_width as u16)
            };

            table
                .load_preset(UTF8_FULL_CONDENSED)
                .set_header(vec![
                    Cell::new("Type").add_attribute(Attribute::Bold),
                    Cell::new("Kind").add_attribute(Attribute::Bold),
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
                // Kind
                column.set_constraint(ColumnConstraint::UpperBoundary(Width::Fixed(KIND_WIDTH)));
            }
            if let Some(column) = table.column_mut(2) {
                // Delta
                column.set_constraint(ColumnConstraint::UpperBoundary(Width::Fixed(DELTA_WIDTH)));
            }
            // Note: No constraint on Symbol column, width is handled by manual truncation.

            for diff in &sorted_diffs {
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
                    Cell::new(diff.kind.to_string()),
                    Cell::new(diff.size_diff.to_string()).set_alignment(CellAlignment::Right),
                    Cell::new(&symbol_name),
                ]);
            }

            if data.include_total {
                table.add_row(vec![
                    Cell::new("TOTAL").add_attribute(Attribute::Bold),
                    Cell::new("FLASH").add_attribute(Attribute::Bold),
                    Cell::new(flash_total.to_string())
                        .set_alignment(CellAlignment::Right)
                        .add_attribute(Attribute::Bold),
                    Cell::new(""),
                ]);
                table.add_row(vec![
                    Cell::new("TOTAL").add_attribute(Attribute::Bold),
                    Cell::new("RAM").add_attribute(Attribute::Bold),
                    Cell::new(ram_total.to_string())
                        .set_alignment(CellAlignment::Right)
                        .add_attribute(Attribute::Bold),
                    Cell::new(""),
                ]);
            }
            writeln!(writer, "{}", table).context("Failed to print table")?;
        }
        OutputType::Csv => {
            let mut wtr = csv::Writer::from_writer(writer);
            wtr.write_record(["Type", "Kind", "Size Diff", "Symbol"])
                .context("Failed to write CSV header")?;
            for diff in &sorted_diffs {
                wtr.write_record(&[
                    diff.change_type.to_string(),
                    diff.kind.to_string(),
                    diff.size_diff.to_string(),
                    diff.name.clone(),
                ])
                .context("Failed to write CSV record")?;
            }
            if data.include_total {
                wtr.write_record(["TOTAL", "FLASH", &flash_total.to_string(), ""])
                    .context("Failed to write CSV FLASH total")?;
                wtr.write_record(["TOTAL", "RAM", &ram_total.to_string(), ""])
                    .context("Failed to write CSV RAM total")?;
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
    pub kind: SymbolKind,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::definitions::SymbolKind;

    #[test]
    fn test_kind_sort_order() {
        assert_eq!(kind_sort_order(&SymbolKind::Code), 0);
        assert_eq!(kind_sort_order(&SymbolKind::Data), 1);
        assert_eq!(kind_sort_order(&SymbolKind::RoData), 2);
        assert_eq!(kind_sort_order(&SymbolKind::Weak), 3);
        assert_eq!(kind_sort_order(&SymbolKind::Bss), 4);
        assert_eq!(kind_sort_order(&SymbolKind::Other), 5);
        assert_eq!(kind_sort_order(&SymbolKind::Undefined), 5);
    }

    use std::io::Cursor;

    fn sample_diffs() -> Vec<SymbolDiff> {
        vec![
            SymbolDiff {
                name: "very_long_symbol_name_that_will_likely_be_truncated".to_string(),
                change_type: ChangeType::Added,
                size_diff: 100,
                kind: SymbolKind::Code,
            },
            SymbolDiff {
                name: "another_symbol".to_string(),
                change_type: ChangeType::Removed,
                size_diff: -50,
                kind: SymbolKind::Data,
            },
            SymbolDiff {
                name: "bss_symbol".to_string(),
                change_type: ChangeType::Changed,
                size_diff: 20,
                kind: SymbolKind::Bss,
            },
            SymbolDiff {
                name: "ro_symbol".to_string(),
                change_type: ChangeType::Added,
                size_diff: 10,
                kind: SymbolKind::RoData,
            },
        ]
    }

    #[test]
    fn test_generate_report_csv() {
        let diffs = sample_diffs();
        let data = ReportData {
            diffs: &diffs,
            output_type: OutputType::Csv,
            include_total: false,
        };
        let mut buffer = Cursor::new(Vec::new());
        generate_report(&mut buffer, &data, 100).unwrap();
        let output = String::from_utf8(buffer.into_inner()).unwrap();

        let expected = "Type,Kind,Size Diff,Symbol\nADDED,Code,100,very_long_symbol_name_that_will_likely_be_truncated\nREMOVED,Data,-50,another_symbol\nADDED,ROData,10,ro_symbol\nCHANGED,BSS,20,bss_symbol\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_generate_report_csv_with_total() {
        let diffs = sample_diffs();
        let data = ReportData {
            diffs: &diffs,
            output_type: OutputType::Csv,
            include_total: true,
        };
        let mut buffer = Cursor::new(Vec::new());
        generate_report(&mut buffer, &data, 100).unwrap();
        let output = String::from_utf8(buffer.into_inner()).unwrap();

        let expected = "Type,Kind,Size Diff,Symbol\nADDED,Code,100,very_long_symbol_name_that_will_likely_be_truncated\nREMOVED,Data,-50,another_symbol\nADDED,ROData,10,ro_symbol\nCHANGED,BSS,20,bss_symbol\nTOTAL,FLASH,110,\nTOTAL,RAM,20,\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_generate_report_table() {
        let diffs = sample_diffs();
        let data = ReportData {
            diffs: &diffs,
            output_type: OutputType::Table,
            include_total: false,
        };
        let mut buffer = Cursor::new(Vec::new());
        generate_report(&mut buffer, &data, 100).unwrap();
        let output = String::from_utf8(buffer.into_inner()).unwrap();

        // Looser checks for table due to term size dependence and formatting
        assert!(output.contains("very_long_symbol_name_that_will")); // Check for truncation indicator
        assert!(output.contains("another_symbol"));
        assert!(output.contains("Code"));
        assert!(output.contains("Data"));
        assert!(!output.contains("TOTAL"));
    }

    #[test]
    fn test_generate_report_table_with_total() {
        let diffs = sample_diffs();
        let data = ReportData {
            diffs: &diffs,
            output_type: OutputType::Table,
            include_total: true,
        };
        let mut buffer = Cursor::new(Vec::new());
        generate_report(&mut buffer, &data, 100).unwrap();
        let output = String::from_utf8(buffer.into_inner()).unwrap();

        assert!(output.contains("very_long_symbol_name_that_will"));
        assert!(output.contains("TOTAL"));
        assert!(output.contains("FLASH"));
        assert!(output.contains("RAM"));
        assert!(output.contains("110")); // FLASH total
        assert!(output.contains("20")); // RAM total
    }

    #[test]
    fn test_generate_report_empty() {
        let diffs: Vec<SymbolDiff> = Vec::new();
        let data = ReportData {
            diffs: &diffs,
            output_type: OutputType::Csv,
            include_total: false,
        };
        let mut buffer = Cursor::new(Vec::new());
        generate_report(&mut buffer, &data, 100).unwrap();
        let output = String::from_utf8(buffer.into_inner()).unwrap();
        assert_eq!(output, "Type,Kind,Size Diff,Symbol\n");

        let data_table = ReportData {
            diffs: &diffs,
            output_type: OutputType::Table,
            include_total: true, // Check totals with empty too
        };
        let mut buffer_table = Cursor::new(Vec::new());
        generate_report(&mut buffer_table, &data_table, 100).unwrap();
        let output_table = String::from_utf8(buffer_table.into_inner()).unwrap();
        assert!(output_table.contains("Type"));
        assert!(output_table.contains("TOTAL"));
        assert!(output_table.contains("0")); // Totals should be 0
    }
}
