use std::path::Path;
use std::process::Command;

pub fn get_symbol_sizes(file_path: &Path, demangle: bool) -> Result<Vec<Symbol>, String> {
    let output = Command::new("nm")
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
            eprintln!("Skipping malformed line: {}", line);
            continue;
        }

        let size: u64 = parts[1]
            .parse()
            .map_err(|e| format!("Failed to parse size: {}\nLine: {}", e, line))?;
        let symbol_type = parts[2].chars().next().unwrap_or('?');
        let mut name = parts[3].to_string();

        if demangle && let Ok(demangled) = rustc_demangle::try_demangle(&name) {
            name = demangled.to_string();
        }

        symbols.push(Symbol {
            name,
            symbol_type,
            size,
        });
    }

    Ok(symbols)
}

#[derive(Debug, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub symbol_type: char,
    pub size: u64,
}
