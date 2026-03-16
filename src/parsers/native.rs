use super::definitions::{ElfParser, Symbol, SymbolKind};
use eyre::Result;
use object::{Object, ObjectSection, ObjectSymbol, SectionKind};
use std::fs;
use std::path::Path;

pub struct NativeParser;

impl ElfParser for NativeParser {
    fn get_symbols(&self, path: &str) -> Result<Vec<Symbol>, String> {
        tracing::debug!("Getting symbol sizes for file (native): {:?}", path);
        let file_path = Path::new(path);
        let bin_data = fs::read(file_path).map_err(|e| format!("Failed to read file: {}", e))?;
        let obj_file = object::File::parse(&*bin_data)
            .map_err(|e| format!("Failed to parse ELF file: {}", e))?;
        let mut symbols = Vec::new();

        for symbol in obj_file.symbols() {
            if symbol.is_undefined() || symbol.size() == 0 {
                continue;
            }

            let name = match symbol.name() {
                Ok(name) => name.to_string(),
                Err(_) => {
                    tracing::warn!("Skipping symbol with invalid name");
                    continue;
                }
            };

            let kind = match symbol.section() {
                object::SymbolSection::Section(index) => match obj_file.section_by_index(index) {
                    Ok(section) => match section.kind() {
                        SectionKind::Text => SymbolKind::Code,
                        SectionKind::Data => SymbolKind::Data,
                        SectionKind::UninitializedData => SymbolKind::Bss,
                        SectionKind::ReadOnlyData => SymbolKind::RoData,
                        _ => SymbolKind::OtherSect,
                    },
                    Err(_) => SymbolKind::ErrSection,
                },
                object::SymbolSection::Unknown => SymbolKind::Unknown,
                object::SymbolSection::None => SymbolKind::None,
                object::SymbolSection::Undefined => SymbolKind::Undefined,
                object::SymbolSection::Absolute => SymbolKind::Absolute,
                object::SymbolSection::Common => SymbolKind::Common,
                _ => SymbolKind::Other,
            };

            symbols.push(Symbol {
                name,
                size: symbol.size() as usize,
                kind,
            });
        }

        tracing::debug!(
            "Found {} symbols in {:?} (native)",
            symbols.len(),
            file_path
        );
        symbols.sort_by(|a, b| b.size.cmp(&a.size));
        Ok(symbols)
    }
}
