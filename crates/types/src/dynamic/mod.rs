// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes 1.3 §7.5 DynamicType-API.
//!
//! Foundation for the XTypes 1.3 type-system completion. The goal is a
//! programmatic, spec-faithful runtime view of XTypes data models
//! (struct/union/enum/sequence/...) that lives independently of the
//! `TypeObject` wire format — and can be mapped to the wire format via
//! the `bridge` submodule (spec §7.6).
//!
//! # Module
//!
//! - `descriptor` — `TypeDescriptor`, `MemberDescriptor`, `TypeKind`,
//!   `ExtensibilityKind`, `TryConstructKind`.
//! - `type_` — `DynamicType`, `DynamicTypeMember`.
//! - `builder` — `DynamicTypeBuilder`, `DynamicTypeBuilderFactory`.
//! - `data` — `DynamicData` + 12 typed accessors + Sequence/Composite
//!   + Loan-Stub.
//! - `factory` — `DynamicDataFactory`.
//! - `bridge` — TypeObject ↔ DynamicType.
//! - `error` — `DynamicError`.
//!
//! # Implementation scope
//!
//! - Try-construct **apply** logic (DISCARD/USE_DEFAULT/TRIM on a
//!   decoder failure) — comes with C4.7. Here only the enum +
//!   member field.
//! - Full loan refcount with lifetime tracking — the current
//!   implementation is an RAII wrapper with an active set.
//! - Annotations apply (spec §7.4 builtin annotations) — comes with
//!   C4.3.
//! - Bitset/bitmask in the bridge path — the TypeObject bridge for these
//!   kinds follows with the C4.5 XML schema loader.

pub mod bridge;
pub mod builder;
pub mod builtin_types;
pub mod codec;
pub mod collection;
pub mod data;
pub mod descriptor;
pub mod error;
pub mod factory;
pub mod try_construct;
pub mod type_;

pub use builder::{DynamicTypeBuilder, DynamicTypeBuilderFactory};
pub use builtin_types::{
    NAME_DDS_BYTES, NAME_DDS_KEYED_BYTES, NAME_DDS_KEYED_STRING, NAME_DDS_STRING,
    all_builtin_types, dds_bytes, dds_keyed_bytes, dds_keyed_string, dds_string,
    is_builtin_type_name,
};
pub use data::{DataLoan, DynamicData, DynamicValue};
pub use descriptor::{
    ExtensibilityKind, MemberDescriptor, MemberId, TryConstructKind, TypeDescriptor, TypeKind,
};
pub use error::DynamicError;
pub use factory::DynamicDataFactory;
pub use try_construct::{TryConstructOutcome, apply_try_construct};
pub use type_::{DynamicType, DynamicTypeMember};
