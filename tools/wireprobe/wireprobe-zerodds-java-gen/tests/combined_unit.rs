// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The one program that answers every form and both byte orders.
//!
//! `zerodds_wireprobe_java_gen::sources` bakes the direction, the emit
//! form and the byte order into the program, which is right for a probe
//! run once and wrong for `wireprobe-java-matrix`: a JVM launch costs far
//! more than a C process launch, and that driver measures the whole
//! matrix, so one program per case, configuration and encoding replaces
//! seven.
//!
//! Moving those three things out of the program is only sound if they were
//! never in the codec to begin with. That is what these tests are for:
//!
//! - the byte order has to be a runtime argument and nothing else, or the
//!   two orders would need two programs;
//! - every form has to be reachable from one `main`, or the collapse is
//!   not a collapse;
//! - the program still has to compile under the warning set
//!   `runtime/java/run-tests.sh` holds its own sources to, which is what
//!   makes `-Xlint:all -Werror` a defect detector rather than noise;
//! - and one definition per method signature, or the file does not compile
//!   at all where two codec sets overlap.
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
use zerodds_wireprobe_java_gen::{
    Args, Emit, answer_stem, combined_sources, combined_sources_with,
};

fn repository_root() -> PathBuf {
    // `tools/wireprobe/wireprobe-zerodds-java-gen` is three below the root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root")
        .to_path_buf()
}

/// One corpus case, as [`Args`] reads it: the directory the IDL and the
/// sample are read from, and the root's scoped name off the case's own
/// `type` file.
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
/// `runtime/java`'s `Writer` and `Reader` both take the order as a
/// constructor argument. So the two generations have to be **identical**;
/// a difference would mean the generator baked one order in somewhere, and
/// the second program would be a lie.
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
                        big.probe.contains("Endianness.LITTLE")
                            && big.probe.contains("Endianness.BIG"),
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
/// The three codec sets have to be in one file for the launch count to
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
    for form in ["\"batch\"", "\"read\""] {
        assert!(
            probe.contains(form),
            "the dispatch does not name {form}:\n{probe}"
        );
    }
    for routine in [
        "zd_enc_",       // the payload, both of its forms
        "zd_keyholder_", // `keyhash`
        "zd_dec_",       // `read`
        "Zdj.",          // the read direction's JSON, from `Zdj.java`
    ] {
        assert!(
            probe.contains(routine),
            "the unit carries no `{routine}` routine:\n{probe}"
        );
    }
    // The payload behind its header is the same body as the bare payload
    // plus a four-octet prefix and the padding §7.6.3.1.2 counts, so both
    // forms go out of one method behind a flag rather than two.
    assert!(
        probe.contains("private static byte[] zd_payload(Endianness order, boolean encapsulate)"),
        "the write direction takes no encapsulation flag:\n{probe}"
    );
    assert!(
        probe.contains("Encapsulation.padding(body.length)")
            && probe.contains("Encapsulation.header(Encoding."),
        "the encapsulation options field is never given the padding count \
         (§7.6.3.1.2):\n{probe}"
    );
}

/// The five octet answers are written where the batch driver looks.
///
/// One rule, in one place: the slot's own name with `/` made into `-`,
/// because `/` is a path separator and the read slots carry one. The
/// generator writes the stems and the driver reads them, so the rule is
/// [`answer_stem`] and both sides call it — and this test says the harness
/// really does spell out what that function would give.
#[test]
fn the_batch_run_writes_each_answer_where_the_driver_looks_for_it() {
    let case = case("combined-keyed-topic");
    let probe = &combined_sources(&args_for(
        &case,
        "specification",
        Encoding::PlCdr2,
        Endianness::Little,
    ))
    .expect("a keyed type under the specification dialect")
    .probe;
    for slot in ["le", "be", "le+enc", "be+enc", "keyhash"] {
        assert!(
            probe.contains(&format!("zd_answer(dir, \"{}\"", answer_stem(slot))),
            "the batch run writes no file for `{slot}`:\n{probe}"
        );
    }
    assert_eq!(
        answer_stem("le/read"),
        "le-read",
        "the read slot's name has a `/` in it and the rule makes it a file name"
    );
}

