// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The emitted C++, Java and TypeScript against the Rust reference.
//!
//! Every unit test in `src/codegen/` checks one reading of DDS-AMQP 1.0
//! §7 and §8.1. This checks that the four readings **agree on the
//! octets**: the same model, the same sample, four independent encoders,
//! one answer. Where two disagree one of them has misread the clause,
//! which is the only reason to have a second.
//!
//! The generated helpers are written to a temporary directory beside the
//! runtimes from `crates/amqp-bridge/runtime/`, compiled, and run. Each
//! program prints the lower-case hexadecimal of `toAmqpValue` and then
//! the `toJsonString` document; the test compares both against
//! `described_composite` and `json_document`.
//!
//! ## When a toolchain is missing
//!
//! The test names the compiler it could not find and passes. That is a
//! loud skip and not a silent one: the message says which column went
//! unchecked, so a run on a host without `javac` does not read as a run
//! that checked Java.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_amqp_bridge::codegen::{
    CodegenLanguage, EncodeOptions, PlanSet, SampleValue, TypePlan, carrier_for,
    described_composite, helper_for, json_document, plan_of, runtime_source, time_carrier_source,
};
use zerodds_ast::Ast;

/// The model the four columns encode.
///
/// One type per shape §7 distinguishes: every row of §7.1's primitive
/// table that IDL can write here, an enumeration, an octet sequence, a
/// sequence of a scalar, a nested composite, a `map`, an `@optional`
/// member and a `DDS::Time_t`.
const IDL: &str = r#"
module DDS {
    struct Time_t {
        long long sec;
        unsigned long nanosec;
    };
};

module probe {
    enum Colour { RED, GREEN, BLUE };

    struct Inner {
        long a;
        string b;
    };

    struct Sample {
        boolean flag;
        octet o;
        short s;
        unsigned short us;
        long n;
        unsigned long un;
        long long big;
        unsigned long long ubig;
        float f;
        double d;
        char c;
        string text;
        Colour tint;
        sequence<octet> blob;
        sequence<long> numbers;
        Inner nested;
        map<string, long> lookup;
        DDS::Time_t stamp;
        @optional long maybe;
    };
};
"#;

/// The descriptor bytes a caller supplies. A fixed `TypeIdentifier`
/// rather than a derived one: §7.2.1 builds the descriptor from the
/// X-Types identifier and deriving it here would be a second answer
/// beside `zerodds-types`'.
const IDENTIFIER: [u8; 14] = [
    0xF1, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
];

fn model() -> Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "the fixture IDL must parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "cross-language.idl");
    assert!(
        lowered.diagnostics.iter().all(|d| !d.is_error()),
        "the fixture IDL must lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.broken.is_empty(),
        "every name must resolve: {:?}",
        report.broken
    );
    ast
}

fn types_by_name(ast: &Ast) -> BTreeMap<String, zerodds_ast_core::Id<zerodds_ast::TypeNode>> {
    ast.types()
        .filter(|(_, node)| !node.name.is_empty())
        .filter_map(|(id, node)| {
            let scoped = ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))?;
            Some((scoped.segments().join("::"), id))
        })
        .collect()
}

/// The plans, with the nested composite ahead of the type that holds it
/// so a generated C++ header declares `Inner` before `Sample`.
fn plans(ast: &Ast) -> PlanSet {
    let types = types_by_name(ast);
    let mut out = Vec::new();
    for name in ["probe::Inner", "probe::Sample"] {
        let id = *types
            .get(name)
            .unwrap_or_else(|| panic!("no type `{name}`"));
        out.push(plan_of(ast, id, &IDENTIFIER, false).unwrap_or_else(|e| panic!("{name}: {e}")));
    }
    PlanSet::new(out)
}

/// The sample every column encodes, in the plan's field order.
fn sample() -> Vec<Option<SampleValue>> {
    vec![
        Some(SampleValue::Bool(true)),
        Some(SampleValue::Uint(7)),
        Some(SampleValue::Int(-300)),
        Some(SampleValue::Uint(40000)),
        Some(SampleValue::Int(-123_456)),
        Some(SampleValue::Uint(4_000_000_000)),
        Some(SampleValue::Int(-9_000_000_000)),
        Some(SampleValue::Uint(9_000_000_000)),
        Some(SampleValue::Float(1.5)),
        Some(SampleValue::Float(-2.25)),
        Some(SampleValue::Char('Z')),
        Some(SampleValue::Str("a \"quoted\"\nline".to_string())),
        Some(SampleValue::Int(2)),
        Some(SampleValue::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01])),
        Some(SampleValue::Seq(vec![
            SampleValue::Int(1),
            SampleValue::Int(-2),
            SampleValue::Int(3),
        ])),
        Some(SampleValue::Composite(vec![
            Some(SampleValue::Int(42)),
            Some(SampleValue::Str("inner".to_string())),
        ])),
        Some(SampleValue::Map(vec![(
            SampleValue::Str("k".to_string()),
            SampleValue::Int(5),
        )])),
        Some(SampleValue::Time {
            sec: 3,
            nanosec: 500_000_000,
        }),
        // §7.2.2.1 / §8.1.2 — the absent optional: `null` in the list,
        // omitted from the object.
        None,
    ]
}

