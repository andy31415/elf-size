use crate::parsers::demangle;
use eyre::Result;

pub trait ElfParser {
    fn get_symbols(&self, path: &str) -> Result<Vec<Symbol>, String>;
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

impl Symbol {
    pub fn demangle(&mut self) {
        self.name = demangle::_demangle_symbol_name(&self.name);
    }
}
