// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The one translation unit that answers every form and both byte
//! orders.
//!
//! `zerodds_wireprobe_cpp_gen::sources` bakes the direction, the emit
//! form and the byte order into the program, which is right for a probe
//! run once and wrong for `wireprobe-cpp-matrix`: the C column's
//! measurement is that the number of freshly linked binaries, and not the
//! compiler, is what a matrix sweep costs, so one program per case,
//! configuration and encoding replaces eight.
//!
//! Moving those three things out of the program is only sound if they were
//! never in the codec to begin with. That is what these tests are for:
//!
//! - the byte order has to be a runtime argument and nothing else, or the
//!   two orders would need two programs;
//! - every form has to be reachable from one `main`, or the collapse is
//!   not a collapse;
//! - and the program still has to compile under the warning set
//!   `runtime/cpp` holds its own sources to, which is what makes
//!   `-Wunused-function` a defect detector rather than noise.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wireprobe_cpp_gen::{Args, Emit, combined_sources, combined_sources_with};

fn repository_root() -> PathBuf {
    // `tools/wireprobe/wireprobe-zerodds-cpp-gen` is three below the root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root")
        .to_path_buf()
}

/// One corpus case, as [`zerodds_wireprobe_cpp_gen::Args`] reads it: the
/// directory the IDL and the sample are read from, and the root's scoped
/// name off the case's own `type` file.
///
/// Not [`zerodds_wire_probe::matrix::Case`], which is the shape the Rust
/// row is built from: nothing here plans a row, so the IDL text and the
/// sample would be read for nothing.
struct Corpus {
    directory: PathBuf,
    type_name: String,
}

fn case(name: &str) -> Corpus {
    let directory = repository_root()
        .join("tests")
        .join("interop-corpus")
        .join(name);
    let read = |file: &str| {
        std::fs::read_to_string(directory.join(file))
            .unwrap_or_else(|error| panic!("{name}/{file}: {error}"))
    };
    Corpus {
        type_name: read("type").trim().to_string(),
        directory,
    }
}

/// The generator's arguments for one case, configuration and byte order.
///
/// The configuration is named rather than passed, because
/// [`zerodds_wire_probe::matrix::Configuration`] is the list every column
/// is measured against and a test that carried its own copy of one entry
/// would be checking itself. Every axis that entry can carry is spelled
/// here; the byte order and the emit form are the only two the combined
/// unit ignores, and the first test below is what says so.
fn args_for(
    case: &Corpus,
    configuration: &str,
    encoding: Encoding,
    endianness: Endianness,
) -> Args {
    let settings = zerodds_wire_probe::matrix::CONFIGURATIONS
        .iter()
        .find(|entry| entry.name == configuration)
        .unwrap_or_else(|| panic!("`{configuration}` is not one of the twelve"));
    Args {
        idl: case.directory.join("type.idl"),
        type_name: case.type_name.clone(),
        values: Some(case.directory.join("values.json")),
        read: false,
        encoding,
        endianness,
        emit: Emit::Payload,
        dialect: settings.dialect,
        length_codes: settings.length_codes,
        wstring_length: settings.wstring_length,
        wstring_unit_order: settings.wstring_unit_order,
        pid_flags: settings.pid_flags,
        enum_holder: settings.enum_holder,
        union_label: settings.union_label,
        map_entries: settings.map_entries,
        enumerator_value: settings.enumerator_value,
        union_framing: settings.union_framing,
        short_pid_range: settings.short_pid_range,
        fixed_digits: settings.fixed_digits,
        map_length_with_delimiter: settings
            .map_length
            .map(|policy| policy == zerodds_wire_probe::value::MapLengthPolicy::SequenceEquivalent),
        ..Args::default()
    }
}

/// The cells every run below is taken over: one per dialect, both XCDR1
/// and XCDR2 in both framings.
const PLANS: [(&str, Encoding); 8] = [
    ("specification", Encoding::PlainCdr2),
    ("cyclone", Encoding::PlCdr2),
    ("rti", Encoding::PlCdr1),
    ("fastdds", Encoding::DelimitedCdr),
    ("omniorb", Encoding::PlainCdr1),
    ("tao", Encoding::PlainCdr2),
    ("jacorb", Encoding::PlCdr1),
    ("opendds", Encoding::PlainCdr2),
];

