pub(crate) fn _demangle_symbol_name(raw_name: &str) -> String {
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