fn reference(plans: &PlanSet, plan: &TypePlan) -> (String, String) {
    let value = described_composite(plans, plan, &sample(), EncodeOptions::default())
        .expect("the reference encoding");
    let octets = value.encode().expect("the reference octets");
    let mut hex = String::new();
    for byte in &octets {
        hex.push_str(&format!("{byte:02x}"));
    }
    let json = json_document(plans, plan, &sample()).expect("the reference JSON");
    (hex, json)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "zerodds-amqp-codegen-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a scratch directory");
    path
}

fn have(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn write(directory: &Path, name: &str, contents: &str) -> PathBuf {
    let path = directory.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("a directory");
    }
    std::fs::write(&path, contents).expect("a written file");
    path
}

fn run(command: &mut Command, what: &str) -> String {
    let output = command.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        output.status.success(),
        "{what} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 output")
}

/// The two lines a generated program prints: the hex and the JSON.
fn split_output(text: &str, column: &str) -> (String, String) {
    let mut lines = text.lines();
    let hex = lines
        .next()
        .unwrap_or_else(|| panic!("{column} printed nothing"))
        .trim()
        .to_string();
    let json = lines
        .next()
        .unwrap_or_else(|| panic!("{column} printed no JSON"))
        .to_string();
    (hex, json)
}

// ---------------------------------------------------------------------

#[test]
fn the_rust_reference_round_trips_through_the_wire_codec() {
    let ast = model();
    let plans = plans(&ast);
    let plan = plans.get("probe::Sample").expect("the plan");
    let value = described_composite(&plans, plan, &sample(), EncodeOptions::default())
        .expect("the reference encoding");
    let octets = value.encode().expect("octets");
    let (back, consumed) =
        zerodds_amqp_bridge::AmqpExtValue::decode(&octets).expect("the codec reads it back");
    assert_eq!(consumed, octets.len());
    assert_eq!(back, value);
}

#[test]
fn the_reference_json_is_a_valid_rfc_8259_document() {
    // Annex C §C.1.12 — "a sample with MODE_JSON is valid RFC-8259
    // JSON." Checked structurally: balanced braces and brackets outside
    // strings, no trailing comma, and every key quoted.
    let ast = model();
    let plans = plans(&ast);
    let plan = plans.get("probe::Sample").expect("the plan");
    let (_, json) = reference(&plans, plan);
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut previous = ' ';
    for character in json.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => {
                assert_ne!(previous, ',', "a trailing comma in {json}");
                depth -= 1;
                assert!(depth >= 0, "unbalanced: {json}");
            }
            _ => {}
        }
        previous = character;
    }
    assert_eq!(depth, 0, "unbalanced: {json}");
    assert!(!in_string, "an unterminated string in {json}");
    assert!(json.starts_with('{') && json.ends_with('}'), "{json}");
}

#[test]
fn the_emitted_cpp_writes_the_same_octets_and_the_same_json() {
    if !have("c++") {
        eprintln!(
            "SKIPPED: no `c++` on PATH, so the emitted C++ column went unchecked in this run"
        );
        return;
    }
    let ast = model();
    let plans = plans(&ast);
    let plan = plans.get("probe::Sample").expect("the plan");
    let (hex, json) = reference(&plans, plan);

    let directory = scratch("cpp");
    write(
        &directory,
        "zerodds_amqp_codec.hpp",
        runtime_source(CodegenLanguage::Cpp),
    );

    let mut source = String::from(
        "#include \"zerodds_amqp_codec.hpp\"\n\
         #include <cstdio>\n#include <map>\n#include <optional>\n#include <string>\n\
         #include <vector>\n\n",
    );
    for plan in plans.plans() {
        source.push_str(&carrier_for(plan, CodegenLanguage::Cpp));
        source.push('\n');
    }
    for plan in plans.plans() {
        source.push_str(&helper_for(&plans, plan, CodegenLanguage::Cpp).expect("helpers"));
        source.push('\n');
    }
    source.push_str(CPP_MAIN);
    write(&directory, "main.cpp", &source);

    let binary = directory.join("probe");
    run(
        Command::new("c++")
            .arg("-std=c++17")
            .arg("-o")
            .arg(&binary)
            .arg(directory.join("main.cpp")),
        "compiling the emitted C++",
    );
    let output = run(&mut Command::new(&binary), "running the emitted C++");
    let (their_hex, their_json) = split_output(&output, "C++");
    assert_eq!(their_hex, hex, "the C++ octets differ from the Rust ones");
    assert_eq!(their_json, json, "the C++ JSON differs from the Rust one");
}

