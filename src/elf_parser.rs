use eyre::Result;
use object::{Object, ObjectSection, ObjectSymbol, SectionKind};
use std::fs;
use std::path::Path;
use std::process::Command;

pub trait ElfParser {
    fn get_symbols(&self, path: &str) -> Result<Vec<Symbol>, String>;
}

pub struct NativeParser;

pub struct NmParser {
    pub nm_path: String,
}

impl Default for NmParser {
    fn default() -> Self {
        NmParser { nm_path: "nm".to_string() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Code,
    Data,
    Bss,
    RoData,
    Weak,
    Undefined,
    Other,
    OtherSect,
    ErrSection,
    Unknown,
    None,
    Absolute,
    Common,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Code => write!(f, "Code"),
            SymbolKind::Data => write!(f, "Data"),
            SymbolKind::Bss => write!(f, "BSS"),
            SymbolKind::RoData => write!(f, "ROData"),
            SymbolKind::Weak => write!(f, "Weak"),
            SymbolKind::Undefined => write!(f, "Undefined"),
            SymbolKind::Other => write!(f, "Other"),
            SymbolKind::OtherSect => write!(f, "OtherSect"),
            SymbolKind::ErrSection => write!(f, "ErrSection"),
            SymbolKind::Unknown => write!(f, "Unknown"),
            SymbolKind::None => write!(f, "None"),
            SymbolKind::Absolute => write!(f, "Absolute"),
            SymbolKind::Common => write!(f, "Common"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    pub name: String,
    pub size: usize,
    pub kind: SymbolKind,
}

fn _demangle_symbol_name(raw_name: &str) -> String {
    // First try Rust demangling
    if let Ok(demangled) = rustc_demangle::try_demangle(raw_name) {
        let demangled_str = demangled.to_string();
        if demangled_str != raw_name {
            tracing::trace!("Rust demangled {} to {}", raw_name, demangled_str);
            return demangled_str;
        }
    }

    // Then try C++ demangling
    match cpp_demangle::Symbol::new(raw_name.as_bytes()) {
        Ok(symbol) => match symbol.demangle(&cpp_demangle::DemangleOptions::default()) {
            Ok(demangled) => {
                tracing::trace!("C++ demangled {} to {}", raw_name, demangled);
                return demangled;
            }
            Err(_) => {
                tracing::trace!("c++ demangle failed for {}, using original", raw_name);
            }
        },
        Err(_) => {
            tracing::trace!("c++ demangle parse failed for {}, using original", raw_name);
        }
    }

    raw_name.to_string()
}

impl ElfParser for NativeParser {
    fn get_symbols(&self, path: &str) -> Result<Vec<Symbol>, String> {
        tracing::debug!("Getting symbol sizes for file (native): {:?}", path);
        let file_path = Path::new(path);
        let bin_data = fs::read(file_path).map_err(|e| format!("Failed to read file: {}", e))?;
        let obj_file = object::File::parse(&*bin_data).map_err(|e| format!("Failed to parse ELF file: {}", e))?;
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

impl ElfParser for NmParser {
    fn get_symbols(&self, path: &str) -> Result<Vec<Symbol>, String> {
        tracing::debug!("Getting symbol sizes for file (nm): {:?}", path);
        let file_path = Path::new(path);
        let output = Command::new(&self.nm_path)
            .arg("--print-size")
            .arg("--size-sort")
            .arg("--radix=d")
            .arg(file_path)
            .output()
            .map_err(|e| format!("Failed to execute nm: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "nm failed with exit code {}: {}\nstderr: {}",
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut symbols = Vec::new();

        for line in output_str.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            if parts.len() != 4 {
                tracing::warn!("Skipping malformed line: {}", line);
                continue;
            }

            let size: u64 = parts[1]
                .parse()
                .map_err(|e| format!("Failed to parse size from line {}: {}", line, e))?;
            let symbol_type = parts[2].chars().next().unwrap_or('?');
            let name = parts[3].to_string();

            let kind = match symbol_type {
                'T' | 't' => SymbolKind::Code,
                'D' | 'd' => SymbolKind::Data,
                'B' | 'b' => SymbolKind::Bss,
                'R' | 'r' => SymbolKind::RoData,
                'W' | 'w' => SymbolKind::Weak,
                'U' => SymbolKind::Undefined,
                _ => SymbolKind::Other,
            };

            symbols.push(Symbol {
                name,
                size: size as usize,
                kind,
            });
        }

        tracing::debug!("Found {} symbols in {:?} (nm)", symbols.len(), file_path);
        Ok(symbols)
    }
}

pub fn demangle_symbols(symbols: &mut [Symbol]) {
    for symbol in symbols {
        symbol.name = _demangle_symbol_name(&symbol.name);
    }
}
