// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java mapping of DDS-Java-PSM 1.0 Clause 9, beside the IDL-to-Java
//! mapping.
//!
//! §9.1.1: "DDS aggregation types shall be mapped to a final Java class.
//! Contained attributes shall be encapsulated. Java Bean style accessors
//! shall be provided. [...] The representation of internal state shall be
//! private." §9.1.2: "Unbounded DDS sequences are mapped to Collection<E>
//! interface. The state is encapsulated and getters/setters are provided
//! through bean style property accessors. Bounded sequences and arrays are
//! mapped to Java arrays."
//!
//! The expectations below are those two clauses, written out for one IDL
//! file; the generated text is checked for what the clauses say and for
//! what must not have moved under the default mapping.
//!
//! `print_stderr` is allowed because a skipped compiler check has to be
//! said, and standard error is the channel a passing test does not
//! otherwise use.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU32, Ordering};

use zerodds_ast::Ast;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;
use zerodds_lang_rules::adapter::interface::ServiceMapping;
use zerodds_lang_rules::adapter::java::{JavaMapping, mapping, with_mapping};

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;
#[path = "support/corpus.rs"]
mod corpus_harness;

const IDL: &str = "
module demo {
  enum Color { RED, GREEN };
  struct Point { long x; long y; @optional long z; boolean on; };
  union Shape switch (long) {
    case 1: Point p;
    case 2: string name;
    default: long other;
  };
  struct Track {
    @key string id;
    Point center;
    Point vicinity[2];
    sequence<long> samples;
    sequence<long, 4> bounded;
    sequence<Point> points;
    sequence<sequence<string> > names;
    sequence<long> rows[3];
    map<string, long> table;
    Color tint;
    Shape form;
    @optional string note;
  };
};
";

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

fn key() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
}

/// The generated Java text, types and codec, under `selected`.
fn generated(idl: &str, selected: JavaMapping) -> Result<String, String> {
    let ast = model(idl);
    with_mapping(selected, || {
        zerodds_lang_rules::generate::source(&ast, Language::Java, key())
            .map_err(|error| error.to_string())
    })
}

fn psm() -> String {
    generated(IDL, JavaMapping::Psm).expect("the Psm mapping generates")
}

fn omg() -> String {
    generated(IDL, JavaMapping::Omg).expect("the Omg mapping generates")
}

#[test]
fn psm_is_the_default_and_the_option_is_scoped() {
    // `idlc` and a library caller that names no mapping get Clause 9.
    assert_eq!(JavaMapping::default(), JavaMapping::Psm);
    assert_eq!(mapping(), JavaMapping::Psm);
    with_mapping(JavaMapping::Omg, || {
        assert_eq!(mapping(), JavaMapping::Omg);
        // Nested scopes restore what they replaced.
        with_mapping(JavaMapping::Psm, || assert_eq!(mapping(), JavaMapping::Psm));
        assert_eq!(mapping(), JavaMapping::Omg);
    });
    assert_eq!(mapping(), JavaMapping::Psm);
    // Also when the body panics.
    let result = std::panic::catch_unwind(|| {
        with_mapping(JavaMapping::Omg, || panic!("inside"));
    });
    assert!(result.is_err());
    assert_eq!(mapping(), JavaMapping::Psm);
    assert_eq!(JavaMapping::from_name("psm"), Some(JavaMapping::Psm));
    assert_eq!(JavaMapping::from_name("omg"), Some(JavaMapping::Omg));
    assert_eq!(JavaMapping::from_name("jacorb"), None);
    // Without a named mapping a library caller gets the text of Clause 9.
    assert_eq!(
        psm(),
        zerodds_lang_rules::generate::source(&model(IDL), Language::Java, key())
            .expect("generates")
    );
}

#[test]
fn the_idl_to_java_mapping_is_what_it_was() {
    let text = omg();
    for expected in [
        "public int x;",
        "public java.lang.Integer z;",
        "public boolean on;",
        "public int[] samples;",
        "public int[] bounded;",
        "public ZdTypes.demo.Point[] points;",
        "public java.lang.String[][] names;",
        "zd_enc_p_long(w, v.x);",
        "w.writeU32(v.length);",
        "v.x = ",
    ] {
        assert!(text.contains(expected), "missing `{expected}`");
    }
    for forbidden in ["getX", "setX", "Collection<", "Sequences."] {
        assert!(
            !text.contains(forbidden),
            "`{forbidden}` in the IDL-to-Java mapping"
        );
    }
}

