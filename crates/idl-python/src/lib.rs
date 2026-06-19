// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-python`. Safety classification: **STANDARD**.
//!
//! IDL4 → Python code generator for ZeroDDS data types. Reads the IDL AST
//! from `zerodds-idl` and emits `@idl_struct(...)` + `@dataclass`
//! classes that meet the convention of the `zerodds-py` crate.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only.
//!
//! # Layer position
//!
//! Layer 3 (schema) — analogous to `zerodds-idl-cpp` / `-csharp` / `-java` /
//! `-rust` / `-ts`, but emits Python code that runs against the
//! `zerodds` Python library at runtime.
//!
//! # Public API
//!
//! - [`generate_python_module`] — AST + options → Python module source.
//! - [`PythonGenOptions`] — codegen options.
//! - [`error::IdlPythonError`] — error family.
//!
//! # What is emitted
//!
//! Per IDL construct:
//!
//! | IDL | Python |
//! |-----|--------|
//! | `struct` (any extensibility) | `@idl_struct(typename=...)` + `@dataclass class` |
//! | `enum` | `class X(IntEnum)` |
//! | `module M { ... }` | flat class name `M_Inner` (Python-PSM convention) |
//! | `boolean` | `bool` |
//! | `short`/`long`/`long long` | `Int16` / `Int32` / `Int64` (zerodds.idl brands) |
//! | `unsigned short`/... | `UInt16` / `UInt32` / `UInt64` |
//! | `octet` | `Octet` |
//! | `float` / `double` / `long double` | `Float32` / `Float64` / `LongDouble` |
//! | `char` / `wchar` | `Char` / `WChar` |
//! | `string` / `wstring` | `String` / `WString` |
//! | `sequence<T>` | `List[T]` |
//! | `T[N]` | `List[T]` (multi-dim → nested) |
//!
//! # Phase-2 material (today `Unsupported`)
//!
//! - `union` with a discriminator
//! - `bitset` / `bitmask`
//! - `typedef T name` (for now without a comment; phase 2: `typing.TypeAlias`)
//! - `valuetype`, `interface`, `exception`
//! - `fixed`, `map`, `any`
//!
//! # Example
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
