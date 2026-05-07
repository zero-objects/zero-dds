// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Repository-ID-Builder — Spec §10.7.3.1.
//!
//! Default-Form: `IDL:<scoped-name>:<major>.<minor>` mit `/` als
//! Modul-Separator. Vendor-Prefixes ueber `#pragma prefix "..."`
//! sind hier nicht modelliert (Caller-Concern).

use alloc::format;
use alloc::string::String;

/// Baut eine Repository-ID aus einem Modulpfad + Type-Name + Version.
///
/// Beispiel:
/// `build_repository_id(&["MyApp", "Trading"], "Order", 1, 0)`
/// → `"IDL:MyApp/Trading/Order:1.0"`.
#[must_use]
pub fn build_repository_id(modules: &[&str], type_name: &str, major: u16, minor: u16) -> String {
    let mut path = String::new();
    for (i, m) in modules.iter().enumerate() {
        if i > 0 {
            path.push('/');
        }
        path.push_str(m);
    }
    if !modules.is_empty() {
        path.push('/');
    }
    path.push_str(type_name);
    format!("IDL:{path}:{major}.{minor}")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn flat_module_path() {
        let s = build_repository_id(&["demo"], "Echo", 1, 0);
        assert_eq!(s, "IDL:demo/Echo:1.0");
    }

    #[test]
    fn nested_module_path() {
        let s = build_repository_id(&["MyApp", "Trading"], "Order", 1, 0);
        assert_eq!(s, "IDL:MyApp/Trading/Order:1.0");
    }

    #[test]
    fn root_level_no_modules() {
        let s = build_repository_id(&[], "Object", 1, 0);
        assert_eq!(s, "IDL:Object:1.0");
    }

    #[test]
    fn version_propagates() {
        let s = build_repository_id(&["x"], "Y", 2, 5);
        assert_eq!(s, "IDL:x/Y:2.5");
    }
}