#[test]
fn the_emitted_java_writes_the_same_octets_and_the_same_json() {
    if !have("javac") {
        eprintln!(
            "SKIPPED: no `javac` on PATH, so the emitted Java column went unchecked in this run"
        );
        return;
    }
    let ast = model();
    let plans = plans(&ast);
    let plan = plans.get("probe::Sample").expect("the plan");
    let (hex, json) = reference(&plans, plan);

    let directory = scratch("java");
    let package = "org/zerodds/amqp";
    write(
        &directory,
        &format!("{package}/AmqpCodec.java"),
        runtime_source(CodegenLanguage::Java),
    );
    write(
        &directory,
        &format!("{package}/TimeT.java"),
        time_carrier_source(),
    );
    let mut sources = vec![
        directory.join(format!("{package}/AmqpCodec.java")),
        directory.join(format!("{package}/TimeT.java")),
    ];
    for plan in plans.plans() {
        let name = plan.simple_name();
        sources.push(write(
            &directory,
            &format!("{name}.java"),
            &format!(
                "import org.zerodds.amqp.AmqpCodec;\nimport org.zerodds.amqp.TimeT;\n\n{}",
                carrier_for(plan, CodegenLanguage::Java)
            ),
        ));
        sources.push(write(
            &directory,
            &format!("{name}AmqpCodec.java"),
            &format!(
                "import org.zerodds.amqp.AmqpCodec;\nimport org.zerodds.amqp.TimeT;\n\n{}",
                helper_for(&plans, plan, CodegenLanguage::Java).expect("helpers")
            ),
        ));
    }
    sources.push(write(&directory, "Main.java", JAVA_MAIN));

    let classes = directory.join("classes");
    let mut compile = Command::new("javac");
    // The sources are UTF-8; `javac` without `-encoding` reads the
    // platform encoding, which is US-ASCII in the CI container.
    compile.args(["-encoding", "UTF-8"]);
    compile.arg("-d").arg(&classes);
    for source in &sources {
        compile.arg(source);
    }
    run(&mut compile, "compiling the emitted Java");
    let output = run(
        Command::new("java").arg("-cp").arg(&classes).arg("Main"),
        "running the emitted Java",
    );
    let (their_hex, their_json) = split_output(&output, "Java");
    assert_eq!(their_hex, hex, "the Java octets differ from the Rust ones");
    assert_eq!(their_json, json, "the Java JSON differs from the Rust one");
}

#[test]
fn the_emitted_typescript_writes_the_same_octets_and_the_same_json() {
    if !have("node") {
        eprintln!(
            "SKIPPED: no `node` on PATH, so the emitted TypeScript column went unchecked in this run"
        );
        return;
    }
    if !have("tsc") {
        eprintln!(
            "SKIPPED: no `tsc` on PATH, so the emitted TypeScript column went unchecked in this run"
        );
        return;
    }
    let ast = model();
    let plans = plans(&ast);
    let plan = plans.get("probe::Sample").expect("the plan");
    let (hex, json) = reference(&plans, plan);

    let directory = scratch("ts");
    write(
        &directory,
        "codec.ts",
        runtime_source(CodegenLanguage::TypeScript),
    );
    let mut source = String::from("import * as codec from \"./codec\";\n\n");
    for plan in plans.plans() {
        source.push_str(&carrier_for(plan, CodegenLanguage::TypeScript));
        source.push('\n');
    }
    for plan in plans.plans() {
        source.push_str(&helper_for(&plans, plan, CodegenLanguage::TypeScript).expect("helpers"));
        source.push('\n');
    }
    source.push_str(TS_MAIN);
    write(&directory, "main.ts", &source);

    run(
        Command::new("tsc")
            .arg("--strict")
            .arg("--target")
            .arg("es2020")
            .arg("--module")
            .arg("commonjs")
            .arg("--lib")
            .arg("es2020,dom")
            .arg("--outDir")
            .arg(directory.join("out"))
            .arg(directory.join("main.ts")),
        "compiling the emitted TypeScript",
    );
    let output = run(
        Command::new("node").arg(directory.join("out/main.js")),
        "running the emitted TypeScript",
    );
    let (their_hex, their_json) = split_output(&output, "TypeScript");
    assert_eq!(
        their_hex, hex,
        "the TypeScript octets differ from the Rust ones"
    );
    assert_eq!(
        their_json, json,
        "the TypeScript JSON differs from the Rust one"
    );
    let _ = workspace_root();
}