/// §9.1.1.
#[test]
fn javapsm_9_1_1_an_aggregate_is_a_final_class_with_private_state_and_bean_accessors() {
    let text = psm();
    for expected in [
        "public static final class Point {",
        "private int x;",
        "private int y;",
        "private java.lang.Integer z;",
        "public Point() {",
        "public int getX() {",
        "public void setX(int v) {",
        "public java.lang.Integer getZ() {",
        "public void setZ(java.lang.Integer v) {",
        // The special rule for boolean properties §9.1.1 allows.
        "public boolean isOn() {",
        "public boolean getOn() {",
        "public void setOn(boolean v) {",
        "public static final class Track {",
        "private java.lang.String id;",
    ] {
        assert!(text.contains(expected), "missing `{expected}`");
    }
    assert!(!text.contains("public int x;"));
    assert!(!text.contains("public java.lang.String id;"));
}

/// §9.1.2.
#[test]
fn javapsm_9_1_2_an_unbounded_sequence_is_a_collection_and_a_bounded_one_an_array() {
    let text = psm();
    for expected in [
        "private java.util.Collection<java.lang.Integer> samples;",
        "private java.util.Collection<ZdTypes.demo.Point> points;",
        "private java.util.Collection<java.util.Collection<java.lang.String>> names;",
        "public java.util.Collection<java.lang.Integer> getSamples() {",
        "public void setSamples(java.util.Collection<java.lang.Integer> v) {",
        // Bounded sequences and arrays: Java arrays.
        "private int[] bounded;",
        "private ZdTypes.demo.Point[] vicinity;",
        // An array of unbounded sequences: an array of Collection<E>.
        "private java.util.Collection<java.lang.Integer>[] rows;",
        // A map is a java.util.Map (XTypes 1.3 §7.5.1.3.5), not a sequence.
        "private ZdTypes.ZdMap_string_long table;",
    ] {
        assert!(text.contains(expected), "missing `{expected}`");
    }
}

#[test]
fn javapsm_9_1_1_a_union_keeps_its_discriminator_as_the_property_discriminator() {
    let text = psm();
    assert!(text.contains("private int _d;"), "the state is private");
    assert!(text.contains("public int getDiscriminator() {"));
    assert!(text.contains("public void setDiscriminator(int v) {"));
    assert!(text.contains("v.setDiscriminator("));
    assert!(text.contains("v.getDiscriminator() == 1"));
}

#[test]
fn javapsm_9_1_1_the_codec_reads_and_writes_through_the_accessors() {
    let text = psm();
    for expected in [
        "zd_enc_p_long(w, v.getX());",
        "w.writeBool(v.getZ() != null);",
        "v.setX(zd_dec_p_long(r, TryConstruct.DISCARD));",
        "v.setSamples(zd_dec_seq_long(r, TryConstruct.DISCARD));",
        "v.setZ(null);",
        // The sequence routines are the same walk for arrays and Collections.
        "public static void zd_enc_seq_long(Writer w, java.util.Collection<java.lang.Integer> v) {",
        "w.writeU32(Sequences.count(v));",
        "zd_enc_p_long(w, Sequences.at(v, i));",
        "v = Sequences.alloc(v, (int) zd_n);",
        "Sequences.put(v, zd_i, zd_dec_p_long(r, TryConstruct.DISCARD));",
        "return new java.util.ArrayList<java.lang.Integer>();",
        // A bounded sequence is trimmed by the same helper.
        "v = Sequences.first(v, 4);",
        // `new Collection<Integer>[3]` is not Java.
        "Sequences.newArray(java.util.Collection.class, 3)",
    ] {
        assert!(text.contains(expected), "missing `{expected}`");
    }
    // No direct field access to a member of an aggregate is left (the
    // entry of a map is the one holder with public fields).
    for forbidden in ["v.x;", "v.x =", "v.id)", "v.samples", "v.points"] {
        assert!(!text.contains(forbidden), "`{forbidden}` in the Psm codec");
    }
}

