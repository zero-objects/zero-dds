//! Build script for zerodds-py. Sets the PyO3 linker flags for
//! `--features extension-module` even on a direct cargo build
//! (without the maturin wrapper).
//!
//! Spec basis: <https://pyo3.rs/v0.22/building_and_distribution#the-extension-module-feature>
//! — when a Python extension module is built directly with cargo,
//! the linker must know that the `Py*` symbols are resolved only at
//! Python runtime.

fn main() {
    // Only active when the feature is set. Otherwise a no-op.
    if std::env::var("CARGO_FEATURE_EXTENSION_MODULE").is_ok() {
        let target = std::env::var("TARGET").unwrap_or_default();
        if target.contains("apple-darwin") || target.contains("apple-ios") {
            // macOS: resolve symbols late (Python provides them on
            // import).
            println!("cargo:rustc-link-arg=-undefined");
            println!("cargo:rustc-link-arg=dynamic_lookup");
        }
        // Linux/Windows: PyO3's extension-module feature produces
        // the right flags anyway.
    }
}
