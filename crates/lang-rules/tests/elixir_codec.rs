// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Elixir *codec* through `elixirc`, both directions.
//!
//! `elixir_end_to_end.rs` holds the materialiser to the compiler; this
//! file holds the shared codec walk's Elixir backend to it. The two
//! gates ask different questions: a snapshot says the emitter produced
//! the text it produced last time, and only `elixirc` says the text is
//! Elixir after every macro has expanded — that every `%Struct{}` a
//! defaulter writes names declared fields, that every generated call
//! resolves, and that the functional lowering's accumulator tuples
//! bind what they read.
//!
//! **Warnings are failures here**, exactly as in the materialiser's
//! gate: an unused variable, an unused import and a clause that can
//! never match are each a defect in what this column generated. The
//! wire runtime is compiled once and put on the code path, because a
//! call to a module `elixirc` cannot find is itself a warning.
//!
//! The byte-level truth is not here — it is the probe's, measured with
//! `wireprobe-compare` against the Rust column's recorded octets. This
//! gate is what runs where Elixir is installed and no corpus is.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stderr
)]

use std::process::Command;
use std::sync::OnceLock;
use zerodds_ast::Ast;
use zerodds_ast_layout::key::LayoutKey;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_lang_rules::adapter::elixir::elixir_file;
use zerodds_lang_rules::default_elixir::elixir_defaulters_called_by;
use zerodds_lang_rules::emit::decode::decoder_from;
use zerodds_lang_rules::emit::elixir::Elixir;
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from};
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, one model out.
fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");
    ast
}

/// The whole generated file for one root under one key: declarations,
/// then the codec module — the same assembly the probe generator
/// makes, minus the harness, which is the generator's own and carries
/// no walk decision.
fn generate(idl: &str, root_name: &str, key: LayoutKey, reading: bool) -> String {
    let ast = model(idl);
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == root_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::elixir::catalogue()).expect("the rules apply");
    let declarations = elixir_file(&ast, &applied).expect("the materialiser answers");

    let codec = if reading {
        let decoders =
            decoder_from::<Elixir>(&ast, root, key, CodecOptions::default()).expect("a decoder");
        let mut all = elixir_defaulters_called_by(&ast, &decoders).expect("Table 9 answers");
        all.extend(decoders);
        all
    } else {
        codec_from::<Elixir>(&ast, root, key, CodecOptions::default()).expect("an encoder")
    };

    let mut module = String::new();
    for function in &codec {
        module.push('\n');
        for line in zerodds_lang_render::elixir::codec_function(function).lines() {
            if line.is_empty() {
                module.push('\n');
            } else {
                module.push_str("  ");
                module.push_str(line);
                module.push('\n');
            }
        }
    }

    let mut out = zerodds_lang_render::elixir::file(&declarations);
    out.push_str("\ndefmodule ZdCodec do\n  @moduledoc false\n");
    if ["&&&", "|||", ">>>", "<<<"]
        .iter()
        .any(|operator| module.contains(operator))
    {
        out.push_str("  import Bitwise\n");
    }
    out.push_str(&module);
    out.push_str("end\n");
    out
}

/// Whether `elixirc` is on the path at all.
fn elixirc_available() -> bool {
    Command::new("elixirc")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The compiled wire runtime, once per test process.
///
/// On the code path rather than left out: a call into a module the
/// compiler cannot find is a warning, and warnings are failures.
fn runtime_ebin() -> &'static str {
    static EBIN: OnceLock<String> = OnceLock::new();
    EBIN.get_or_init(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let runtime = root.join("runtime/elixir/lib");
        let out = std::env::temp_dir().join(format!("zd-elixir-rt-{}", std::process::id()));
        std::fs::create_dir_all(&out).expect("the runtime ebin directory");
        let mut sources = vec![runtime.join("zerodds_wire.ex")];
        for entry in std::fs::read_dir(runtime.join("zerodds_wire")).expect("the runtime modules") {
            sources.push(entry.expect("a runtime module").path());
        }
        let output = Command::new("elixirc")
            .arg("--ignore-module-conflict")
            .arg("-o")
            .arg(&out)
            .args(&sources)
            .output()
            .expect("elixirc runs");
        // Both streams, and the version. `elixirc` puts a compile
        // error on standard output as readily as on standard error, and
        // printing only the latter produced exactly this in CI:
        //
        //     the wire runtime does not compile:
        //
        // with nothing after the colon — a failure that names no cause,
        // on a machine nobody is sitting at. The version is here for the
        // same reason: this runtime compiles on 1.20.2 and on 1.18.3,
        // and the CI image carries what Debian bookworm ships.
        assert!(
            output.status.success(),
            "the wire runtime does not compile under {}:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(
                &Command::new("elixir")
                    .arg("--version")
                    .output()
                    .map(|v| v.stdout)
                    .unwrap_or_default()
            )
            .trim(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        out.display().to_string()
    })
}

