use std::path::Path;
use std::process::Command;
use eyre::{Context, Result, bail};

pub fn get_symbol_sizes(file_path: &Path, demangle: bool) -> Result<Vec<Symbol>> {
    tracing::debug!("Getting symbol sizes for file: {:?}", file_path);
    let output = Command::new("nm")
        .arg("--print-size")
        .arg("--size-sort")
        .arg("--radix=d")
        .arg(file_path)
        .output()
        .context("Failed to execute nm")?;

    if !output.status.success() {
        bail!(
            "nm failed with exit code {}: {}\nstderr: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
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
            .with_context(|| format!("Failed to parse size from line: {}", line))?;
        let symbol_type = parts[2].chars().next().unwrap_or('?');
        let mut name = parts[3].to_string();

        if demangle {
            if let Ok(demangled) = rustc_demangle::try_demangle(&name) {
                name = demangled.to_string();
                tracing::trace!("Demangled {} to {}", parts[3], name);
            }
        }

        symbols.push(Symbol {
            name,
            symbol_type,
            size,
        });
    }

    tracing::debug!("Found {} symbols in {:?}", symbols.len(), file_path);
    Ok(symbols)
}

#[derive(Debug, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub symbol_type: char,
    pub size: u64,
}
