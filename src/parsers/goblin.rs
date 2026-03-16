use crate::parsers::definitions::{ElfParser, Symbol, SymbolKind};
use goblin::elf;
use std::fs;
use std::path::Path;

pub struct GoblinParser;

impl ElfParser for GoblinParser {
    fn get_symbols(&self, path: &str) -> std::result::Result<Vec<Symbol>, String> {
        let path = Path::new(path);
        let buffer = fs::read(path).map_err(|e| e.to_string())?;
        let elf = elf::Elf::parse(&buffer).map_err(|e| e.to_string())?;

        let mut symbols = Vec::new();

        for sym in &elf.syms {
            if sym.st_type() == elf::sym::STT_NOTYPE && sym.st_size == 0 {
                continue; // Skip symbols without type or size
            }

            let name = elf.strtab.get_at(sym.st_name).unwrap_or("").to_string();

            let kind = map_symbol_kind(&sym, &elf);

            symbols.push(Symbol {
                name: crate::parsers::demangle::_demangle_symbol_name(&name),
                size: sym.st_size as usize,
                kind,
            });
        }

        Ok(symbols)
    }
}

fn map_symbol_kind(sym: &elf::sym::Sym, elf: &elf::Elf) -> SymbolKind {
    match sym.st_shndx {
        s if s == elf::section_header::SHN_UNDEF as usize => return SymbolKind::Undefined,
        s if s == elf::section_header::SHN_ABS as usize => return SymbolKind::Absolute,
        s if s == elf::section_header::SHN_COMMON as usize => return SymbolKind::Common,
        _ => {}
    }

    if sym.st_bind() == elf::sym::STB_WEAK {
        return SymbolKind::Weak;
    }

    match sym.st_type() {
        elf::sym::STT_OBJECT | elf::sym::STT_COMMON => {
            // Further check section flags for ROData
            if let Some(shdr) = elf.section_headers.get(sym.st_shndx) {
                if shdr.sh_flags & u64::from(elf::section_header::SHF_WRITE) == 0 {
                    SymbolKind::RoData
                } else if shdr.sh_flags & u64::from(elf::section_header::SHF_ALLOC) != 0 {
                    // Heuristic for BSS: ALLOC but not WRITE, and NOBITS type
                    if shdr.sh_type == elf::section_header::SHT_NOBITS {
                        SymbolKind::Bss
                    } else {
                        SymbolKind::Data
                    }
                } else {
                    SymbolKind::Data
                }
            } else {
                SymbolKind::Data // Default to Data if section header not found
            }
        }
        elf::sym::STT_FUNC => SymbolKind::Code,
        elf::sym::STT_FILE => SymbolKind::Other, // Or a new kind for File?
        elf::sym::STT_SECTION => SymbolKind::OtherSect,
        elf::sym::STT_NOTYPE => {
            if let Some(shdr) = elf.section_headers.get(sym.st_shndx) {
                if shdr.sh_flags & u64::from(elf::section_header::SHF_EXECINSTR) != 0 {
                    SymbolKind::Code
                } else if shdr.sh_flags & u64::from(elf::section_header::SHF_WRITE) == 0
                    && shdr.sh_flags & u64::from(elf::section_header::SHF_ALLOC) != 0
                {
                    SymbolKind::RoData
                } else if shdr.sh_type == elf::section_header::SHT_NOBITS {
                    SymbolKind::Bss
                } else {
                    SymbolKind::Data
                }
            } else {
                SymbolKind::Unknown
            }
        }
        _ => SymbolKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Helper to create a dummy ELF file (very basic)
    fn create_dummy_elf() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        // This is not a valid ELF, but enough to mock file reading
        // TODO: Find a way to create a minimal valid ELF for testing
        writeln!(file, "ELF content placeholder").unwrap();
        file
    }

    #[test]
    fn test_goblin_parser_basic() {
        // TODO: Implement a real test with a minimal ELF file
        let parser = GoblinParser;
        // let elf_file = create_dummy_elf();
        // let symbols = parser.get_symbols(elf_file.path().to_str().unwrap());
        // assert!(symbols.is_ok());
    }
}
