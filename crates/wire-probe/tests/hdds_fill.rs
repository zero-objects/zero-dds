// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust expression the HDDS fill writes, and what it refuses.
//!
//! `hddsgen` emits a data-carrying Rust enum for a union — one variant
//! per member, one payload each — so the fill has exactly one place to
//! put a value. A sample that names two branches has no expression in
//! that shape, and the emitter has to say so: the loop used to keep the
//! last key it saw and drop the rest, which produced a Rust literal of
//! the right shape for a sample it had not been given.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast_core::ScopedName;
use zerodds_wire_probe::encode::{ProbeError, model_from_idl};
use zerodds_wire_probe::fill_rust::fill_hdds;

fn emit(idl: &str, type_name: &str, values: &serde_json::Value) -> Result<String, ProbeError> {
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let segments: Vec<String> = type_name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    let type_ = ast
        .find_by_scoped_name(&ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the type");
    fill_hdds(&ast, type_, values)
}

const UNION: &str = "@final union Choice switch (long) { \
                     case 1: long whole; \
                     default: short part; };";

/// One named branch is the shape the enum has, and it is written.
#[test]
fn a_union_naming_one_member_becomes_that_variant() {
    let out = emit(UNION, "Choice", &json!({ "_d": 1, "whole": 7 })).expect("the emitter");
    assert!(out.contains("Choice::Whole("), "{out}");
    assert!(out.contains('7'), "{out}");
}

/// Two named branches are refused by name. Before this, the loop
/// assigned over its own result and the emitter answered with the last
/// key alone — `Choice::Part(3)` for a sample that also named `whole`,
/// which no reader of the recorded octets could tell from a sample that
/// had never named it.
#[test]
fn a_union_naming_two_members_is_refused_and_names_both() {
    let error = emit(UNION, "Choice", &json!({ "_d": 1, "whole": 7, "part": 3 }))
        .expect_err("two branches have no single enum variant");
    match error {
        ProbeError::Mismatch { found, .. } => {
            assert!(found.contains("whole"), "{found}");
            assert!(found.contains("part"), "{found}");
        }
        other => panic!("expected a mismatch naming both members, got {other:?}"),
    }
}