/// One generated file through `elixirc`, the runtime on the path.
/// Anything on standard error is a failure, whatever the exit status.
fn compiles(source: &str) -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!(
        "zd-elixir-codec-{}-{stamp}-{}",
        std::process::id(),
        source.len()
    ));
    let out = dir.join("ebin");
    std::fs::create_dir_all(&out).map_err(|error| error.to_string())?;
    let file = dir.join("zd_codec.ex");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new("elixirc")
        .arg("--ignore-module-conflict")
        .arg("-pa")
        .arg(runtime_ebin())
        .arg("-o")
        .arg(&out)
        .arg(&file)
        .output()
        .map_err(|error| error.to_string());
    let result = match output {
        Err(why) => Err(why),
        Ok(output) => {
            // Both streams, for the reason `elixir_end_to_end.rs`'s
            // `elixir_compiles` carries: `elixirc` writes to either, and
            // reading one of them turns a complaint into a pass.
            let said = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if output.status.success() && only_accepted_warnings(&said) {
                Ok(())
            } else {
                Err(said)
            }
        }
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Whether everything on standard error is a warning this gate accepts.
///
/// One class is, and it is named rather than shrugged at: Elixir 1.18's
/// type checker flow-types a literal, so an `@appendable` aggregate's
/// tail latch — `zd_tail = false` followed by the *first* member's
/// `if not zd_tail and …` — is reported as a conditional that "will
/// always succeed". It will: the shared walk emits one uniform guard
/// per member so that no member's read is a special case, and the first
/// guard's latch half is trivially true by construction. That is a fact
/// about the generated shape, the same shape every column emits and no
/// other compiler in the fleet flow-types; suppressing the *shape*
/// would un-unify the members, which is the larger defect. Every other
/// warning — unused variables, unresolved modules, clauses that never
/// match — still fails the gate.
fn only_accepted_warnings(stderr: &str) -> bool {
    if stderr.trim().is_empty() {
        return true;
    }
    let mut segments = stderr.split("warning:");
    // Text before the first warning marker has to be blank; anything
    // else is a diagnostic of another kind.
    if segments
        .next()
        .is_some_and(|preamble| !preamble.trim().is_empty())
    {
        return false;
    }
    let mut any = false;
    for segment in segments {
        any = true;
        if !segment.contains("will always succeed") {
            return false;
        }
    }
    any
}

/// The shapes both directions are held to: every construct the nine
/// measured cases exercise, plus the ones whose lowering is its own —
/// the `@mutable` loops with their breaks, the map's two-pass count,
/// the recursive type, the keyword members.
const SOURCES: &[(&str, &str)] = &[
    (
        "@final struct Sample { boolean flag; octet o; char c; wchar w; short s; \
         unsigned short us; long l; unsigned long ul; long long ll; \
         unsigned long long ull; float f; double d; };",
        "Sample",
    ),
    (
        "@final struct Sample { string narrow; wstring wide; string<8> bounded; \
         fixed<5,2> money; long double big; };",
        "Sample",
    ),
    (
        "@final struct Sample { sequence<long> ids; sequence<string, 3> names; \
         long grid[2][3]; map<boolean, long> entries; map<string, long> table; };",
        "Sample",
    ),
    (
        "enum Status { @value(1) OK, @value(100) WARN, @value(65535) FAIL };
         @final struct Sample { Status a; Status b; };",
        "Sample",
    ),
    (
        "enum Colour { RED, GREEN, BLUE };
         union Choice switch (Colour) { case RED: long whole; \
         case GREEN: case BLUE: double fraction; };
         @final struct Sample { Choice picked; long tail; };",
        "Sample",
    ),
    (
        "union CharKeyed switch (char) { case 'a': long one; default: string other; };
         @final struct Sample { CharKeyed keyed; };",
        "Sample",
    ),
    (
        "@mutable struct Evo { @id(1) long a; @id(2) long b; \
         @id(3) @must_understand long c; };",
        "Evo",
    ),
    (
        "@appendable struct Sample { long a; @optional long b; long c; };",
        "Sample",
    ),
    (
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };
         bitset Bits { bitfield<3> low; bitfield<12> high; };
         @final struct Sample { Flags f; Bits b; };",
        "Sample",
    ),
    ("@final struct Node { sequence<Node> children; };", "Node"),
    (
        "typedef long Distance; @final struct Sample { Distance d; long end; long When; };",
        "Sample",
    ),
    ("@final struct Empty { };", "Empty"),
];

