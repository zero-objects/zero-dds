// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Safety classification per crate, read from the `lib.rs` doc comment.
//!
//! Source of truth: every crate contains at the start of `src/lib.rs` a
//! Marker `Safety classification: **{SAFE|STANDARD|COMFORT|TOOLING}**`. This
//! is extracted by `zerodds-lint` and controls which lints are active for which crate
//! .

use std::fmt;
use std::path::Path;

/// Safety class of a crate (see `04_safety_by_architecture.md §2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafetyClass {
    /// `#![forbid(unsafe_code)]`, no_std-capable, no panic in the hot path.
    Safe,
    /// `#![deny(unsafe_code)]`, std allowed, with FFI modules.
    Standard,
    /// `#![warn(unsafe_code)]`, everything allowed with a mandatory SAFETY comment.
    Comfort,
    /// Build/tooling crate, no runtime code.
    Tooling,
}

impl fmt::Display for SafetyClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Safe => "SAFE",
            Self::Standard => "STANDARD",
            Self::Comfort => "COMFORT",
            Self::Tooling => "TOOLING",
        })
    }
}

/// Extracts the classification from a `lib.rs` source text.
///
/// Expects an occurrence of `Safety classification: **<CLASS>**` in a
/// the first ~50 lines. Tolerates suffix modifiers like
/// `**SAFE (std-only)**`.
#[must_use]
pub fn parse_from_lib_rs(content: &str) -> Option<SafetyClass> {
    for line in content.lines().take(60) {
        if let Some(idx) = line.find("Safety classification: **") {
            let rest = &line[idx + "Safety classification: **".len()..];
            // The class is everything up to the next whitespace, '*' or '('.
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '*' || c == '(')
                .unwrap_or(rest.len());
            return match rest[..end].to_ascii_uppercase().as_str() {
                "SAFE" => Some(SafetyClass::Safe),
                "STANDARD" => Some(SafetyClass::Standard),
                "COMFORT" => Some(SafetyClass::Comfort),
                "TOOLING" => Some(SafetyClass::Tooling),
                _ => None,
            };
        }
    }
    None
}

/// Reads the classification from a file. Returns `Ok(None)` if the
/// the file exists but contains no classification.
///
/// # Errors
/// I/O error reading the file.
pub fn read_from_file(path: &Path) -> std::io::Result<Option<SafetyClass>> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_from_lib_rs(&content))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_safe() {
        let src = "//! Foo\n//! Safety classification: **SAFE**\n";
        assert_eq!(parse_from_lib_rs(src), Some(SafetyClass::Safe));
    }

    #[test]
    fn parses_with_modifier() {
        let src = "//! Safety classification: **SAFE (std-only)**\n";
        assert_eq!(parse_from_lib_rs(src), Some(SafetyClass::Safe));
    }

    #[test]
    fn parses_standard_comfort_tooling() {
        assert_eq!(
            parse_from_lib_rs("//! Safety classification: **STANDARD**"),
            Some(SafetyClass::Standard)
        );
        assert_eq!(
            parse_from_lib_rs("//! Safety classification: **COMFORT**"),
            Some(SafetyClass::Comfort)
        );
        assert_eq!(
            parse_from_lib_rs("//! Safety classification: **TOOLING**"),
            Some(SafetyClass::Tooling)
        );
    }

    #[test]
    fn returns_none_when_absent() {
        let src = "//! Some doc\n//! without marker\n";
        assert_eq!(parse_from_lib_rs(src), None);
    }

    #[test]
    fn returns_none_for_unknown_class() {
        let src = "//! Safety classification: **MAGIC**\n";
        assert_eq!(parse_from_lib_rs(src), None);
    }

    #[test]
    fn ignores_marker_after_first_60_lines() {
        let src = "// blank\n".repeat(70) + "//! Safety classification: **SAFE**\n";
        assert_eq!(parse_from_lib_rs(&src), None);
    }

    #[test]
    fn display_format() {
        assert_eq!(SafetyClass::Safe.to_string(), "SAFE");
        assert_eq!(SafetyClass::Standard.to_string(), "STANDARD");
        assert_eq!(SafetyClass::Comfort.to_string(), "COMFORT");
        assert_eq!(SafetyClass::Tooling.to_string(), "TOOLING");
    }
}