/// The byte order is the compiled program's argument, not a constant in
/// its source.
///
/// The claim, and the reason the collapse is sound: the layout plan does
/// not depend on the byte order — DDS-XTypes 1.3 puts member offsets in
/// the type and the order in the stream, and `LayoutKey`'s own
/// documentation says offsets are invariant under it — and
/// `runtime/cpp`'s `Writer` and `Reader` both take the order as a
/// constructor argument. So the two generations have to be **identical**;
/// a difference would mean the generator baked one order in somewhere, and
/// the second binary would be a lie.
///
/// And the order has to be *present*, or the identity above would be
/// trivial: a program that ignored its argument would satisfy this test
/// and write the wrong bytes. Both orders are named in the dispatch, and
/// both encapsulation identifiers are in the source, because the
/// identifier is the one thing about the order that is a constant.
#[test]
fn the_byte_order_is_a_runtime_argument() {
    // Two plans of two shapes: a keyed aggregate with a nested struct, a
    // sequence, an `@optional` member and an enumeration, and a `@mutable`
    // one under a parameter list. `combined-keyed-topic` also carries the
    // key holder, whose plan is big-endian whatever this argument says.
    let mut refused = 0usize;
    for name in ["combined-keyed-topic", "struct-mutable"] {
        let case = case(name);
        for (configuration, encoding) in PLANS {
            let little = combined_sources(&args_for(
                &case,
                configuration,
                encoding,
                Endianness::Little,
            ));
            let big = combined_sources(&args_for(&case, configuration, encoding, Endianness::Big));
            // A refusal is an answer and not a failure, and it is compared
            // as one: a cell refused under both orders said nothing about
            // the order, and a cell refused under exactly one of them is a
            // finding — the plan does not depend on the order, so neither
            // does the generation.
            match (little, big) {
                (Err(_), Err(_)) => refused += 1,
                (Err(only), Ok(_)) | (Ok(_), Err(only)) => panic!(
                    "{name} under `{configuration}` and {encoding:?}: one byte order \
                     generates and the other refuses ({only}); the layout plan does not \
                     depend on the byte order, so neither can the generation"
                ),
                (Ok(little), Ok(big)) => {
                    assert_eq!(
                        little.probe, big.probe,
                        "{name} under `{configuration}` and {encoding:?}: the byte order is \
                         in the source, so one program per plan cannot answer for both"
                    );
                    assert_eq!(
                        little.types, big.types,
                        "{name} under `{configuration}`: the declarations name the byte order"
                    );
                    assert!(
                        big.probe.contains("zerodds::wire::Endianness::Little")
                            && big.probe.contains("zerodds::wire::Endianness::Big"),
                        "{name} under `{configuration}`: the dispatch does not offer both orders"
                    );
                }
            }
        }
    }
    println!("{refused} cell(s) the generator refused under both byte orders");
}

/// One `main` reaches all four forms and both directions.
///
/// The three codec sets have to be in one file for the binary count to
/// fall: the payload encoder for `payload` and `payload+encapsulation`, the
/// key holder for `keyhash`, and the decoder for `read`. A form missing
/// from the dispatch is a slot the driver would report as unproduced with
/// no reason attached.
#[test]
fn the_combined_unit_answers_every_form_and_both_directions() {
    let case = case("combined-keyed-topic");
    let sources = combined_sources(&args_for(
        &case,
        "specification",
        Encoding::PlCdr2,
        Endianness::Little,
    ))
    .expect("a keyed type under the specification dialect");
    let probe = &sources.probe;
    for form in [
        "\"payload\"",
        "\"payload+encapsulation\"",
        "\"keyhash\"",
        "\"read\"",
    ] {
        assert!(
            probe.contains(form),
            "the dispatch does not name {form}:\n{probe}"
        );
    }
    for routine in [
        "zd_enc_",       // the payload, both of its forms
        "zd_keyholder_", // `keyhash`
        "zd_dec_",       // `read`
        "zdj_",          // the read direction's JSON, from `dumpjson.hpp`
    ] {
        assert!(
            probe.contains(routine),
            "the unit carries no `{routine}` routine:\n{probe}"
        );
    }
    // The payload behind its header is the same body as the bare payload
    // plus a four-octet prefix and the padding §7.6.3.1.2 counts, so both
    // forms go out of one function behind a flag rather than two.
    assert!(
        probe.contains("static int zd_write(zerodds::wire::Endianness order, bool encapsulate)"),
        "the write direction takes no encapsulation flag:\n{probe}"
    );
    assert!(
        probe.contains("zd_head[3] = static_cast<unsigned char>(zd_head[3] | zd_pad);"),
        "the encapsulation options field is never given the padding count \
         (§7.6.3.1.2):\n{probe}"
    );
}