/// A case member named `discriminator` would collide with the PSM's
/// `getDiscriminator` accessor, but it never reaches the generator from
/// IDL: DDS-XTypes 1.3 §7.2.2.4.4.3, "The name of the discriminator member
/// is always 'discriminator'; that name is reserved for union types and is
/// not permitted for type-specific union members", and the front end
/// refuses it in either case (IDL 4.2 §7.2.3.1 compares identifiers without
/// regard to case).
#[test]
fn a_member_named_discriminator_is_refused_before_the_generator() {
    for idl in [
        "union U switch (long) { case 1: long discriminator; };",
        "union U switch (long) { case 1: long Discriminator; };",
    ] {
        let parsed = zerodds_idl_parse::parse(idl);
        assert!(
            parsed.diagnostics().is_empty(),
            "{:?}",
            parsed.diagnostics()
        );
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered
                .diagnostics
                .iter()
                .any(|d| d.message.contains("cannot be named `discriminator`")),
            "{idl}: {:?}",
            lowered.diagnostics
        );
    }
}

// -- the DDS-RPC binding under Clause 9 ------------------------------------

/// Every shape an operation takes: an exception, an enumeration, a
/// structure with an unbounded sequence and an `@optional` member, a base
/// interface, attributes, `out` and `inout` parameters, a sequence
/// parameter and a sequence result.
const RICH: &str = "
module robot {
  exception Overflow { long code; string why; };
  enum Mode { SLOW, FAST };
  struct Pose { long x; long y; sequence<long> trail; @optional string tag; Mode drive; };
  typedef sequence<Pose> Poses;
  interface Base { long ping(); };
  interface Rich : Base {
    readonly attribute long total;
    attribute string label;
    long add(in long a, in long b) raises (Overflow);
    long divmod(in long a, in long b, out long remainder);
    Pose locate(in string room);
    Poses route(in Pose from, in sequence<long> via, inout Mode drive);
    void reset();
    double scale(in double value);
  };
};
";

/// The generated Java of `idl` under `selected`, for the service mapping
/// `service`.
fn generated_service(
    idl: &str,
    selected: JavaMapping,
    service: ServiceMapping,
) -> Result<String, String> {
    let ast = model(idl);
    with_mapping(selected, || {
        zerodds_lang_rules::generate::source_with_mapping(
            &ast,
            Language::Java,
            zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
            zerodds_lang_rules::emit::encode::CodecOptions::default(),
            service,
        )
        .map_err(|error| error.to_string())
    })
}

/// The generated units through `javac -Xlint:all -Werror` with the RPC
/// runtime and the DDS Java PSM it is written over.
fn rpc_compiles(text: &str, tag: &str) -> Result<(), String> {
    let root = corpus_harness::repository_root();
    let dir = std::env::temp_dir().join(format!("zd-java-psm-rpc-{}-{tag}", std::process::id()));
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes).map_err(|error| error.to_string())?;
    let mut files = Vec::new();
    for unit in zerodds_lang_rules::generate::java_units(text) {
        let body = if unit.class == "ZdCodec" {
            format!("import org.zerodds.wire.*;\n\n{}", unit.text)
        } else {
            unit.text.clone()
        };
        let file = dir.join(format!("{}.java", unit.class));
        std::fs::write(&file, body).map_err(|error| error.to_string())?;
        files.push(file);
    }
    for package in [
        "runtime/java/src/main/java/org/omg/dds/rpc",
        "runtime/java/src/main/java/org/zerodds/ddsrpc",
    ] {
        for entry in std::fs::read_dir(root.join(package)).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().is_some_and(|ext| ext == "java") {
                files.push(path);
            }
        }
    }
    let output = Command::new("javac")
        .args(["-encoding", "UTF-8", "-Xlint:all,-unchecked", "-Werror"])
        .arg("-d")
        .arg(&classes)
        .arg("-sourcepath")
        .arg(format!(
            "{}:{}:{}",
            root.join("runtime/java/src/main/java").display(),
            root.join("crates/java-omgdds/java/src/main/java").display(),
            dir.display()
        ))
        .args(&files)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// DDS-RPC 1.0 §7.11.2.3 under DDS-Java-PSM 1.0 Clause 9: the interface of
