// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Generates Python modules from representative IDL into a stable directory so
//! the Python-side pytest (`crates/py/python/tests/test_idl.py`) can import the
//! ACTUAL codegen output and run real encode→decode roundtrips. This closes the
//! loop the generation-only smoke tests cannot: it proves the emitted module
//! imports and roundtrips against the live runtime (Bugs M / R5 / Q-cluster).
//!
//! The full `zerodds-idlc` binary cannot be built here (sibling backend crates
//! are mid-refactor), so we drive the Python backend directly.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_python::{PythonGenOptions, generate_python_module};

fn emit_py(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_python_module(&ast, &PythonGenOptions::default()).expect("gen")
}

fn out_dir() -> PathBuf {
    // crates/idl-python/tests/ -> crates/py/python/tests/_generated/
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = here
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let dir = root.join("crates/py/python/tests/_generated");
    std::fs::create_dir_all(&dir).expect("mkdir _generated");
    dir
}

fn write_module(name: &str, src: &str) {
    let py = emit_py(src);
    let path = out_dir().join(format!("{name}.py"));
    std::fs::write(&path, py).expect("write generated module");
}

/// Emit the modules pytest consumes. This is a test (not a build script) so it
/// runs under `cargo test -p zerodds-idl-python` and stays in lockstep with the
/// emitter under test.
#[test]
fn generate_python_modules_for_pytest() {
    // Bug R5 (#64): @optional members.
    write_module(
        "optionals_gen",
        "module conf { struct Optionals { \
            long required; @optional long maybe; @optional string note; \
        }; };",
    );

    // Bug M (#56): const declarations.
    write_module(
        "consts_gen",
        "const long MAX_ITEMS = 10; \
         const double RATE = 2.5; \
         const boolean ENABLED = TRUE; \
         module conf { const long N = 4; };",
    );

    // Bug Q-cluster (#60): module flattening + fixed array + char/wstring +
    // union-as-member.
    write_module(
        "combo_gen",
        "module combo { \
            enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT }; \
            typedef double CurrentInAmpsType; \
            struct Sample { long seq; double value; }; \
            union Reading switch (Mode) { \
                case MODE_IDLE: long idleTicks; \
                case MODE_ACTIVE: double activeRate; \
                default: string faultCode; \
            }; \
            struct Telemetry { \
                long unitId; \
                Mode mode; \
                CurrentInAmpsType batteryCurrent; \
                sequence<Sample> history; \
                Reading reading; \
                @optional double calibration; \
                long window[4]; \
            }; \
        };",
    );

    // Char / WString brands standalone.
    write_module("chars_gen", "struct Chars { char c; wstring w; };");

    // Fixed multidim array roundtrip.
    write_module(
        "arrays_gen",
        "module conf { \
            struct Point { long x; long y; }; \
            struct Arrays { long grid[2][2]; Point shape[2]; }; \
        };",
    );

    // Adversarial edge sweep — the cases that break adapters past hello-world.
    // BOUND enforcement: bounded sequence / string / wstring / map must emit
    // the runtime brand that checks the bound (over-N -> error, not corruption).
    write_module(
        "bounds_gen",
        "module b { struct Bounded { \
            sequence<long, 3> nums; \
            string<5> name; \
            wstring<3> wname; \
            map<string, long, 2> counters; \
            sequence<long> unbounded; \
            string fullstr; \
        }; };",
    );

    // DEEP nesting: struct->struct->struct (3 levels); sequence<sequence<struct>>;
    // map<string, struct-with-a-sequence>.
    write_module(
        "deepnest_gen",
        "module d { \
            struct L3 { long v; }; \
            struct L2 { L3 inner; }; \
            struct L1 { L2 inner; }; \
            struct HasSeq { sequence<long> xs; }; \
            struct Deep { \
                L1 chain; \
                sequence<sequence<L3>> grid; \
                map<string, HasSeq> table; \
            }; \
        };",
    );

    // @optional of AGGREGATE types: optional nested struct, optional sequence,
    // optional map, optional string — present AND absent must both roundtrip.
    write_module(
        "optagg_gen",
        "module o { \
            struct Inner { long v; }; \
            struct OptAgg { \
                @optional Inner nested; \
                @optional sequence<long> nums; \
                @optional map<string, long> table; \
                @optional string note; \
            }; \
        };",
    );

    // UNICODE: CJK + emoji through both string and wstring.
    write_module("unicode_gen", "struct Uni { string s; wstring w; };");

    // EXTREME primitives: full-width integer min/max/0/-1 across all widths.
    write_module(
        "extremes_gen",
        "struct Extremes { \
            int8 i8; uint8 u8; \
            int16 i16; uint16 u16; \
            int32 i32; uint32 u32; \
            int64 i64; uint64 u64; \
            float f; double dbl; \
        };",
    );

    // KEYED: a @key field plus a payload — same key, different payload.
    write_module(
        "keyed_gen",
        "struct Keyed { @key long id; string label; long long payload; };",
    );

    // XCDR1 wire path (F.1 item 13): the generated `encode(representation=0)` /
    // `decode(..., representation=0)` path had no coverage. Three extensibility
    // modes each hit a distinct XCDR1 rule:
    //   * @final    — alignment cap 8 (double aligns to 8, not XCDR2's 4).
    //   * @appendable — NO DHEADER on the wire (unlike XCDR2 rule(30)).
    //   * @mutable  — PL_CDR1 [PID][len] members + PID_LIST_END sentinel.
    write_module(
        "xcdr1_gen",
        "@final struct X1Final { octet a; double d; }; \
         @appendable struct X1App { long a; double d; }; \
         @mutable struct X1Mut { @id(1) long a; @id(2) double d; };",
    );
}
