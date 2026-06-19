// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Lint definitions.
//!
//! Each lint implements either [`FileLint`] (per source file) or
//! [`CrateLint`] (per crate). The [`runner`](crate::runner) collects the
//! activated lints and runs them over the `CrateInfo`s provided by the
//! [`scanner`](crate::scanner).

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

/// Context for file-based lints.
pub struct FileLintContext<'a> {
    /// Path of the file to check.
    pub file: &'a Path,
    /// Raw source text of the file.
    pub source: &'a str,
    /// Geparster AST.
    pub ast: &'a syn::File,
    /// Classification of the containing crate, if present.
    pub crate_class: Option<SafetyClass>,
    /// Crate name (for reports).
    pub crate_name: &'a str,
}

/// Lint that runs per source file.
pub trait FileLint {
    /// Maschinenlesbarer Lint-Name (z.B. `dds_require_safety_comment`).
    fn name(&self) -> &'static str;

    /// Runs the lint and returns diagnostics.
    fn check(&self, ctx: &FileLintContext<'_>) -> Vec<Diagnostic>;
}

/// Lint that runs per crate.
pub trait CrateLint {
    /// Maschinenlesbarer Lint-Name.
    fn name(&self) -> &'static str;

    /// Runs the lint and returns diagnostics.
    fn check(&self, krate: &CrateInfo) -> Vec<Diagnostic>;
}

/// Returns the active file lints.
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

/// Returns the crate lints active in task 1.
#[must_use]
pub fn default_crate_lints() -> Vec<Box<dyn CrateLint>> {
    vec![Box::new(
        safety_classification_present::SafetyClassificationPresent,
    )]
}