/// the model reaches the seam. The holders of the binding (`Request`,
/// `Reply`, `In`, `Out`, `Result`) keep their public fields and take the
/// types of the mapping in force; the data types a call carries are the
/// encapsulated classes of Clause 9, and the codec reaches them through
/// their accessors.
#[test]
fn rpc_7_11_2_an_interface_is_generated_under_the_psm_mapping() {
    for service in [ServiceMapping::Basic, ServiceMapping::Enhanced] {
        let psm = generated_service(RICH, JavaMapping::Psm, service)
            .expect("the Psm mapping takes an interface");
        let omg = generated_service(RICH, JavaMapping::Omg, service)
            .expect("the Omg mapping takes an interface");
        // The user types are Clause 9's: private members and accessors.
        assert!(
            psm.contains("public int getCode() {"),
            "{service:?}: the exception is encapsulated"
        );
        assert!(
            !omg.contains("getCode"),
            "{service:?}: the default mapping is unchanged"
        );
        // The operations carry the types of Clause 9: an unbounded sequence
        // is a `Collection<E>`.
        assert!(
            psm.contains("java.util.Collection<java.lang.Integer> via"),
            "{service:?}: a sequence parameter is a Collection"
        );
        assert!(
            omg.contains("int[] via"),
            "{service:?}: and an array under the default mapping"
        );
        // The service holds the exception through its accessors.
        if service == ServiceMapping::Basic {
            assert!(
                psm.contains("value.getCode()"),
                "the exception codec reads through getCode()"
            );
            assert!(
                psm.contains("value.setCode("),
                "and writes through setCode()"
            );
            assert!(
                omg.contains("value.code"),
                "the default mapping reads the field"
            );
        }
    }
}

/// The Clause 9 interface is Java: it compiles with `-Xlint:all -Werror`
/// against the RPC runtime, Basic and Enhanced.
#[test]
fn javapsm_7_2_6_an_interface_under_the_psm_mapping_compiles() {
    if !javac_available() {
        eprintln!("SKIPPED: `javac` is not usable.");
        return;
    }
    for (tag, service) in [
        ("basic", ServiceMapping::Basic),
        ("enhanced", ServiceMapping::Enhanced),
    ] {
        let text = generated_service(RICH, JavaMapping::Psm, service).expect("generates");
        rpc_compiles(&text, &format!("rich-{tag}")).unwrap_or_else(|why| panic!("{tag}: {why}"));
    }
}

/// A service defined by a pair of types (§7.3.1.4) under Clause 9: the
/// `header` member is the runtime's class and is reached through its
/// accessors.
#[test]
fn rpc_7_11_2_a_type_pair_under_the_psm_mapping_compiles() {
    let idl = "
module dds {
  typedef octet GuidPrefix_t[12];
  struct EntityId_t { octet entityKey[3]; octet entityKind; };
  struct GUID_t { GuidPrefix_t guidPrefix; EntityId_t entityId; };
  struct SequenceNumber_t { long high; unsigned long low; };
  struct SampleIdentity { GUID_t writer_guid; SequenceNumber_t sequence_number; };
  module rpc {
    enum RemoteExceptionCode_t { REMOTE_EX_OK, REMOTE_EX_UNSUPPORTED,
      REMOTE_EX_INVALID_ARGUMENT, REMOTE_EX_OUT_OF_RESOURCES,
      REMOTE_EX_UNKNOWN_OPERATION, REMOTE_EX_UNKNOWN_EXCEPTION };
    struct RequestHeader { SampleIdentity requestId; string<255> instanceName; };
    struct ReplyHeader { SampleIdentity relatedRequestId; RemoteExceptionCode_t remoteEx; };
  };
};
module pairs {
  @RPCRequestType @DDSRequestTopic(name=\"MoveRequests\") @service(name=\"Arm\")
  struct Req { dds::rpc::RequestHeader header; long x; double speed; string label; };
  @RPCReplyType @DDSReplyTopic(name=\"MoveReplies\")
  struct Rep { dds::rpc::ReplyHeader header; long y; sequence<long> trail; };
};
";
    let text = generated_service(idl, JavaMapping::Psm, ServiceMapping::Basic).expect("generates");
    assert!(
        text.contains("value.getHeader()"),
        "the codec reads the header through getHeader()"
    );
    assert!(
        text.contains("value.setHeader("),
        "and writes it through setHeader()"
    );
    assert!(
        !text.contains("ZdTypes.dds.rpc.RequestHeader"),
        "the header is the runtime's class, not a declaration of the model"
    );
    if !javac_available() {
        eprintln!("SKIPPED: `javac` is not usable.");
        return;
    }
    rpc_compiles(&text, "pair").unwrap_or_else(|why| panic!("{why}"));
}

#[test]
fn the_psm_mapping_changes_nothing_but_the_shape() {
    // The same wire walk: every encoder and decoder of the default mapping
    // has its counterpart, with the same name, under Clause 9.
    let routine = |text: &str| -> Vec<String> {
        let mut names: Vec<String> = text
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let rest = line
                    .strip_prefix("public static ")
                    .or_else(|| line.strip_prefix("private static "))?;
                let name = rest.split('(').next()?.rsplit(' ').next()?;
                name.starts_with("zd_").then(|| name.to_string())
            })
            .collect();
        names.sort();
        names
    };
    assert_eq!(routine(&omg()), routine(&psm()));
}

