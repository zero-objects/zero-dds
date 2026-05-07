// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-lint` — Custom-Lints und Projekt-Regeln fuer ZeroDDS.
//!
//! Safety classification: **TOOLING** (kein Runtime-Code, nicht safety-relevant).
//! Public-Strategy: **internal-only** (🏠) — wird nicht nach crates.io gepusht.
//!
//! ## Schichten-Position
//!
//! Tooling — laeuft via `cargo run -p zerodds-lint -- check` als CLI-Binary.
//! Wird in CI (`.gitlab-ci.yml::zerodds-lint`) und im pre-commit-Hook
//! (`scripts/pre-commit.sh`) ausgefuehrt; erzwingt 0 errors / 0 warnings
//! workspace-weit als RC1-DoD-Gate.
//!
//! Implementiert die in `docs/architecture/04_safety_by_architecture.md §3.4`
//! spezifizierten Projekt-Lints **AST-basiert auf stable Rust**, ohne
//! Nightly-Toolchain oder dylint.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! Tooling-Crate — die Public-API ist primaer fuer den eingebetteten
//! Binary `zerodds-lint`. Externe Konsumenten der Lint-Library sind moeglich
//! (z.B. eigener Lint-Runner mit Custom-Lints), bleiben aber Phase-2.
//!
//! - [`classification::SafetyClass`] / [`classification::parse_from_lib_rs`]
//!   / [`classification::read_from_file`] — Safety-Klasse pro Crate aus
//!   `lib.rs`-Doc-Kommentar.
//! - [`scanner::CrateInfo`] / [`scanner::scan_workspace`] — Workspace-Walk
//!   via `cargo_metadata`, Datei-Enumeration.
//! - [`diagnostic::Diagnostic`] / [`diagnostic::Severity`] —
//!   Diagnose-Datentyp mit Display-Impl.
//! - [`lints::FileLint`] / [`lints::CrateLint`] / [`lints::FileLintContext`]
//!   — Trait-Familie + Default-Lints-Liste.
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
//! ## Aufruf
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
