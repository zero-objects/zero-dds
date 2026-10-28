// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The two generated files `build.rs` writes, compiled in.
//!
//! Crate `zerodds-rpc-rust-e2e`. Safety classification: **STANDARD**.
//!
//! `include!`, not a module the generator writes to `src/`: the whole
//! point of a golden-by-compiler gate is that nobody hand-edits what
//! `rustc` is about to check, which is `crates/wire-conformance/src/
//! lib.rs`'s own reason for the same pattern.
// No blanket allow here: a warning in what `include!` pulls in is a
// warning in the generator's own output, and `emit::interface_rust`
// answers for each one at the site it can — `non_camel_case_types` on
// the wire-named items themselves (DDS-RPC 1.0 §7.5.1.1.4-7 fixes
// their spelling; renaming them is not a fix this crate can make), a
// `#[doc]` or `#[allow(missing_docs)]` on every public item, `_`
// bindings where a match arm's payload is not read. Nothing is
// suppressed here that the generated file does not answer for itself.

/// `module demo { … }` of `build.rs`'s `CALCULATOR` source.
///
/// `build.rs` calls `zerodds_lang_rules::generate::source` directly —
/// the layer under `zerodds-idlc generate`'s own `tools/idlc::generate::
/// source`, which is what writes the `use zerodds_wire::{…}` preamble a
/// real `zerodds-idlc generate --lang rust` run carries. That preamble
/// is a CLI concern (`crates/rpc/tests/cpp_rpc_runtime.rs` builds its
/// own C++ one the same way, for the same reason), so it is written
/// once here instead.
pub mod calculator {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/calculator.rs"));
}

/// `module hierarchy { … }` of `build.rs`'s `HIERARCHY` source.
///
/// Named `hierarchy_bindings`, not `hierarchy`: the IDL module itself is
/// `hierarchy`, and `generate::source` already writes `pub mod hierarchy
/// { … }` for it (the same rule `calculator` below relies on, just with
/// `demo` as the IDL module's own name instead) — wrapping that in an
/// outer module of the same name is `clippy::module_inception`, so the
/// wrapper here is named differently instead of silencing the lint.
pub mod hierarchy_bindings {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/hierarchy.rs"));
}

/// `module wide { … }` of `build.rs`'s `WIDE` source: members of eight
/// octets, for the cross-column wire test.
pub mod wide_bindings {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/wide.rs"));
}

/// `module nested { … }` of `build.rs`'s `NESTED` source: nested
/// `@appendable` and `@mutable` members, for the classic-CDR codec.
// The structures and their codecs are declared by the general Rust
// generator (`crate::emit::rust`, `crate::encode`), which does not document
// what an IDL file did not.
#[allow(missing_docs)]
pub mod nested_bindings {
    use zerodds_wire::{ParameterRole, Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/nested.rs"));
}

/// `module dds { … } module robot { … }` of `build.rs`'s `PAIR` source: a
/// service defined by a pair of types with their own header.
// The common types of §7.5.1.1.1 keep the spelling the clause gives them.
#[allow(missing_docs, non_camel_case_types, non_snake_case)]
pub mod pair_bindings {
    use zerodds_wire::{Narrowed, Reader, TryConstruct, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/pair.rs"));
}

/// `module enh { … }` of `build.rs`'s `ENHANCED` source, generated for the
/// Enhanced Service Mapping (DDS-RPC 1.0 §7.5.1.2).
pub mod enhanced_bindings {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/enhanced.rs"));
}

/// `module emp { … }` of `build.rs`'s `EMPTY` source, generated for the
/// Enhanced Service Mapping with `@Empty` structures (§7.5.1.2.1.3).
pub mod empty_bindings {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/empty.rs"));
}

/// `module evo { … }` of `build.rs`'s `EVOLUTION_OLD` source: the old
/// version of an interface, for the Enhanced Service Mapping.
pub mod evolution_old {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/evolution_old.rs"));
}

/// `module evo { … }` of `build.rs`'s `EVOLUTION_NEW` source: the evolved
/// version of the interface of [`evolution_old`], with the same IDL names.
pub mod evolution_new {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/evolution_new.rs"));
}

/// `module evo { … }` of `build.rs`'s `EVOLUTION_CHANGED` source: the
/// interface of [`evolution_old`] with the type of a parameter changed.
pub mod evolution_changed {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/evolution_changed.rs"));
}

/// `build.rs`'s source of evolution_return: the interface of [`evolution_old`] with the return type of an operation changed.
pub mod evolution_return {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/evolution_return.rs"));
}

/// `build.rs`'s source of evolution_pose_old: an interface with a `@mutable` structure parameter.
#[allow(missing_docs)]
pub mod evolution_pose_old {
    use zerodds_wire::{ParameterRole, Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/evolution_pose_old.rs"));
}

/// `build.rs`'s source of evolution_pose_new: the interface of [`evolution_pose_old`] with a member added to the structure.
#[allow(missing_docs)]
pub mod evolution_pose_new {
    use zerodds_wire::{ParameterRole, Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/evolution_pose_new.rs"));
}

/// `module evo { … }` of `build.rs`'s `BASIC_FEWER` source: a Basic version
/// of the interface with fewer operations (§7.7.1.1).
pub mod basic_fewer {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/basic_fewer.rs"));
}

/// `module evo { … }` of `build.rs`'s `BASIC_OLD` source: the old Basic
/// version, with the operation `legacy`.
pub mod basic_old {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/basic_old.rs"));
}

/// `module evo { … }` of `build.rs`'s `BASIC_NEW` source: the evolved Basic
/// version, with the operation `extra` instead of `legacy`.
pub mod basic_new {
    use zerodds_wire::{Reader, WireError, Writer};
    include!(concat!(env!("OUT_DIR"), "/basic_new.rs"));
}
