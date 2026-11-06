// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-interop` with `zerodds-lang-render`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-render` (`build.rs`, a build
//! dependency): `rust::file`, which writes the data types and their codecs
//! as the first half of `corba_gen.rs`; `build.rs` merges the modules of
//! that text with those `zerodds-corba-rust` writes for the interfaces, by
//! name. The crate relies on each IDL module arriving as one `pub mod` of
//! that name, holding the data types the interfaces of the same module use.
//!
//! Oracle: IDL 4.2 §7.5.2 (`CosNaming::NameComponent` is the scoped name
//! of the type in the module `CosNaming`) and the Rust reference
//! ("Modules"), checked by the compiler building this target: a type and an
//! interface of one IDL module are reached under one Rust module.
#![allow(clippy::unwrap_used, dead_code, unused_imports)]

use std::sync::Arc;

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));

use corba_gen::CosNaming::{NameComponent, NamingContextStub};

#[test]
fn idl_7_5_2_a_data_type_and_an_interface_of_one_module_share_its_rust_module() {
    let component = NameComponent {
        id: "Echo".into(),
        kind: String::new(),
    };
    assert_eq!(component.id, "Echo");
    // The stub of the same IDL module resolves under the same path.
    let _named: fn() -> Option<NamingContextStub> = || None;
}
