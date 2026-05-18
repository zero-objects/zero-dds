// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-python`. Safety classification: **STANDARD**.
//!
//! IDL4 → Python-Code-Generator fuer ZeroDDS-DataTypes. Liest den IDL-AST
//! aus `zerodds-idl` und emittiert `@idl_struct(...)` + `@dataclass`-
//! Klassen, die das Konvention der `zerodds-py`-Crate erfuellen.
//!
//! Build-Zeit-Tool — `forbid(unsafe_code)`, std-only.
//!
//! # Schichten-Position
//!
//! Layer 3 (Schema) — analog zu `zerodds-idl-cpp` / `-csharp` / `-java` /
//! `-rust` / `-ts`, aber emittiert Python-Code, der zur Laufzeit gegen
//! die `zerodds`-Python-Library laeuft.
//!
//! # Public API
//!
//! - [`generate_python_module`] — AST + Optionen → Python-Modul-Source.
//! - [`PythonGenOptions`] — Codegen-Optionen.
//! - [`error::IdlPythonError`] — Fehler-Familie.
//!
//! # Was wird emittiert
//!
//! Pro IDL-Konstrukt:
//!
//! | IDL | Python |
//! |-----|--------|
//! | `struct` (jede Extensibility) | `@idl_struct(typename=...)` + `@dataclass class` |
//! | `enum` | `class X(IntEnum)` |
//! | `module M { ... }` | flacher Klassenname `M_Inner` (Python-PSM-Konvention) |
//! | `boolean` | `bool` |
//! | `short`/`long`/`long long` | `Int16` / `Int32` / `Int64` (zerodds.idl-Brands) |
//! | `unsigned short`/... | `UInt16` / `UInt32` / `UInt64` |
//! | `octet` | `Octet` |
//! | `float` / `double` / `long double` | `Float32` / `Float64` / `LongDouble` |
//! | `char` / `wchar` | `Char` / `WChar` |
//! | `string` / `wstring` | `String` / `WString` |
//! | `sequence<T>` | `List[T]` |
//! | `T[N]` | `List[T]` (Multi-Dim → verschachtelt) |
//!
//! # Phase-2 Material (heute `Unsupported`)
//!
//! - `union` mit Discriminator
//! - `bitset` / `bitmask`
//! - `typedef T name` (vorerst kommentarlos; Phase-2: `typing.TypeAlias`)
//! - `valuetype`, `interface`, `exception`
//! - `fixed`, `map`, `any`
//!
//! # Beispiel
//!
//! ```rust
//! use zerodds_idl::config::ParserConfig;
//! use zerodds_idl_python::{generate_python_module, PythonGenOptions};
//!
//! let ast = zerodds_idl::parse(
//!     "struct Greeting { long id; string<128> text; };",
//!     &ParserConfig::default(),
//! )
//! .expect("parse");
//! let py_src =
//!     generate_python_module(&ast, &PythonGenOptions::default()).expect("gen");
//! assert!(py_src.contains("@dataclass"));
//! assert!(py_src.contains("class Greeting:"));
//! assert!(py_src.contains("id: Int32"));
//! assert!(py_src.contains("text: String"));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;

pub use emitter::{PythonGenOptions, generate_python_module};
pub use error::{IdlPythonError, Result};
