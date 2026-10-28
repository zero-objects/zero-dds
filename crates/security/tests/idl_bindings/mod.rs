// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! Renders the language representations of `dds_security.idl` (DDS-Security
//! 1.2 clause 11): C (§11.3), C++ classic (§11.4), Java classic (§11.5),
//! C++11 (§11.6) and Java modern (§11.7). Used by the test
//! `dds_security_bindings.rs` and by the example `dds_security_bindings`.

#![allow(dead_code)]

pub mod c;
pub mod cpp;
pub mod java;
pub mod model;
pub mod resolve;

use std::collections::BTreeMap;

pub const IDL: &str = include_str!("../../idl/dds_security.idl");

/// Relative path to text, for all five representations.
pub fn render_all() -> BTreeMap<String, String> {
    let decls = model::parse(IDL);
    let flat = resolve::flat_of(&decls);
    let table = resolve::Table::new(&flat);
    let mut out = BTreeMap::new();
    out.insert("c/dds_security.h".into(), c::render(&flat, &table));
    out.insert(
        "cpp/dds_security.hpp".into(),
        cpp::render(&flat, &table, cpp::Dialect::Classic),
    );
    out.insert(
        "cpp11/dds_security.hpp".into(),
        cpp::render(&flat, &table, cpp::Dialect::Cxx11),
    );
    for f in java::render(&flat, &table, java::Dialect::Classic) {
        out.insert(format!("java/{}", f.path), f.text);
    }
    for f in java::render(&flat, &table, java::Dialect::Modern) {
        out.insert(format!("java-modern/{}", f.path), f.text);
    }
    out
}
