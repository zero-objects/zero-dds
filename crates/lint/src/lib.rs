// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-lint` — custom lints and project rules for ZeroDDS.
//!
//! Safety classification: **TOOLING** (no runtime code, not safety-relevant).
//! Public strategy: **internal-only** (🏠) — not pushed to crates.io.
//!
//! ## Layer position
//!
//! Tooling — runs via `cargo run -p zerodds-lint -- check` as a CLI binary.
//! Runs in CI (`.gitlab-ci.yml::zerodds-lint`) and in the pre-commit hook
//! (`scripts/pre-commit.sh`); enforces 0 errors / 0 warnings
//! workspace-wide as the RC1-DoD gate.
//!
//! Implements the project lints specified in
//! `docs/architecture/04_safety_by_architecture.md §3.4` **AST-based on stable
//! Rust**, without a nightly toolchain or dylint.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! Tooling crate — the public API is primarily for the embedded
//! `zerodds-lint` binary. External consumers of the lint library are possible
//! (e.g. a custom lint runner with custom lints), but remain phase 2.
//!
//! - [`classification::SafetyClass`] / [`classification::parse_from_lib_rs`]
//!   / [`classification::read_from_file`] — safety class per crate from the
//!   `lib.rs` doc comment.
//! - [`scanner::CrateInfo`] / [`scanner::scan_workspace`] — workspace walk
//!   via `cargo_metadata`, file enumeration.
//! - [`diagnostic::Diagnostic`] / [`diagnostic::Severity`] —
//!   diagnostic data type with a Display impl.
//! - [`lints::FileLint`] / [`lints::CrateLint`] / [`lints::FileLintContext`]
//!   — trait family + default lints list.
//! - [`runner::run`] / [`runner::RunConfig`] / [`runner::RunReport`] /
//!   [`runner::locate_workspace_root`] — orchestriert Scanner + Lints +
//!   Reporter.
//!
//! ## Lint-Inventar (7 aktiv registrierte Lints)
//!
//! - [`lints::require_safety_comment::RequireSafetyComment`]
//! - [`lints::no_dyn_in_safe::NoDynInSafe`]
//! - [`lints::no_panic_in_safe::NoPanicInSafe`]
//! - [`lints::no_alloc_in_hot_path::NoAllocInHotPath`]
//! - [`lints::no_realloc_in_hot_path::NoReallocInHotPath`]
//! - [`lints::bounded_recursion::BoundedRecursion`]
//! - [`lints::safety_classification_present::SafetyClassificationPresent`]
//!   (Crate-Level)
//!
//! ## Invocation
//!
//! ```text
//! cargo run -p zerodds-lint -- check
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod classification;
pub mod diagnostic;
pub mod lints;
pub mod runner;
pub mod scanner;
