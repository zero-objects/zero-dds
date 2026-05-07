//! Build-Script fuer zerodds-py. Setzt die PyO3-Linker-Flags fuer
//! `--features extension-module` auch beim direkten cargo-Build
//! (ohne maturin-Wrapper).
//!
//! Spec-Basis: <https://pyo3.rs/v0.22/building_and_distribution#the-extension-module-feature>
//! — wenn ein Python-Extension-Module direkt mit cargo gebaut wird,
//! muss der Linker wissen dass die `Py*`-Symbole erst zur Python-
//! Laufzeit aufgeloest werden.

fn main() {
    // Nur aktiv wenn das Feature gesetzt ist. Sonst no-op.
    if std::env::var("CARGO_FEATURE_EXTENSION_MODULE").is_ok() {
        let target = std::env::var("TARGET").unwrap_or_default();
        if target.contains("apple-darwin") || target.contains("apple-ios") {
            // macOS: Symbole spaet aufloesen (Python liefert sie beim
            // Import).
            println!("cargo:rustc-link-arg=-undefined");
            println!("cargo:rustc-link-arg=dynamic_lookup");
        }
        // Linux/Windows: PyO3's extension-module Feature erzeugt
        // ohnehin die richtigen Flags.
    }
}