/// The read direction is one program away.
///
/// The fallback the driver reaches for when a decoder does not compile:
/// one broken form must not take the payload with it. The retry therefore
/// has to carry no decoder and no `dumpjson.hpp`, and its `zd_read` has to
/// say so rather than answer with the sample it was never given.
#[test]
fn the_read_direction_can_be_left_out() {
    let case = case("combined-keyed-topic");
    let arguments = args_for(&case, "specification", Encoding::PlCdr2, Endianness::Little);
    let with_read = combined_sources(&arguments).expect("the full unit");
    let without = combined_sources_with(&arguments, false).expect("the unit without its reader");
    assert!(
        with_read.probe.contains("zd_dec_")
            && with_read.probe.contains("#include \"dumpjson.hpp\""),
        "the full unit carries no decoder"
    );
    assert!(
        !without.probe.contains("zd_dec_"),
        "the fallback still carries a decoder:\n{}",
        without.probe
    );
    assert!(
        !without.probe.contains("dumpjson.hpp"),
        "the fallback still carries the JSON dialect it does not use:\n{}",
        without.probe
    );
    assert!(
        without
            .probe
            .contains("this probe was built without the read direction"),
        "the fallback's `zd_read` does not refuse:\n{}",
        without.probe
    );
    // The three write forms are untouched by dropping the reader, which is
    // the whole point of the retry.
    assert!(
        without.probe.contains("zd_keyholder_") && without.probe.contains("zd_enc_"),
        "the fallback lost a write direction:\n{}",
        without.probe
    );
}

/// A type with no `@key` member still answers three of the four forms.
///
/// Not a hole in the column and not a compile error: such a type has one
/// instance and no KeyHash, and every column answers that the same way.
/// What it must not do is take the other three forms with it, so the
/// refusal is in the `keyhash` function and nowhere else.
#[test]
fn a_type_with_no_key_member_still_answers_the_other_three_forms() {
    let case = case("primitive-octet");
    let sources = combined_sources(&args_for(
        &case,
        "specification",
        Encoding::PlainCdr2,
        Endianness::Little,
    ))
    .expect("a keyless type");
    let probe = &sources.probe;
    assert!(
        probe.contains("declares no @key member"),
        "the keyless type does not say why it has no KeyHash:\n{probe}"
    );
    assert!(
        !probe.contains("zd_keyholder_"),
        "a keyless type grew a key holder:\n{probe}"
    );
    assert!(
        probe.contains("zd_enc_") && probe.contains("zd_dec_"),
        "the keyless type lost a direction:\n{probe}"
    );
}

/// One definition per name, in the cell where two plans write the same
/// function.
///
/// §7.6.8 Step 1 removes a nested aggregate's non-key members only "if
/// there are any key members", and RTI under XCDR1 reads the consequence as
/// "and below a keyless one, stop filtering at all" — so the holder's
/// subtree is written with the **data**-purpose encoders, which the
/// payload's own set already holds. `annotation-key-whole-struct` is the
/// one corpus case with that shape: `@key Inner nested` where `Inner` has
/// no `@key` member of its own.
///
/// Two definitions of one name do not compile, and one definition of one
/// plan would drop the other silently, so both are checked here: the
/// generation has to succeed, and every name has to be defined once.
#[test]
fn the_holder_and_the_payload_share_one_definition_of_each_name() {
    let case = case("annotation-key-whole-struct");
    let sources = combined_sources(&args_for(
        &case,
        "rti",
        // XCDR1 is what selects `KeyHolderNesting::KeylessNestedWhole`
        // (`Dialect::key_holder_nesting`), and not the configuration's
        // name.
        Encoding::PlainCdr1,
        Endianness::Little,
    ))
    .expect("the holder's subtree under the payload's own encoders");
    let probe = &sources.probe;
    assert!(
        probe.contains("zd_keyholder_") && probe.contains("zd_enc_"),
        "the cell is not the one this test is about:\n{probe}"
    );
    let definitions = definitions(probe);
    assert!(
        definitions.values().all(|count| *count == 1),
        "a name is defined more than once, which is either a redefinition \
         the compiler refuses or a plan that was dropped:\n{probe}"
    );
    // The shared subtree has to be there at all: an over-eager prune would
    // satisfy the count above by leaving the holder with nothing to call.
    assert!(
        definitions.keys().any(|name| name.contains("Inner")),
        "the holder's subtree is not in the unit:\n{probe}"
    );
    assert!(
        definitions.contains_key("zd_keyholder_Sample"),
        "the holder itself is not in the unit:\n{probe}"
    );
}

/// Every function defined in a generated unit, and how often.
///
/// A definition and not a declaration: a prototype is one name with no
/// body, and there are two of those per function by design — C++ resolves
/// a call to a free function by what is declared above it, which is what a
/// mutually recursive type needs.
fn definitions(probe: &str) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for line in probe.lines() {
        let line = line.trim();
        if !line.starts_with("static ") || !line.ends_with(") {") {
            continue;
        }
        let Some(open) = line.find('(') else { continue };
        let name = line[..open]
            .rsplit(|c: char| c.is_whitespace() || c == '*' || c == '&')
            .next()
            .unwrap_or_default()
            .to_string();
        assert!(!name.is_empty(), "a definition with no name: {line}");
        *out.entry(name).or_default() += 1;
    }
    out
}

