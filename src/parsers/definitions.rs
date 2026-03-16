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
    pub address: u64,
}

impl Symbol {
    pub fn demangle(&mut self) {
        self.name = demangle::_demangle_symbol_name(&self.name);
    }
}

#[cfg(test)]
mod tests {

    use crate::parsers::definitions::{Symbol, SymbolKind};

    #[test]
    fn test_symbol_demangle() {
        let mut symbol = Symbol {
            name: "_ZN3foo3barEv".to_string(),
            size: 10,
            kind: SymbolKind::Code,
            address: 0,
        };
        symbol.demangle();
        assert_eq!(symbol.name, "foo::bar()");

        let mut symbol2 = Symbol {
            name: "not_mangled".to_string(),
            size: 20,
            kind: SymbolKind::Data,
            address: 0,
        };
        symbol2.demangle();
        assert_eq!(symbol2.name, "not_mangled");
    }
}
