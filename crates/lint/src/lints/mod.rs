// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Lint-Definitionen.
//!
//! Jeder Lint implementiert entweder [`FileLint`] (per Quelldatei) oder
//! [`CrateLint`] (per Crate). Der [`runner`](crate::runner) sammelt die
//! aktivierten Lints und fuehrt sie ueber die vom [`scanner`](crate::scanner)
//! gelieferten `CrateInfo`s aus.

use std::path::Path;

use crate::classification::SafetyClass;
use crate::diagnostic::Diagnostic;
use crate::scanner::CrateInfo;

pub mod bounded_recursion;
pub mod no_alloc_in_hot_path;
pub mod no_dyn_in_safe;
pub mod no_panic_in_safe;
pub mod no_realloc_in_hot_path;
pub mod require_safety_comment;
pub mod safety_classification_present;

/// Kontext fuer File-basierte Lints.
pub struct FileLintContext<'a> {
    /// Pfad der zu pruefenden Datei.
    pub file: &'a Path,
    /// Roher Quelltext der Datei.
    pub source: &'a str,
    /// Geparster AST.
    pub ast: &'a syn::File,
    /// Klassifikation der enthaltenden Crate, falls vorhanden.
    pub crate_class: Option<SafetyClass>,
    /// Crate-Name (fuer Reports).
    pub crate_name: &'a str,
}

/// Lint, der pro Quelldatei laeuft.
pub trait FileLint {
    /// Maschinenlesbarer Lint-Name (z.B. `dds_require_safety_comment`).
    fn name(&self) -> &'static str;

    /// Fuehrt den Lint aus und liefert Diagnosen.
    fn check(&self, ctx: &FileLintContext<'_>) -> Vec<Diagnostic>;
}

/// Lint, der pro Crate laeuft.
pub trait CrateLint {
    /// Maschinenlesbarer Lint-Name.
    fn name(&self) -> &'static str;

    /// Fuehrt den Lint aus und liefert Diagnosen.
    fn check(&self, krate: &CrateInfo) -> Vec<Diagnostic>;
}

/// Liefert die aktiven File-Lints.
#[must_use]
pub fn default_file_lints() -> Vec<Box<dyn FileLint>> {
    vec![
        Box::new(require_safety_comment::RequireSafetyComment),
        Box::new(no_dyn_in_safe::NoDynInSafe),
        Box::new(no_panic_in_safe::NoPanicInSafe),
        Box::new(no_alloc_in_hot_path::NoAllocInHotPath),
        Box::new(no_realloc_in_hot_path::NoReallocInHotPath),
        Box::new(bounded_recursion::BoundedRecursion),
    ]
}

/// Liefert die in Task 1 aktiven Crate-Lints.
#[must_use]
pub fn default_crate_lints() -> Vec<Box<dyn CrateLint>> {
    vec![Box::new(
        safety_classification_present::SafetyClassificationPresent,
    )]
}
