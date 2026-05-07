// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Typisierter Abstract Syntax Tree fuer OMG IDL 4.2 (T5.1).
//!
//! Im Gegensatz zum [`crate::cst`] verliert der AST grammatik-spezifische
//! Boilerplate (Listen-Helfer, Disambiguation-Alternativen) und behaelt nur
//! die semantisch relevante Struktur. Jeder Node traegt eine [`Span`] auf
//! den Source-Text fuer Diagnostik.
//!
//! # Layout
//! - [`types`] — alle AST-Strukturen und Enums.
//! - `builder` (T5.2) — CST → AST-Konversion.
//! - `print` (T5.3) — Display/Pretty-Print.
//!
//! Modul-Wurzel re-exportiert die Public-Types fuer ergonomischen Zugriff.

pub mod builder;
pub mod print;
pub mod types;

pub use builder::{BuildResult, BuilderError, build};
pub use types::*;
