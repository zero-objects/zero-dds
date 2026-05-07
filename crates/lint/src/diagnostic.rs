// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Diagnostic-Datentyp + textueller Reporter.
//!
//! Format: `path:line:col [lint_name] message`.

use std::fmt;
use std::path::{Path, PathBuf};

/// Schweregrad einer Diagnose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Hartes Fehler — laesst CI rot werden.
    Error,
    /// Warning — kein Exit-Fail, aber sichtbar.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
        })
    }
}

/// Eine einzelne Lint-Findung.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Datei, in der die Findung auftritt.
    pub file: PathBuf,
    /// Zeile (1-basiert) im Quelltext.
    pub line: usize,
    /// Spalte (1-basiert).
    pub column: usize,
    /// Lint-Name (z.B. `dds_require_safety_comment`).
    pub lint: &'static str,
    /// Schweregrad.
    pub severity: Severity,
    /// Menschen-lesbare Beschreibung.
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{} [{}] {}: {}",
            self.file.display(),
            self.line,
            self.column,
            self.severity,
            self.lint,
            self.message,
        )
    }
}

impl Diagnostic {
    /// Erzeugt eine Error-Diagnose.
    #[must_use]
    pub fn error(
        file: &Path,
        line: usize,
        column: usize,
        lint: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            file: file.to_path_buf(),
            line,
            column,
            lint,
            severity: Severity::Error,
            message: message.into(),
        }
    }

    /// Erzeugt eine Warning-Diagnose.
    #[must_use]
    pub fn warning(
        file: &Path,
        line: usize,
        column: usize,
        lint: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            file: file.to_path_buf(),
            line,
            column,
            lint,
            severity: Severity::Warning,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn format_error() {
        let d = Diagnostic::error(&PathBuf::from("foo/bar.rs"), 10, 5, "dds_test", "broken");
        assert_eq!(d.to_string(), "foo/bar.rs:10:5 [error] dds_test: broken");
    }

    #[test]
    fn format_warning() {
        let d = Diagnostic::warning(&PathBuf::from("x.rs"), 1, 1, "dds_x", "watch");
        assert_eq!(d.to_string(), "x.rs:1:1 [warning] dds_x: watch");
    }
}
