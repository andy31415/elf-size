pub(crate) fn _demangle_symbol_name(raw_name: &str) -> String {
    // First try Rust demangling
    if let Ok(demangled) = rustc_demangle::try_demangle(raw_name) {
        let demangled_str = format!("{:#}", demangled);
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

#[cfg(test)]
mod tests {
    use super::_demangle_symbol_name;

    #[test]
    fn test_rust_demangle() {
        // Example from std library
        assert_eq!(
            _demangle_symbol_name("_ZN3std2io4Read11read_to_end17hb85a0f6802e14499E"),
            "std::io::Read::read_to_end"
        );
    }

    #[test]
    fn test_cpp_demangle() {
        // Basic C++ function void foo::bar()
        assert_eq!(_demangle_symbol_name("_ZN3foo3barEv"), "foo::bar()");
        // Basic C++ function int foo::baz(int)
        assert_eq!(_demangle_symbol_name("_ZN3foo3bazEi"), "foo::baz(int)");
    }

    #[test]
    fn test_non_mangled() {
        assert_eq!(_demangle_symbol_name("main"), "main");
        assert_eq!(
            _demangle_symbol_name("MyClass::MyMethod"),
            "MyClass::MyMethod"
        );
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(_demangle_symbol_name(""), "");
    }
}
