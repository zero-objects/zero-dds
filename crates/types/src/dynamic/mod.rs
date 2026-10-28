// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The reflective type system. XTypes 1.3 §7.5.
//!
//! A type nobody compiled in, and a value of it: what a tool reads when
//! it subscribes to a topic whose IDL it has never seen, and what a peer
//! sends when it publishes one.
//!
//! ## The three things here, and the two that are elsewhere
//!
//! - [`DynamicType`] is a **view on [`zerodds_ast::Ast`]** — §7.5.2.8's
//!   read API over the model, not a second representation of a type.
//! - [`DynamicData`] is a **value**, and specifically the second
//!   implementation of [`zerodds_wire_codec::Source`] and
//!   [`zerodds_wire_codec::Sink`]. `serde_json::Value` is the first.
//! - [`to_ast`] turns a peer's `CompleteTypeObject` back into a model, so
//!   a type that arrived over discovery is read by the code that reads a
//!   type that was compiled in.
//! - [`builder`] is §7.5.2's other direction: a schema that arrives at
//!   run time — a `.proto` descriptor set, an XML type description —
//!   becomes types in a model, which is the same model IDL lowers to.
//!
//! What is **not** here is a codec. The module this replaces carried
//! 1,743 lines of one, written against a byte runtime that no longer
//! exists; rewriting it would have been a second answer to every framing
//! question `zerodds_ast_layout` answers once — which DHEADER, which
//! EMHEADER, which length code, which dialect setting. `zerodds-wire-codec`
//! executes a layout plan against a value in both directions and is
//! generic over the value, so [`DynamicData`] reaches the wire through
//! exactly the code a JSON value does:
//!
//! ```text
//! zerodds_wire_codec::encode(&ast, &request, &data)   -> Vec<u8>
//! zerodds_wire_codec::decode(&ast, &request, &bytes)  -> DynamicData
//! ```
//!
//! Nor is `@try_construct` applied here. §7.2.2.7's behaviour is applied
//! by `zerodds_wire_codec::decode` at every position that has one;
//! [`TryConstructKind`] is the annotation as the model records it, so a
//! caller reading a type can see what a peer will do.
//!
//! ## What is measured
//!
//! `tests/dynamic_corpus.rs` runs all 347 cases of
//! `tests/interop-corpus/` in all four `{XCDR1, XCDR2} × {LE, BE}`
//! combinations: the bytes a `serde_json::Value` produces are decoded
//! into a [`DynamicData`] and re-encoded, and the two byte strings are
//! compared octet for octet. A facade that changed a value would show up
//! as a case that moved, by name.
//!
//! ## What is open
//!
//! - **A recursive type's `TI_STRONGLY_CONNECTED_COMPONENT` rests on an
//!   unsettled placeholder.** [`crate::from_ast`] computes the component
//!   identifier and [`to_ast`] resolves one, but one input to the hash is
//!   a decision the RTF has not made; the `docs/OPEN-ITEMS.md` entry
//!   carries it.
//! - **Custom annotations do not reach the wire form.** The five builtin
//!   member annotations of §7.3.4.5.4 are written and read back
//!   (`tests/type_object_annotations.rs`); an `AppliedAnnotation` for a
//!   user-declared annotation is not, because its annotation type has no
//!   `TypeObject` this crate produces or reads. See [`to_ast`]'s note.

pub mod annotation;
pub mod builder;
pub mod data;
mod data_equals;
mod default_literal;
pub mod descriptor;
pub mod error;
pub mod factory;
mod import;
pub mod sample;
pub mod to_ast;
pub mod type_;

pub use annotation::{
    AnnotationDescriptor, AnnotationMember, AnnotationType, ValueKind, VerbatimTextDescriptor,
    annotations_of, builtin_parameters, verbatim_texts_of,
};
pub use builder::{
    BUILTIN_TOPIC_TYPE_NAMES, DynamicTypeBuilder, LENGTH_UNLIMITED, NewMember, builtin_topic_types,
};
pub use data::DynamicData;
pub use default_literal::default_expr_text;
pub use descriptor::{
    ExtensibilityKind, MEMBER_ID_INVALID, MemberDescriptor, MemberId, TK_ALIAS, TK_ANNOTATION,
    TK_ARRAY, TK_BITMASK, TK_BITSET, TK_BOOLEAN, TK_BYTE, TK_CHAR8, TK_CHAR16, TK_ENUM, TK_FLOAT32,
    TK_FLOAT64, TK_FLOAT128, TK_INT8, TK_INT16, TK_INT32, TK_INT64, TK_MAP, TK_NONE, TK_SEQUENCE,
    TK_STRING8, TK_STRING16, TK_STRUCTURE, TK_UINT8, TK_UINT16, TK_UINT32, TK_UINT64, TK_UNION,
    TryConstructKind, TypeDescriptor, TypeKind, TypeKindValue,
};
pub use error::DynamicError;
pub use factory::{AnnotationTypeBuilder, DynamicTypeBuilderFactory, TypeBuilder};
pub use sample::{
    BooleanSeq, ByteSeq, CharSeq, DynamicDataFactory, DynamicSample, Float32Seq, Float64Seq,
    Float128Seq, Int8Seq, Int16Seq, Int32Seq, Int64Seq, StringSeq, UInt8Seq, UInt16Seq, UInt32Seq,
    UInt64Seq, WcharSeq, WstringSeq,
};
pub use to_ast::{ModelFromTypeObjects, dynamic_type};
pub use type_::{DynamicType, DynamicTypeMember};