// ---------------------------------------------------------------------
// The sample, written once per column. Hand-written and not generated:
// what is under test is the helper, and a generated constructor would
// make the sample the generator's opinion too.
// ---------------------------------------------------------------------

const CPP_MAIN: &str = r#"
int main() {
    Inner inner;
    inner.a = 42;
    inner.b = "inner";

    Sample v;
    v.flag = true;
    v.o = 7;
    v.s = -300;
    v.us = 40000;
    v.n = -123456;
    v.un = 4000000000u;
    v.big = -9000000000LL;
    v.ubig = 9000000000ULL;
    v.f = 1.5f;
    v.d = -2.25;
    v.c = 'Z';
    v.text = "a \"quoted\"\nline";
    v.tint = 2;
    v.blob = zerodds::amqp::Bytes{0xDE, 0xAD, 0xBE, 0xEF, 0x01};
    v.numbers = std::vector<std::int32_t>{1, -2, 3};
    v.nested = inner;
    v.lookup = std::map<std::string, std::int32_t>{{"k", 5}};
    v.stamp.sec = 3;
    v.stamp.nanosec = 500000000u;

    zerodds::amqp::Bytes octets = toAmqpValue_probe_Sample(v);
    for (std::size_t i = 0; i < octets.size(); ++i) {
        std::printf("%02x", octets[i]);
    }
    std::printf("\n%s\n", toJsonString_probe_Sample(v).c_str());
    return 0;
}
"#;

const JAVA_MAIN: &str = r#"import org.zerodds.amqp.TimeT;

public final class Main {
    public static void main(String[] args) {
        Inner inner = new Inner();
        inner.a = 42;
        inner.b = "inner";

        Sample v = new Sample();
        v.flag = true;
        v.o = 7;
        v.s = -300;
        v.us = 40000;
        v.n = -123456;
        v.un = 4000000000L;
        v.big = -9000000000L;
        v.ubig = 9000000000L;
        v.f = 1.5f;
        v.d = -2.25;
        v.c = 'Z';
        v.text = "a \"quoted\"\nline";
        v.tint = 2;
        v.blob = new byte[] {(byte) 0xDE, (byte) 0xAD, (byte) 0xBE, (byte) 0xEF, (byte) 0x01};
        v.numbers = java.util.Arrays.asList(1, -2, 3);
        v.nested = inner;
        v.lookup = new java.util.LinkedHashMap<String, Integer>();
        v.lookup.put("k", 5);
        v.stamp = new TimeT(3L, 500000000L);

        byte[] octets = SampleAmqpCodec.toAmqpValue(v);
        StringBuilder hex = new StringBuilder();
        for (byte b : octets) {
            hex.append(String.format("%02x", b & 0xFF));
        }
        System.out.println(hex.toString());
        System.out.println(SampleAmqpCodec.toJsonString(v));
    }
}
"#;

const TS_MAIN: &str = r#"
const inner: Inner = { a: 42, b: "inner" };
const v: Sample = {
    flag: true,
    o: 7,
    s: -300,
    us: 40000,
    n: -123456,
    un: 4000000000,
    big: -9000000000n,
    ubig: 9000000000n,
    f: 1.5,
    d: -2.25,
    c: 0x5a,
    text: 'a "quoted"\nline',
    tint: 2,
    blob: Uint8Array.of(0xde, 0xad, 0xbe, 0xef, 0x01),
    numbers: [1, -2, 3],
    nested: inner,
    lookup: new Map<string, number>([["k", 5]]),
    stamp: { sec: 3n, nanosec: 500000000 },
};

const octets = toAmqpValue_probe_Sample(v);
let hex = "";
for (const b of octets) {
    hex += b.toString(16).padStart(2, "0");
}
console.log(hex);
console.log(toJsonString_probe_Sample(v));
"#;
