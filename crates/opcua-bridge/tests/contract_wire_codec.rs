// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-bridge` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-codec`, only with its `daemon`
//! feature (`src/dynamic_value.rs::default_data`, called from
//! `ua_client.rs`, `pubsub_glue.rs` and `daemon/server.rs`):
//! `default::of_type::<DynamicData>(ty.ast(), ty.id(), &ty.qualified_name())`
//! as the initial fields of a `PublishedDataSet` and the starting value of
//! `unflatten`; `ProbeError` is the error type of that public function.
//!
//! Oracle: DDS-XTypes 1.3 §7.2.2.4.4.4.7 Table 9 (`BOOLEAN` False,
//! integers and floats 0, `STRING` "", `SEQUENCE` empty, `ENUM` the first
//! literal, `STRUCTURE` its non-optional members at their defaults, an
//! `@optional` member absent) and §7.2.2.4.4.4.9 (a member's explicit
//! default first).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use zerodds_ast::{Ast, PrimitiveKind};
use zerodds_types::dynamic::{DynamicData, DynamicType};

fn model(idl: &str) -> Arc<Ast> {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        idl,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(
        !composed.failed(),
        "the fixture does not compose: {:?}",
        composed.diagnostics
    );
    Arc::new(composed.ast)
}

#[test]
fn the_error_of_default_data_is_the_codecs() {
    let _: fn(&DynamicType) -> Result<DynamicData, zerodds_wire_codec::ProbeError> =
        zerodds_opcua_bridge::dynamic_value::default_data;
    fn shown<E: core::fmt::Display + core::fmt::Debug>() {}
    shown::<zerodds_wire_codec::ProbeError>();
}

#[test]
fn xtypes_7_2_2_4_4_4_7_the_initial_value_of_a_published_data_set() {
    let ast = model(
        "module M {
            enum Mode { IDLE, RUN };
            @final struct Sample {
                boolean on;
                @default(7) long level;
                double ratio;
                string label;
                sequence<long> history;
                Mode state;
                @optional long spare;
            };
        };",
    );
    let ty = DynamicType::find(&ast, "M::Sample").unwrap();
    let value: DynamicData =
        zerodds_wire_codec::default::of_type(ty.ast(), ty.id(), &ty.qualified_name()).unwrap();
    let DynamicData::Aggregate(members) = value else {
        panic!("a structure's default is an aggregate");
    };
    let get = |name: &str| {
        members
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(get("on"), Some(DynamicData::Boolean(false)));
    assert_eq!(
        get("level"),
        Some(DynamicData::integer(PrimitiveKind::Int32, 7)),
        "the explicit default first"
    );
    assert_eq!(
        get("ratio"),
        Some(DynamicData::float(PrimitiveKind::Float64, 0.0))
    );
    assert_eq!(get("label"), Some(DynamicData::Text(String::new())));
    assert_eq!(get("history"), Some(DynamicData::Collection(Vec::new())));
    assert_eq!(
        get("state"),
        Some(DynamicData::Enumerator(String::from("IDLE")))
    );
    assert_eq!(get("spare"), None, "an @optional member starts absent");
}
