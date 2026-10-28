// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-AMQP 1.0 §7 and §8.1 — the type-driven half of the mapping.
//!
//! The wire codec under this module is type-blind on purpose: it encodes
//! an [`AmqpExtValue`] and decodes octets, and neither operation needs
//! to know what IDL type produced them. §7 and §8.1 are the other half
//! and cannot be written without a type — which is why
//! `crates/amqp-endpoint/`'s `encode_dds_to_amqp_body` writes
//! `{"_xcdr2":"<hex>"}` in JSON mode and says in a comment that §8.1.2
//! "requires type reflection".
//!
//! ## The three consumers of one reading
//!
//! [`plan`] reads §7 once, off the model, into a [`TypePlan`]. Then:
//!
//! - [`encode`] applies it in Rust: [`described_composite`] is §8.1.3's
//!   body and [`json_document`] is §8.1.2's.
//! - [`emit`] applies it in C++, Java and TypeScript, writing the
//!   `toAmqpValue<T>` and `toJsonString<T>` helpers a generated
//!   binding carries beside its types.
//! - A caller with a `TypeIdentifier` and a decoded sample uses the
//!   first directly.
//!
//! The emitted helpers and the Rust reference make the **same**
//! decisions because they walk the same plan. That is checked by
//! running them: `tests/cross_language_agreement.rs` compiles the
//! emitted C++, Java and TypeScript, runs them over one sample, and
//! compares their octets and their JSON against
//! [`described_composite`] and [`json_document`]. Where a toolchain is
//! missing the test says which one and skips loudly.
//!
//! [`AmqpExtValue`]: crate::extended_types::AmqpExtValue

pub mod emit;
pub mod encode;
pub mod plan;

pub use emit::{
    CodegenLanguage, EmitError, carrier_for, carriers_for, helper_for, helpers_for, runtime_source,
    time_carrier_source,
};
pub use encode::{
    EncodeError, EncodeOptions, PlanSet, SampleValue, described_composite, json_document,
};
pub use plan::{BodyForm, Descriptor, FieldEncoding, FieldPlan, PlanError, TypePlan, plan_of};
