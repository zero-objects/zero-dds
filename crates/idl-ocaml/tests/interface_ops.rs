// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OCaml backend: interface operations/attributes (#11). The backend previously
//! dropped every interface `Export::Op`/`Export::Attr`; it now emits native
//! `<Iface>_Client` / `<Iface>_Handler` module-type signatures mirroring the
//! idl-ts / idl-rust / idl-swift interface surface. Operations carry no wire
//! form (there is no OCaml `zerodds-rpc` runtime — the honest boundary is the
//! typed signature), so these tests are string-level plus an `ocamlfind`
//! signature-compile (gated on the OCaml toolchain).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ocaml::{OcamlGenOptions, generate_ocaml_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ocaml_module(&ast, &OcamlGenOptions::default()).expect("gen")
}

#[test]
fn interface_emits_client_and_handler_signatures() {
    let s = emit("interface Calc { long add(in long a, in long b); void reset(); };");
    assert!(s.contains("module type Calc_Client = sig"), "{s}");
    assert!(s.contains("module type Calc_Handler = sig"), "{s}");
    assert!(s.contains("val add : int -> int -> int"), "{s}");
    assert!(s.contains("val reset : unit -> unit"), "{s}");
}

#[test]
fn out_and_inout_params_fold_into_return_tuple() {
    let s = emit("interface Svc { long compute(in long x, out long hi, inout long acc); };");
    // in + inout become curried arguments; the return folds result + out + inout.
    assert!(
        s.contains("val compute : int -> int -> (int * int * int)"),
        "{s}"
    );
}

#[test]
fn single_out_param_void_return_is_bare_type() {
    let s = emit("interface Q { void peek(out long v); };");
    assert!(s.contains("val peek : unit -> int"), "{s}");
}

#[test]
fn attributes_emit_get_and_conditional_set() {
    let s =
        emit("interface Sensor { readonly attribute long temperature; attribute string name; };");
    assert!(s.contains("val get_temperature : unit -> int"), "{s}");
    assert!(
        !s.contains("val set_temperature"),
        "readonly must have no setter:\n{s}"
    );
    assert!(s.contains("val get_name : unit -> string"), "{s}");
    assert!(s.contains("val set_name : string -> unit"), "{s}");
}

#[test]
fn interface_bases_become_module_type_inclusion() {
    let s = emit("interface Base { void ping(); }; interface Derived : Base { long value(); };");
    assert!(s.contains("module type Derived_Client = sig"), "{s}");
    assert!(s.contains("  include Base_Client"), "{s}");
    assert!(s.contains("module type Derived_Handler = sig"), "{s}");
    assert!(s.contains("  include Base_Handler"), "{s}");
}

#[test]
fn interface_nested_type_param_resolves_to_promoted_name() {
    // #A39: the nested struct is promoted to the scope-encoded `Reg_sEntry`; a
    // param referencing it must resolve to that same flattened module's `.t`.
    let s = emit("interface Reg { struct Entry { long id; }; void put(in Entry e); };");
    assert!(s.contains("module Reg_sEntry = struct"), "{s}");
    assert!(s.contains("val put : Reg_sEntry.t -> unit"), "{s}");
}

/// The emitted module-type signatures must be valid OCaml (compile-only gate).
#[test]
fn interface_signatures_compile() {
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP interface_signatures_compile: `ocamlfind` not on PATH");
        return;
    }
    let src = emit(
        "interface Base { void ping(); };\n\
         interface Calc : Base {\n\
           struct Entry { long id; };\n\
           long add(in long a, in long b);\n\
           void reset();\n\
           readonly attribute long temp;\n\
           attribute string name;\n\
           long compute(in long x, out long hi, inout long acc);\n\
           void put(in Entry e);\n\
         };",
    );
    let dir = std::env::temp_dir().join(format!("idlocaml_iface_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "-c", "main.ml"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
