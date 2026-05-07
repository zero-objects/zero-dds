// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes 1.3 §7.5 DynamicType-API.
//!
//! Foundation für die XTypes-1.3-Type-System-Vervollständigung. Ziel
//! ist eine programmatische, Spec-treue Runtime-Sicht auf XTypes-
//! Datenmodelle (Struct/Union/Enum/Sequence/...), die unabhaengig vom
//! Wire-Format `TypeObject` lebt — und ueber das `bridge`-Submodul auf
//! das Wire-Format gemappt werden kann (Spec §7.6).
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
//! # Implementations-Scope
//!
//! - Try-Construct-**Apply**-Logik (DISCARD/USE_DEFAULT/TRIM bei
//!   Decoder-Fehlschlag) — kommt mit C4.7. Hier ist nur das Enum +
//!   Member-Feld.
//! - Voller Loan-Refcount mit Lifetime-Tracking — aktuelle
//!   Implementation ist ein RAII-Wrapper mit Active-Set.
//! - Annotations-Apply (Spec §7.4 builtin annotations) — kommt mit
//!   C4.3.
//! - Bitset/Bitmask im Bridge-Pfad — TypeObject-Bridge fuer diese
//!   Kinds folgt mit C4.5 XML-Schema-Loader.

pub mod bridge;
pub mod builder;
pub mod builtin_types;
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