// -- the program a compiler has to accept ----------------------------------

/// The warning set `runtime/cpp`'s Makefile holds its own four sources
/// and its own two tests to: `CXXSTD`, `OPT` and `WARNINGS` of
/// `runtime/cpp/Makefile`, and the same set `wireprobe-cpp-matrix` compiles
/// a probe with. `-Werror` is appended separately, below.
///
/// The conversions are the point. Every silent narrowing this column's C++
/// could commit is a byte on the wire nobody asked for, and `-Werror` is
/// what keeps a warning from becoming something nobody reads.
const FLAGS: [&str; 7] = [
    "-std=c++17",
    "-O1",
    "-Wall",
    "-Wextra",
    "-Wpedantic",
    "-Wconversion",
    "-Wsign-conversion",
];

/// A C++ compiler, or [`None`] and a loud skip.
fn compiler() -> Option<String> {
    if let Ok(chosen) = std::env::var("CXX") {
        return Some(chosen);
    }
    ["c++", "g++", "clang++"]
        .iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(|found| (*found).to_string())
}

/// The generated unit compiles, in the cells of the matrix this column
/// finds hardest.
///
/// `-fsyntax-only` and not a link: the harness's own text is what is under
/// test here, and what it would link against is `runtime/cpp`, whose own
/// `make check` compiles it with the very same flags. The cells are the
/// ones that turn an axis on — the holder's shared subtree under RTI and
/// XCDR1, a keyless type, a `@mutable` parameter list, a `@mutable` union,
/// a `map`, a bounded `wstring` whose code units are above U+00FF, an
/// `@external` aggregate, a keyed aggregate and a delimited form.
#[test]
fn the_combined_unit_compiles() {
    let Some(cxx) = compiler() else {
        eprintln!(
            "skip the_combined_unit_compiles: no C++ compiler on PATH, and the generated \
             harness is what this checks"
        );
        return;
    };
    let root = repository_root();
    let directory = std::env::temp_dir().join(format!(
        "zd-combined-unit-{}-{}",
        std::process::id(),
        env!("CARGO_PKG_NAME")
    ));
    // Only ever this process's own directory, which is named after its
    // process id.
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the work directory");

    let cells: [(&str, &str, Encoding); 10] = [
        // The one cell where the key holder's subtree is written with the
        // payload's own encoders, so the unit has to hold one definition of
        // each name and not two.
        ("annotation-key-whole-struct", "rti", Encoding::PlainCdr1),
        ("annotation-key-whole-struct", "rti", Encoding::PlCdr1),
        // A type with no `@key` member: the KeyHash form refuses at run
        // time and the other three must not notice.
        ("primitive-octet", "specification", Encoding::PlainCdr2),
        ("struct-mutable", "cyclone", Encoding::PlCdr1),
        ("union-mutable", "rti", Encoding::PlCdr1),
        ("map-key-string", "omniorb", Encoding::PlainCdr2),
        ("wstring-bounded-utf8", "opendds", Encoding::DelimitedCdr),
        (
            "annotation-external-struct",
            "specification",
            Encoding::PlCdr2,
        ),
        ("combined-keyed-topic", "fastdds", Encoding::PlainCdr2),
        ("primitive-octet", "specification", Encoding::DelimitedCdr),
    ];

    for (name, configuration, encoding) in cells {
        let case = case(name);
        let sources = combined_sources(&args_for(
            &case,
            configuration,
            encoding,
            Endianness::Little,
        ))
        .unwrap_or_else(|error| panic!("{name}/{configuration}: {error}"));
        std::fs::write(directory.join("zd_types.hpp"), &sources.types).expect("the declarations");
        std::fs::write(directory.join("probe.cpp"), &sources.probe).expect("the probe");
        let compiled = Command::new(&cxx)
            .args(FLAGS)
            .arg("-Werror")
            .arg("-fsyntax-only")
            // `dumpjson.hpp` lives beside the probe wrappers: the JSON
            // dialect is a shared contract and not a wire rule.
            .arg(format!("-I{}", directory.display()))
            .arg(format!(
                "-I{}",
                root.join("tools").join("wireprobe").display()
            ))
            .arg(format!(
                "-I{}",
                root.join("runtime").join("cpp").join("include").display()
            ))
            .arg(directory.join("probe.cpp"))
            .output()
            .expect("invoke the C++ compiler");
        assert!(
            compiled.status.success(),
            "{name} under `{configuration}` and {encoding:?} does not compile:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
}