/// The two keys the gate compiles under: the two encoding versions
/// pull different framing arms out of the walk — PL_CDR1 headers and
/// the sentinel under version 1, DHEADER and EMHEADER under version 2.
fn keys() -> [LayoutKey; 2] {
    [
        LayoutKey::data(Encoding::PlainCdr1, Endianness::Big),
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    ]
}

#[test]
fn the_same_input_generates_the_same_text_twice_in_one_process() {
    // The temporaries are numbered, and a counter that ran on past the
    // end of a routine would number the second generation differently:
    // one generator process emitting two types, or a corpus run, would
    // produce text that differs from the text a single run produced.
    // Needs no `elixirc` and runs everywhere.
    for key in keys() {
        for (idl, root) in SOURCES {
            for reading in [false, true] {
                let first = generate(idl, root, key, reading);
                let second = generate(idl, root, key, reading);
                assert_eq!(
                    first, second,
                    "the second generation differs, reading={reading}, {key:?}\n{idl}"
                );
            }
        }
    }
}

#[test]
fn the_compiler_check_catches_a_deliberate_error() {
    // Before the gates below are trusted: a call into a module that is
    // not on the path is the warning class this gate turns on, and it
    // exits 0.
    if !elixirc_available() {
        eprintln!("SKIPPED: `elixirc` is not on the path.");
        return;
    }
    let broken = "defmodule ZdProbeCodec do\n  def go(w), do: ZdNoSuchModule.write(w)\nend\n";
    let why = compiles(broken).expect_err("an unresolved module did not fail the check");
    assert!(why.contains("ZdNoSuchModule"), "{why}");
}

#[test]
fn every_shape_encodes_and_the_generated_elixir_compiles_without_warnings() {
    if !elixirc_available() {
        eprintln!(
            "SKIPPED: `elixirc` is not on the path, so the generated Elixir codec was not \
             compiled. Set it up or run this on a host that has it."
        );
        return;
    }
    for key in keys() {
        for (idl, root) in SOURCES {
            let generated = generate(idl, root, key, false);
            if let Err(why) = compiles(&generated) {
                panic!("write, {key:?}\n{idl}\n\n{generated}\n\nelixirc said:\n{why}");
            }
        }
    }
}

#[test]
fn every_shape_decodes_and_the_generated_elixir_compiles_without_warnings() {
    if !elixirc_available() {
        eprintln!(
            "SKIPPED: `elixirc` is not on the path, so the generated Elixir decoder was not \
             compiled. Set it up or run this on a host that has it."
        );
        return;
    }
    for key in keys() {
        for (idl, root) in SOURCES {
            let generated = generate(idl, root, key, true);
            if let Err(why) = compiles(&generated) {
                panic!("read, {key:?}\n{idl}\n\n{generated}\n\nelixirc said:\n{why}");
            }
        }
    }
}

/// One generated file loaded into `elixir`, the runtime on the code
/// path, and `script` run after it; what the script printed.
fn runs(source: &str, script: &str) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!(
        "zd-elixir-run-{}-{}",
        std::process::id(),
        source.len()
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let file = dir.join("zd_codec.ex");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new("elixir")
        .arg("-pa")
        .arg(runtime_ebin())
        .arg("-r")
        .arg(&file)
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|error| error.to_string());
    let _ = std::fs::remove_dir_all(&dir);
    let output = output?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// A decoded array is the tuple its member declares, and so is Table 9's.
///
/// The materialiser declares `long grid[2][3]` as `{{integer(), …}, …}`
/// with a tuple `defstruct` default; the reader answered with lists, and
/// a sample read back was not `==` to the one written (measured: a
/// `long w[3]` came back `%A{w: [1, 2, 3]}`).
#[test]
fn a_decoded_array_is_the_tuple_its_member_declares() {
    if !elixirc_available() {
        eprintln!("SKIPPED: `elixirc` is not on the path, so the decoded array was not run.");
        return;
    }
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let source = generate("@final struct G { long grid[2][3]; };", "G", key, true);
    assert!(
        source.contains("ZeroddsWire.array_to_lists(v, 2)")
            && source.contains("ZeroddsWire.array_to_tuples(v, 2)"),
        "the rows are not turned into lists and back:\n{source}"
    );
    let printed = runs(
        &source,
        "r = ZeroddsWire.Reader.new(<<1::little-32, 2::little-32, 3::little-32, \
         4::little-32, 5::little-32, 6::little-32>>, :le, 4)\n\
         {v, _} = ZdCodec.zd_dec_G(r, :discard)\n\
         IO.puts(v == %G{grid: {{1, 2, 3}, {4, 5, 6}}})\n\
         IO.puts(ZdCodec.zd_def_G() == %G{})\n",
    )
    .unwrap_or_else(|why| panic!("{source}\n\n{why}"));
    assert_eq!(
        printed.trim(),
        "true\ntrue",
        "a decoded or a default array is not the declared tuple:\n{source}"
    );
}
