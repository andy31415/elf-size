use super::definitions::{ElfParser, Symbol, SymbolKind};
use eyre::Result;
use std::path::Path;
use std::process::Command;

pub struct NmParser {
    pub nm_path: String,
}

impl Default for NmParser {
    fn default() -> Self {
        NmParser {
            nm_path: "nm".to_string(),
        }
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