// -- the compiler --------------------------------------------------------

fn javac_available() -> bool {
    // A look at PATH and not a start of the tool: a JVM, .NET or BEAM
    // start cost seconds of a test's 25 s with eight compilers side by side
    // (pipeline 8678); a tool that cannot start fails the compile that follows.
    on_path("javac")
}

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(tool).is_file()))
}

/// The compiled wire runtime, built once: `Sequences` is part of what the
/// Clause 9 codec means, so the whole of `org/zerodds/wire` is on the
/// classpath.
static RUNTIME: LazyLock<Result<PathBuf, String>> = LazyLock::new(|| {
    let classes = std::env::temp_dir().join(format!("zd-java-psm-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&classes).map_err(|error| error.to_string())?;
    let wire =
        corpus_harness::repository_root().join("runtime/java/src/main/java/org/zerodds/wire");
    let mut sources: Vec<PathBuf> = std::fs::read_dir(&wire)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
        .collect();
    sources.sort();
    if let Some(classes) = corpus_harness::precompiled_java_runtime(&sources) {
        return Ok(classes);
    }
    let output = Command::new("javac")
        .args(["-encoding", "UTF-8"])
        .arg("-d")
        .arg(&classes)
        .args(&sources)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(classes)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
});

/// The text `generate::source` writes as the two compilation units
/// `idlc generate` writes. `idlc` puts a header and a preamble (the
/// `org.zerodds.wire` import and the `ZdEndian` holder) ahead of the
/// body; `generate::source` is the body alone: the declarations, then
/// the codec. The declarations take no preamble, the codec takes it.
fn halves(text: &str) -> (String, String) {
    let codec_at = text.find("public class ZdCodec {").expect("a codec");
    let preamble = "import org.zerodds.wire.*;\n\
                    final class ZdEndian { static final Endianness VALUE = Endianness.LITTLE; }\n";
    (
        format!("{}\n", text[..codec_at].trim_end()),
        format!("{preamble}{}\n", text[codec_at..].trim_end()),
    )
}

fn compiles(runtime: &Path, text: &str, tag: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("zd-java-psm-{}-{tag}", std::process::id()));
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes).map_err(|error| error.to_string())?;
    let (types, codec) = halves(text);
    std::fs::write(dir.join("ZdTypes.java"), types).map_err(|error| error.to_string())?;
    std::fs::write(dir.join("ZdCodec.java"), codec).map_err(|error| error.to_string())?;
    let result = javac_without_a_warning(
        runtime,
        &classes,
        &[dir.join("ZdTypes.java"), dir.join("ZdCodec.java")],
    );
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// `javac` over `files` into `classes`, with the warnings generics bring
/// switched on; an error or any of those warnings is a failure.
fn javac_without_a_warning(
    runtime: &Path,
    classes: &Path,
    files: &[PathBuf],
) -> Result<(), String> {
    let output = Command::new("javac")
        .args(["-encoding", "UTF-8", "-Xlint:unchecked,rawtypes,cast"])
        .arg("-cp")
        .arg(runtime)
        .arg("-d")
        .arg(classes)
        .args(files)
        .output()
        .map_err(|error| error.to_string())?;
    // Everything on standard error is a warning, except the notice the JVM
    // launcher prints for JAVA_TOOL_OPTIONS, which the merge jobs set
    // (ci/merge-jobs.yml): "Picked up JAVA_TOOL_OPTIONS: ...".
    let stderr = String::from_utf8_lossy(&output.stderr);
    let warnings: Vec<&str> = stderr
        .lines()
        .filter(|line| !line.starts_with("Picked up JAVA_TOOL_OPTIONS:"))
        .collect();
    if output.status.success() && warnings.is_empty() {
        Ok(())
    } else {
        Err(stderr.into_owned())
    }
}

/// A counter, so two batches of one test process do not share a directory.
static SERIAL: AtomicU32 = AtomicU32::new(0);

/// The cases of one share through one `javac`, each case's two units in a
/// directory and a package of their own (`zd_case_<n>`; the generated text
/// declares no package, and every case declares a public `ZdTypes` and
/// `ZdCodec`): one JVM for the share instead of one per case. A `javac` per
/// case was most of the 7.8 s a share of nine cases took on the CI runner host (pipeline
/// 8967, job 229628). When the batch fails or warns, every case is compiled
/// alone ([`compiles`]), so that a failure names its case in the compiler's
/// words. The cases that did not compile, with `javac`'s message.
fn compiles_together(runtime: &Path, cases: &[(String, String)]) -> Vec<(String, String)> {
    if cases.is_empty() {
        return Vec::new();
    }
    let dir = std::env::temp_dir().join(format!(
        "zd-java-psm-together-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let started = std::time::Instant::now();
    let together = (|| {
        let classes = dir.join("classes");
        std::fs::create_dir_all(&classes).map_err(|error| error.to_string())?;
        let mut files = Vec::new();
        for (index, (_, text)) in cases.iter().enumerate() {
            let package = format!("zd_case_{index}");
            let case = dir.join(&package);
            std::fs::create_dir_all(&case).map_err(|error| error.to_string())?;
            let (types, codec) = halves(text);
            for (class, unit) in [("ZdTypes", types), ("ZdCodec", codec)] {
                let file = case.join(format!("{class}.java"));
                std::fs::write(&file, format!("package {package};\n\n{unit}"))
                    .map_err(|error| error.to_string())?;
                files.push(file);
            }
        }
        javac_without_a_warning(runtime, &classes, &files)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    // Said on the channel a passing test does not use: how long the one
    // `javac` took, and whether the cases go on alone.
    eprintln!(
        "JAVA_PSM_TOGETHER cases={} ok={} seconds={:.1}",
        cases.len(),
        together.is_ok(),
        started.elapsed().as_secs_f64()
    );
    if together.is_ok() {
        return Vec::new();
    }
    cases
        .iter()
        .filter_map(|(name, text)| {
            compiles(runtime, text, name)
                .err()
                .map(|why| (name.clone(), why))
        })
        .collect()
}

/// The Clause 9 types and their codec are Java: `javac` takes them with
/// no error and no warning of the kinds generics bring (unchecked, raw
/// type, redundant cast).
#[test]
fn javapsm_9_1_1_the_psm_types_and_codec_compile_without_a_warning() {
    if !javac_available() {
        eprintln!("SKIPPED: `javac` is not usable.");
        return;
    }
    let runtime = RUNTIME.clone().expect("runtime/java compiles");
    compiles(&runtime, &psm(), "unit").unwrap_or_else(|why| panic!("{why}"));
    compiles(&runtime, &omg(), "unit-omg").unwrap_or_else(|why| panic!("{why}"));
    // The batch the corpus goes through: both cases compile together, and
    // a case with a raw type among them is named alone, by its warning.
    let cases = [("psm".to_string(), psm()), ("omg".to_string(), omg())];
    let failures = compiles_together(&runtime, &cases);
    assert!(failures.is_empty(), "{failures:#?}");
    let raw = psm().replacen(
        "public class ZdCodec {",
        "public class ZdCodec {\n    static java.util.List raw = new java.util.ArrayList();",
        1,
    );
    let failures = compiles_together(&runtime, &[cases[0].clone(), ("raw".to_string(), raw)]);
    assert_eq!(
        failures
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["raw"],
        "{failures:#?}"
    );
    assert!(failures[0].1.contains("[rawtypes]"), "{failures:#?}");
}

/// The cases the Java column itself refuses, as `java_corpus_compiles`
/// lists them.
const REFUSED: &[&str] = &["resolve-module-named-as-type"];

/// How many corpus cases have to reach `javac` for this gate to mean
/// anything. 416 render today.
const MUST_RENDER: usize = 350;

/// The whole corpus under the Clause 9 mapping, through `javac`: the cases
/// of `share` (of `shares`) compile, in one `javac` ([`compiles_together`]),
/// and every case renders.
fn every_corpus_case_compiles_under_the_psm_mapping(share: usize, shares: usize) {
    if !javac_available() {
        eprintln!("SKIPPED: `javac` is not usable.");
        return;
    }
    let runtime = RUNTIME.clone().expect("runtime/java compiles");
    let mut failures = Vec::new();
    let mut selected = Vec::new();
    let mut rendered = 0_usize;
    for (position, (name, path)) in corpus_harness::corpus().into_iter().enumerate() {
        if REFUSED.contains(&name.as_str()) {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let text = match generated(&source, JavaMapping::Psm) {
            Ok(text) => text,
            Err(why) => {
                failures.push(format!("{name}: {why}"));
                continue;
            }
        };
        rendered += 1;
        if position % shares == share {
            selected.push((name, text));
        }
    }
    let checked = selected.len();
    for (name, why) in compiles_together(&runtime, &selected) {
        failures.push(format!("{name}: {why}"));
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} did not compile:\n{failures:#?}",
        failures.len()
    );
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} corpus cases reached `javac`"
    );
}

/// CORBA 3.3 part 3 §1.5.2: `octet` is a Java `byte`. An octet constant
/// above 127 is the byte with its bits, written as the hex literal cast to
/// `byte`; the octet constants of XTypes 1.3 Annex B are such (`EK_MINIMAL
/// = 0xF1`). One below 128 keeps its decimal form, through a typedef too.
#[test]
fn an_octet_constant_above_127_is_its_bit_pattern_as_a_byte() {
    const OCTETS: &str = "
module M {
  typedef octet Kind;
  const octet EK_MINIMAL = 0xF1;
  const Kind EK_BOTH = 0xF3;
  const octet SMALL = 0x10;
  @final struct S { octet k; };
};
";
    for mapping in [JavaMapping::Omg, JavaMapping::Psm] {
        let text = generated(OCTETS, mapping).expect("octet constants have a Java form");
        assert!(
            text.contains("public static final byte EK_MINIMAL = (byte) 0xF1;"),
            "{mapping:?}: {text}"
        );
        assert!(
            text.contains("public static final byte EK_BOTH = (byte) 0xF3;"),
            "{mapping:?}: {text}"
        );
        assert!(
            text.contains("public static final byte SMALL = (byte) 16;"),
            "{mapping:?}: {text}"
        );
    }
}

/// Every corpus case compiles under the Clause 9 mapping through `javac`.
///
/// The corpus in 16 shares of 26 or 27 cases, each share one `javac`
/// ([`compiles_together`]) within the 25 s a test has
/// (internal/release/rc8-test-suite.md, "Crate tier"); the shares together
/// are every case. `#[ignore]`: the crate tier has no toolchain for it; the
/// weekly `integration-codegen-java-psm-d` job runs them with the toolchain,
/// four at a time.
mod every_corpus_case_compiles_under_the_psm_mapping_shares {
    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_01() {
        super::every_corpus_case_compiles_under_the_psm_mapping(0, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_02() {
        super::every_corpus_case_compiles_under_the_psm_mapping(1, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_03() {
        super::every_corpus_case_compiles_under_the_psm_mapping(2, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_04() {
        super::every_corpus_case_compiles_under_the_psm_mapping(3, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_05() {
        super::every_corpus_case_compiles_under_the_psm_mapping(4, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_06() {
        super::every_corpus_case_compiles_under_the_psm_mapping(5, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_07() {
        super::every_corpus_case_compiles_under_the_psm_mapping(6, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_08() {
        super::every_corpus_case_compiles_under_the_psm_mapping(7, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_09() {
        super::every_corpus_case_compiles_under_the_psm_mapping(8, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_10() {
        super::every_corpus_case_compiles_under_the_psm_mapping(9, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_11() {
        super::every_corpus_case_compiles_under_the_psm_mapping(10, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_12() {
        super::every_corpus_case_compiles_under_the_psm_mapping(11, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_13() {
        super::every_corpus_case_compiles_under_the_psm_mapping(12, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_14() {
        super::every_corpus_case_compiles_under_the_psm_mapping(13, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_15() {
        super::every_corpus_case_compiles_under_the_psm_mapping(14, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus under the PSM mapping through one javac; runs in integration-codegen-java-psm-d"]
    fn share_16() {
        super::every_corpus_case_compiles_under_the_psm_mapping(15, 16);
    }
}
