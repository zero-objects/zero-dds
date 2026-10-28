// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wire-probe`. Safety classification: **SAFE**.
//!
//! One IDL type plus one JSON value, in; the bytes that type takes on
//! the wire, out. This is the ZeroDDS side of the interop suite —
//! `docs/specs/zerodds-interop-suite-1.0.md`.
//!
//! ## Why it is reflective and not generated
//!
//! A probe that went through the code generator would test the
//! generator. This one walks the model and the layout plan directly, so
//! a difference between it and a foreign implementation is a difference
//! in what the plan says, which is the thing under test. The generated
//! path is tested separately, by `zerodds-wire-conformance`.
//!
//! ## The value format
//!
//! One JSON document per sample, read against the type rather than
//! against a Rust struct, so an unknown field or a missing member is an
//! error naming the path rather than a silent default.
//!
//! | IDL | JSON |
//! |---|---|
//! | `boolean` | `true` / `false` |
//! | integers up to 32 bits | number |
//! | `long long`, `unsigned long long` | **string** — a JSON number loses precision past 2^53 |
//! | `float`, `double`, `long double` | number, written round-trip exact |
//! | `char`, `wchar` | one-character string |
//! | `string`, `wstring` | string |
//! | `fixed<P, S>` | **string** — a decimal, `"123.45"`, because a JSON number is a double |
//! | `enum` | the enumerator's name, as a string |
//! | `bitmask` | array of the set bit names |
//! | `bitset` | object, keyed by bitfield name, each an integer |
//! | structure | object, keyed by member name |
//! | `sequence`, array | array |
//! | `map<K, V>` | **array of two-element arrays** — a JSON object keys on a string, and `K` need not be one |
//! | `union` | object with `_d` and the selected member |
//! | `@optional` absent | the key is left out — `null` is an error |
//!
//! ## Layer position
//!
//! A tool over `zerodds-wire-codec`, which is the plan executor itself
//! and knows nothing about JSON. What is here is the probe: the value
//! file's representation (`json`), the front end that turns IDL text
//! into a model, and the five foreign-column emitters — C for Cyclone
//! and RTI, C++ for Fast DDS, C for the ROS type supports, and the three
//! CORBA columns.
//!
//! The engine modules below — [`canonical`], [`contract`], [`mod@decode`],
//! [`default`], [`mod@encode`], [`value`] — are that crate's, pinned to
//! `serde_json::Value`. Nothing is re-derived here: the wrappers forward
//! and the signatures are the ones every call site already had.
#![forbid(unsafe_code)]

/// The corpus cases the IDL front end refuses, so that no walk over
/// `tests/interop-corpus/` gets a model for them.
///
/// Both are in the corpus deliberately, and both have a README saying
/// every implementation must refuse the file. What changed is *where*
/// the refusal happens: it used to arrive at the layout plan or at the
/// value contract, with a model in hand; `zerodds_idl_compose` carries
/// the two clauses now, so `model_from_idl` answers `Err` and there is
/// no model to walk.
///
/// - `recursive-array` — `struct Node { long value; Node pair[2]; }`.
///   A cycle with nothing on it that terminates a value: an array's
///   element count is fixed and non-zero, so no value of the type is
///   finite. IDL 4.2 §7.4.1.4.4.4.4.
/// - `resolve-module-named-as-type` — `module common { struct common
///   { … }; }`. IDL 4.2 §7.5.2 does not admit a name redefined within
///   the immediate scope of the construct that carries it, and prints
///   `module M { typedef short M; }` as its own `// Error` example. The
///   case measures which vendors refuse the file and which resolve it
///   anyway.
///
/// `tools/idlc/tests/corpus.rs` carries both as `Intended` diagnostics,
/// with the same two clauses.
///
/// A list and never a pattern: a case that stops being refused is as
/// much a finding as one that starts, and each walk that reads this
/// asserts the refusal it skips actually happened.
pub const REFUSED_BY_THE_FRONT_END: [&str; 2] = ["recursive-array", "resolve-module-named-as-type"];

pub mod canonical;
pub mod contract;
pub mod corba_cpp;
pub mod corba_idl;
pub mod corba_java;
pub mod decode;
pub mod default;
pub mod dump;
pub mod dump_cpp;
pub mod dump_ros;
pub mod dump_rust;
pub mod encode;
pub mod fill;
pub mod fill_cpp;
pub mod fill_ros;
pub mod fill_rust;
pub mod matrix;
pub mod recursion;
pub mod ros_idl;
pub mod target_keywords;
pub mod value;

pub use canonical::{Difference, canonicalise};
pub use decode::decode;
pub use default::of_type as default_of_type;
pub use dump::dump;
pub use dump_cpp::dump_fastdds;
pub use dump_rust::dump_hdds;
pub use encode::{ProbeError, encode};
pub use fill::{Vendor, fill};
pub use value::{Emit, ProbeRequest};
pub use zerodds_wire_codec::json::EXACT_JSON_INTEGER;