/// The read direction is one program run away.
///
/// The fallback the driver reaches for when a decoder does not compile:
/// one broken form must not take the payload with it. The retry therefore
/// has to carry no decoder and no JSON dialect, and its `zd_read` has to
/// say so rather than answer with a sample it was never given.
#[test]
fn the_read_direction_can_be_left_out() {
    let case = case("combined-keyed-topic");
    let arguments = args_for(&case, "specification", Encoding::PlCdr2, Endianness::Little);
    let with_read = combined_sources(&arguments).expect("the full unit");
    let without = combined_sources_with(&arguments, false).expect("the unit without its reader");
    assert!(
        with_read.probe.contains("zd_dec_") && with_read.probe.contains("Zdj."),
        "the full unit carries no decoder"
    );
    assert!(
        !without.probe.contains("zd_dec_"),
        "the fallback still carries a decoder:\n{}",
        without.probe
    );
    assert!(
        !without.probe.contains("Zdj."),
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

/// A type with no `@key` member still answers the other six slots.
///
/// Not a hole in the column and not a compile error: such a type has one
/// instance and no KeyHash, and every column answers that the same way.
/// What it must not do is take the other forms with it, so the refusal is
/// in the `keyhash` answer and nowhere else.
#[test]
fn a_type_with_no_key_member_still_answers_the_other_forms() {
    let case = case("primitive-octet");
    let probe = &combined_sources(&args_for(
        &case,
        "specification",
        Encoding::PlainCdr2,
        Endianness::Little,
    ))
    .expect("a keyless type")
    .probe;
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

/// One definition per signature, in the cell where two plans write the
/// same method.
///
/// §7.6.8 Step 1 removes a nested aggregate's non-key members only "if
/// there are any key members", and RTI under XCDR1 reads the consequence as
/// "and below a keyless one, stop filtering at all" — so the holder's
/// subtree is written with the **data**-purpose encoders, which the
/// payload's own set already holds. `annotation-key-whole-struct` is the
/// one corpus case with that shape: `@key Inner nested` where `Inner` has
/// no `@key` member of its own.
///
/// Two methods of one signature do not compile, and one definition of one
/// plan would drop the other silently, so both are checked here: the
/// generation has to succeed, and every signature has to be defined once.
#[test]
fn the_holder_and_the_payload_share_one_definition_of_each_signature() {
    let case = case("annotation-key-whole-struct");
    let probe = &combined_sources(&args_for(
        &case,
        "rti",
        // XCDR1 is what selects `KeyHolderNesting::KeylessNestedWhole`
        // (`Dialect::key_holder_nesting`), and not the configuration's
        // name.
        Encoding::PlainCdr1,
        Endianness::Little,
    ))
    .expect("the holder's subtree under the payload's own encoders")
    .probe;
    assert!(
        probe.contains("zd_keyholder_") && probe.contains("zd_enc_"),
        "the cell is not the one this test is about:\n{probe}"
    );
    let definitions = definitions(probe);
    assert!(
        definitions.values().all(|count| *count == 1),
        "a signature is defined more than once, which is either a redefinition \
         javac refuses or a plan that was dropped:\n{probe}"
    );
    // The shared subtree has to be there at all: an over-eager prune would
    // satisfy the count above by leaving the holder with nothing to call.
    assert!(
        definitions.keys().any(|name| name.contains("Inner")),
        "the holder's subtree is not in the unit:\n{probe}"
    );
    assert!(
        definitions
            .keys()
            .any(|name| name.contains("zd_keyholder_")),
        "the holder itself is not in the unit:\n{probe}"
    );
}

/// Every method signature defined in a generated unit, and how often.
///
/// A definition and not a declaration, and keyed by everything up to and
/// including the closing parenthesis: name, parameter types and return
/// type. The name alone is not a key, because a Java method is
/// **overloaded** — `zd_enc_int(Writer, int)` and `zd_enc_int(Writer,
/// long)` are two methods and both belong in the file.
///
/// The scan is over the whole probe flattened to one line, because a
/// signature the renderer wrapped across lines is still one signature and a
/// line-by-line scan would count neither copy of it.
fn definitions(probe: &str) -> BTreeMap<String, usize> {
    let flat = without_comments(probe)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    let mut from = 0usize;
    while let Some(found) = flat[from..].find(')') {
        let close = from + found;
        // The statement or block before it: a definition starts where the
        // previous one ended.
        let head = &flat[..close];
        let start = head.rfind([';', '{', '}']).map_or(0, |at| at + 1);
        let signature = flat[start..=close].trim();
        // A definition opens with a modifier and not with the expression
        // that calls one: `static` on its own is a field, not a method.
        let first = signature
            .trim_start_matches("@Override")
            .split([' ', '('])
            .next()
            .unwrap_or_default();
        // The five `ZdAnswer` bodies are one method each in its own
        // anonymous class, so five definitions of `run` is not a
        // redefinition and is not what this counts.
        if matches!(first, "public" | "private" | "protected" | "static")
            && !signature.ends_with("run()")
        {
            *out.entry(signature.to_string()).or_default() += 1;
        }
        from = close + 1;
    }
    out
}

/// The probe without its comments.
///
/// A definition's signature is what follows the previous statement or
/// block, and a Javadoc between the two would open it with `/**` instead
/// of a modifier: the key holder's comment hid `zd_keyholder_Sample` from
/// [`definitions`] on the Linux test host. The generated Java carries no `//` or `/*`
/// inside a string literal, so a plain scan is enough.
fn without_comments(probe: &str) -> String {
    let mut out = String::with_capacity(probe.len());
    let mut rest = probe;
    loop {
        let block = rest.find("/*");
        let line = rest.find("//");
        let block_first = match (block, line) {
            (None, None) => {
                out.push_str(rest);
                return out;
            }
            (Some(b), Some(l)) => b < l,
            (Some(_), None) => true,
            (None, Some(_)) => false,
        };
        if block_first {
            let at = block.unwrap_or_default();
            out.push_str(&rest[..at]);
            match rest[at + 2..].find("*/") {
                Some(end) => rest = &rest[at + 2 + end + 2..],
                None => return out,
            }
        } else {
            let at = line.unwrap_or_default();
            out.push_str(&rest[..at]);
            match rest[at..].find('\n') {
                Some(end) => rest = &rest[at + end..],
                None => return out,
            }
        }
    }
}

// -- the program a compiler has to accept ----------------------------------

/// The warning set `runtime/java/run-tests.sh` holds its own sixteen
/// sources and its own three tests to, and the same set
/// `wireprobe-java-matrix` compiles a probe with.
///
/// `--release 8` because the runtime targets Java 8 and nothing here needs
/// more; `-options` is out of the lint set because javac 21 warns that
/// release 8 is obsolete, which is a fact about the compiler and not about
/// this code; and `-encoding UTF-8` because the generated sources carry `§`
/// in their comments and `javac` without it reads the platform encoding —
/// US-ASCII in a container, and then every one of them is an error.
///
/// The conversions are the point. Every silent narrowing this column's
/// Java could commit is a byte on the wire nobody asked for, and `-Werror`
/// is what keeps a warning from becoming something nobody reads.
const FLAGS: [&str; 6] = [
    "--release",
    "8",
    "-Xlint:all,-options",
    "-Werror",
    "-encoding",
    "UTF-8",
];

/// `Zdj.java` cannot be compiled for release 8 — it builds its
/// `PrintStream` with the `(OutputStream, boolean, Charset)` constructor,
/// which arrived in Java 10 — so it gets the same set without
/// `--release`, which is what `wireprobe-zerodds-java` does with it.
const ZDJ_FLAGS: [&str; 4] = ["-Xlint:all,-options", "-Werror", "-encoding", "UTF-8"];

/// A Java compiler, or [`None`] and a loud skip.
fn compiler() -> Option<String> {
    if let Ok(chosen) = std::env::var("JAVAC") {
        return Some(chosen);
    }
    ["javac"]
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
/// The cells are the ones that turn an axis on — a keyless type, a
/// `@mutable` parameter list, a `@mutable` union, a `map`, a bounded
/// `wstring` whose code units are above U+00FF, an `@external` aggregate, a
/// keyed aggregate and a delimited form — and `runtime/java` and
/// `Zdj.java` are compiled once beside them, under the same flags, because
/// a probe is compiled against their classes and not against their
/// sources.
#[test]
fn the_combined_unit_compiles() {
    let Some(javac) = compiler() else {
        eprintln!(
            "skip the_combined_unit_compiles: no Java compiler on PATH, and the generated \
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

    let runtime = compile_runtime(&javac, &root, &directory.join("runtime"));

    let cells: [(&str, &str, Encoding); 12] = [
        // A type with no `@key` member: the KeyHash answer refuses at run
        // time and the other six must not notice.
        ("primitive-octet", "specification", Encoding::PlainCdr2),
        // The one shape that puts the key holder's subtree under the
        // payload's own encoders, so the unit has to hold one definition of
        // each signature and not two.
        (
            "annotation-key-whole-struct",
            "cyclone",
            Encoding::PlainCdr1,
        ),
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
        // The three cells that used to be pinned as this column's one
        // compile gap: `--dialect rti` chooses the PL_CDR1 parameter
        // header per sample, and the walk declared that header's `Pid`
        // handle with the storage a Java local the body writes *into*
        // needs — `new Writer.Pid()`, against a runtime constructor that
        // is package-private. The walk declares it instead, assigned in
        // both arms of the choice before it is read, and these compile.
        ("combined-keyed-topic", "rti", Encoding::PlainCdr1),
        ("struct-mutable", "rti", Encoding::PlCdr1),
        ("evolution-mutable-union", "rti", Encoding::PlainCdr1),
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
        let here = directory.join("cell");
        std::fs::create_dir_all(&here).expect("the cell's directory");
        std::fs::write(here.join("ZdTypes.java"), &sources.types).expect("the declarations");
        std::fs::write(here.join("Probe.java"), &sources.probe).expect("the probe");
        let compiled = Command::new(&javac)
            .args(FLAGS)
            .arg("-cp")
            .arg(&runtime)
            .arg("-d")
            .arg(&here)
            .arg(here.join("ZdTypes.java"))
            .arg(here.join("Probe.java"))
            .output()
            .expect("invoke the Java compiler");
        assert!(
            compiled.status.success(),
            "{name} under `{configuration}` and {encoding:?} does not compile:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
}

/// `runtime/java`'s wire package, or wherever `ZERODDS_JAVA_RUNTIME` says
/// it is.
///
/// `org/zerodds/wire` and nothing else, for the reason
/// `tools/wireprobe/wireprobe-zerodds-java` gives: the tree also carries
/// `org/omg/dds/rpc`, six of whose files import a package that lives in
/// another repository behind its own `pom.xml`.
fn wire_package(root: &Path) -> PathBuf {
    std::env::var_os("ZERODDS_JAVA_RUNTIME")
        .map_or_else(
            || {
                root.join("runtime")
                    .join("java")
                    .join("src")
                    .join("main")
                    .join("java")
            },
            PathBuf::from,
        )
        .join("org")
        .join("zerodds")
        .join("wire")
}

/// `runtime/java`'s own sources and `Zdj.java`, compiled once into
/// `classes`, under the two flag sets this file holds.
///
/// A probe is compiled against these classes and not against their
/// sources, so this is what `wireprobe-java-matrix` does before it
/// compiles a single unit.
fn compile_runtime(javac: &str, root: &Path, classes: &Path) -> PathBuf {
    std::fs::create_dir_all(classes).expect("the runtime's class directory");
    let mut sources: Vec<PathBuf> = std::fs::read_dir(wire_package(root))
        .expect("runtime/java's wire package")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "java")
        })
        .collect();
    sources.sort();
    assert!(!sources.is_empty(), "runtime/java has no wire sources");
    let compiled = Command::new(javac)
        .args(FLAGS)
        .arg("-d")
        .arg(classes)
        .args(&sources)
        .output()
        .expect("invoke the Java compiler");
    assert!(
        compiled.status.success(),
        "runtime/java does not compile:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let zdj = root.join("tools").join("wireprobe").join("Zdj.java");
    let compiled = Command::new(javac)
        .args(ZDJ_FLAGS)
        .arg("-cp")
        .arg(classes)
        .arg("-d")
        .arg(classes)
        .arg(&zdj)
        .output()
        .expect("invoke the Java compiler");
    assert!(
        compiled.status.success(),
        "tools/wireprobe/Zdj.java does not compile:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    classes.to_path_buf()
}
