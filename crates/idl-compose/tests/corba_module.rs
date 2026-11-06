// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OMG IDL 4.2 §7.4.6.4.7, the CORBA module, with the clause's own
//! example: `CORBA::Object` is an error because `Object` is a keyword,
//! `TypeCode` is undefined unless written `CORBA::TypeCode`, and
//! `orb.idl` — or, for `CORBA::TypeCode`, `TypeCode.idl` — brings the
//! definitions of the module.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl_compose::{ComposeOptions, compose_source_with};
use zerodds_idl_parse::MemoryLoader;

/// The error diagnostics of composing `source` with `loader`.
fn errors(source: &str, loader: &MemoryLoader) -> Vec<String> {
    compose_source_with("corba.idl", source, loader, ComposeOptions::default())
        .diagnostics
        .into_iter()
        .filter(|d| d.is_error())
        .map(|d| format!("{} {}", d.code.as_str(), d.message))
        .collect()
}

#[test]
fn idl_7_4_6_4_7_the_corba_module_comes_from_orb_idl_and_typecode_idl() {
    let none = MemoryLoader::new();
    // The clause's example, line by line.
    let ok = errors(
        "#include <orb.idl>\nmodule M { typedef CORBA::TypeCode TypeCode; };",
        &none,
    );
    assert!(ok.is_empty(), "{ok:?}");
    let scoped_object = errors(
        "#include <orb.idl>\nmodule M { typedef CORBA::Object myObjRef; };",
        &none,
    );
    assert!(
        !scoped_object.is_empty(),
        "`CORBA::Object` scopes a keyword"
    );
    let bare_typecode = errors(
        "#include <orb.idl>\nmodule M { typedef TypeCode myTypeCode; };",
        &none,
    );
    assert!(
        !bare_typecode.is_empty(),
        "`TypeCode` is undefined unscoped"
    );

    // TypeCode.idl is enough for CORBA::TypeCode.
    let through_typecode_idl = errors(
        "#include <TypeCode.idl>\nstruct S { CORBA::TypeCode tc; };",
        &none,
    );
    assert!(through_typecode_idl.is_empty(), "{through_typecode_idl:?}");

    // Other names of the module need orb.idl, and are there with it.
    let without = errors("struct S { CORBA::Policy p; };", &none);
    assert!(!without.is_empty(), "no CORBA module without orb.idl");
    let with = errors(
        "#include <orb.idl>\ninterface I { CORBA::PolicyList policies(); \
         void fail() raises (CORBA::PolicyError); };",
        &none,
    );
    assert!(with.is_empty(), "{with:?}");

    // A caller's own orb.idl is read instead of the built-in one.
    let own = MemoryLoader::new().with("orb.idl", "module CORBA { typedef long Mine; };");
    assert!(errors("#include <orb.idl>\ntypedef CORBA::Mine M;", &own).is_empty());
    assert!(!errors("#include <orb.idl>\ntypedef CORBA::TypeCode T;", &own).is_empty());
}
