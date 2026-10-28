// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The generated C#, compiled into one assembly and used from another.
//!
//! `csharp_corpus_compiles.rs` asks whether the emitted text is C# that
//! Roslyn accepts. That is one question and it is the smaller one. It
//! compiles the declarations on their own, into one assembly, with
//! nothing outside that assembly naming anything in it — so `public`,
//! `internal` and `private` are indistinguishable to it, and so are a
//! member that is there under the name the mapping promised and a
//! member that is there under another.
//!
//! Nor does the byte ledger close that gap.
//! `tools/wireprobe/wireprobe-zerodds-csharp` compiles `ZdTypes.cs` and
//! `Probe.cs` **together**, with `-target:exe`, into a single assembly;
//! `wireprobe-zerodds-csharp-gen` puts the codec's methods *inside*
//! `public static class Probe` beside `Main`. Every octet in
//! `tests/interop-results/sources/zerodds-csharp.tsv` was produced by a
//! caller sitting in the same assembly as the code it called. That is
//! exactly the blind spot
//! `crates/lang-rules/tests/every_column_publishes_its_codec.rs` names
//! in its own module doc — "every probe column writes its entry point
//! into the same translation unit as the generated code … so a private
//! codec and a public one look identical to it" — and it answers the
//! question with a substring.
//!
//! So here the generated source is compiled into `ZdTypes.dll` and a
//! **second** assembly is compiled that references it, names its types
//! and its codecs in C# source, calls them, and runs. Roslyn's
//! accessibility check across an assembly reference is what turns each
//! of the substrings below into something an execution confirms:
//!
//! | asserted as a substring in | what runs here |
//! |---|---|
//! | `every_column_publishes_its_codec.rs`: `public static void zd_enc_Max(` | a second assembly calls `ZdCodec.zd_enc_Max` and reads the octets back |
//! | `inherited_operations.rs`: `public interface Derived`, `void G();`, `void F();` | a second assembly declares a class implementing `Derived` and calls both |
//! | `keyword_collision.rs`: `Class;` and `Class_;` | a second assembly assigns and reads both fields |
//! | `a_bound_belongs_to_the_hoisted_routine.rs`: `seq_b4_long` / `string_b4` / `wstring_b4` / `map_b4_` and the `zd_over` counts | a second assembly decodes a stream that is over the bound in the bounded member and one that is over it in the unbounded member beside it, and sees the refusal on the first only |
//!
//! What is **not** closed here, and cannot be by any executing
//! assembly: `verbatim_end_to_end.rs`'s
//! `the_wildcard_reaches_all_four_columns` asserts `// vb-everyone` in
//! the C# output. `@verbatim` text placed as a comment is not in the
//! compiled metadata at all, so a consumer has nothing to name. That
//! one stays a text assertion because its subject is text.
//!
//! ## The assembly the library is built as
//!
//! Not `generate::source`'s text verbatim, and the difference is one
//! documented edit. The C# materialiser emits its own copy of the
//! `ZeroDds.Idl` support namespace into every file — `IdlNameAttribute`
//! and `Float128` — because one generated file has to be one compiler
//! argument. `runtime/csharp` declares the same two names, so a
//! compilation that has both answers `error CS0436` under
//! `-warnaserror`, and where the local `Float128` wins it is a
//! different type from the `ZeroDds.Idl.Float128` that
//! `Writer.WriteF128` takes. `wireprobe-zerodds-csharp-gen` drops the
//! generated copy for that reason and says so;
//! [`library_source`] does the same through the same public seam,
//! `adapter::csharp::drop_support_namespace`, and asserts that the file
//! it dropped it from is character-for-character the one
//! `generate::declarations` renders — so the assembly under test is the
//! column's own artifact and not a variant of it.
//!
//! ## Roslyn directly, not `dotnet build`
//!
//! `csharp_corpus_compiles.rs`'s reason, unchanged: `dotnet build`
//! spends two to four seconds in MSBuild before Roslyn sees a file.
//! The SDK, its Roslyn and its reference pack are found the same way
//! that file finds them, off `dotnet --list-sdks`.
//!
//! `print_stderr` is allowed here on purpose: a skipped compiler check
//! has to be *said*, and standard error is the channel a passing test
//! does not otherwise use.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use zerodds_ast::Ast;
use zerodds_ast_layout::key::Dialect;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;
use zerodds_lang_cst::csharp;
use zerodds_lang_rules::emit::csharp::CSharp;
use zerodds_lang_rules::emit::encode::CodecOptions;
use zerodds_lang_rules::{ModelGraph, adapter, apply, rule};

#[path = "support/corpus.rs"]
mod corpus_harness;

/// The layout every assembly here is built under.
///
/// XCDR2 plain, little-endian — `every_column_publishes_its_codec.rs`'s
/// key, so the codec whose publication that file asserts as a substring
/// is the one that is called here.
fn key() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
}

/// The options `generate::source` builds a codec under.
///
/// `crates/lang-rules/src/generate.rs::codec_options`, which is private
/// to that module: a `map` under a delimited encoding carries its entry
/// count where the dialect does not omit it.
fn codec_options() -> CodecOptions {
    CodecOptions {
        map_length_with_delimiter: !Dialect::Specification.map_count_omitted_when_delimited(),
    }
}

// -- the model ------------------------------------------------------------

/// IDL in, a resolved model out.
fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "consumer.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

/// The codec's members: the encoders, the Table 9 defaulters and the
/// decoders, over the whole model and under one plan.
///
/// `crates/lang-rules/src/generate.rs::codec_of` bound to `CSharp`,
/// through the public seams `wireprobe-zerodds-csharp-gen` uses.
fn codec_members(ast: &Ast, key: LayoutKey) -> Vec<csharp::Member> {
    let options = codec_options();
    let mut members =
        zerodds_lang_rules::emit::encode::codec::<CSharp>(ast, key, options).expect("the encoders");
    let decoders = zerodds_lang_rules::emit::decode::decoder::<CSharp>(ast, key, options)
        .expect("the decoders");
    members.extend(
        zerodds_lang_rules::default_csharp::csharp_defaulters_called_by(ast, &decoders)
            .expect("the Table 9 defaulters"),
    );
    members.extend(decoders);
    members
}

/// The `ZdCodec` class the C# arm of `generate::codec` renders.
fn codec_class(members: Vec<csharp::Member>) -> String {
    zerodds_lang_render::csharp::type_decl(&csharp::TypeDecl::Class(csharp::Class {
        doc: zerodds_lang_cst::common::Doc::none(),
        attributes: Vec::new(),
        modifiers: vec![csharp::Modifier::Public, csharp::Modifier::Static],
        name: zerodds_lang_cst::common::Ident::new("ZdCodec"),
        generics: Vec::new(),
        bases: Vec::new(),
        constraints: Vec::new(),
        members,
    }))
}

/// The C# library assembly's source: the column's own declarations and
/// its own codec, with the generated copy of the support namespace
/// dropped in favour of `runtime/csharp`'s.
///
/// The two assertions are what tie this to `generate::source`: the file
/// before the drop renders to exactly what `generate::declarations`
/// produces, and the codec class renders to exactly the tail
/// `generate::source` appends. Nothing else is edited.
fn library_source(ast: &Ast, key: LayoutKey) -> String {
    let graph = ModelGraph::of(ast);
    let applied = apply(&graph, &rule::csharp::catalogue()).expect("apply");
    let mut file = adapter::csharp::csharp_file(ast, &applied).expect("materialise");

    let declarations =
        zerodds_lang_rules::generate::declarations(ast, Language::CSharp).expect("declarations");
    assert_eq!(
        zerodds_lang_render::csharp::file(&file),
        declarations,
        "this file is not the one `generate::declarations` renders"
    );
    assert!(
        adapter::csharp::drop_support_namespace(&mut file),
        "the materialiser emitted no `ZeroDds.Idl` namespace, so it cannot be \
         told whether `runtime/csharp`'s is the one in force"
    );
    let types = zerodds_lang_render::csharp::file(&file);

    let codec = codec_class(codec_members(ast, key));
    let whole = zerodds_lang_rules::generate::source(ast, Language::CSharp, key).expect("source");
    assert!(
        whole.ends_with(&codec),
        "this codec is not the one `generate::source` appends:\n{codec}\n\n{whole}"
    );

    format!("{types}\n\n{codec}")
}

// -- Roslyn ---------------------------------------------------------------

/// Where Roslyn and the reference assemblies are.
struct Roslyn {
    /// The path of `csc.dll` inside the SDK.
    csc: PathBuf,
    /// A response file naming every reference assembly, one `-r:` per
    /// line. On the command line instead, `net9.0`'s hundred and sixty
    /// assemblies are longer than some `execve` limits.
    references: PathBuf,
}

/// The newest SDK the installation carries, and its reference pack.
///
/// `csharp_corpus_compiles.rs::roslyn`, unchanged: `ZERODDS_CSC` and
/// `ZERODDS_NETREF` override, as they do for the probe, and `None` where
/// there is no compiler — a runtime-only install has none, and neither
/// does a host without .NET.
fn roslyn() -> Option<Roslyn> {
    let listed = Command::new("dotnet").arg("--list-sdks").output().ok()?;
    if !listed.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&listed.stdout).into_owned();
    let mut lines: Vec<&str> = text.lines().filter(|line| line.contains('[')).collect();
    lines.sort_unstable();
    let line = lines.last()?;
    let (version, rest) = line.split_once(' ')?;
    let directory = rest.trim().trim_start_matches('[').trim_end_matches(']');
    let sdk = PathBuf::from(directory);
    let csc = match std::env::var("ZERODDS_CSC") {
        Ok(chosen) => PathBuf::from(chosen),
        Err(_) => sdk.join(version).join("Roslyn/bincore/csc.dll"),
    };
    if !csc.is_file() {
        return None;
    }
    let pack = match std::env::var("ZERODDS_NETREF") {
        Ok(chosen) => PathBuf::from(chosen),
        Err(_) => {
            let root = sdk.parent()?.join("packs/Microsoft.NETCore.App.Ref");
            let mut versions: Vec<PathBuf> = std::fs::read_dir(&root)
                .ok()?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect();
            versions.sort();
            let newest = versions.last()?.join("ref");
            let mut frameworks: Vec<PathBuf> = std::fs::read_dir(&newest)
                .ok()?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect();
            frameworks.sort();
            frameworks.last()?.clone()
        }
    };
    let mut response = String::new();
    let mut assemblies: Vec<PathBuf> = std::fs::read_dir(&pack)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "dll"))
        .collect();
    assemblies.sort();
    for assembly in assemblies {
        response.push_str(&format!("-r:{}\n", assembly.display()));
    }
    let references = std::env::temp_dir().join(format!(
        "zd-csharp-consumer-refs-{}.rsp",
        std::process::id()
    ));
    std::fs::write(&references, response).ok()?;
    Some(Roslyn { csc, references })
}
/// The scratch directory this test binary owns.
fn scratch() -> PathBuf {
    std::env::temp_dir().join(format!("zd-csharp-consumer-{}", std::process::id()))
}

/// One `csc` invocation, with the framework reference pack and whatever
/// `extra` names on top of it.
///
/// `-warnaserror+ -warn:4`, the flags `wireprobe-zerodds-csharp`
/// compiles both of its assemblies with: a warning in generated wire
/// code is a question nobody answered.
fn compile(roslyn: &Roslyn, extra: &[String], sources: &[PathBuf]) -> Result<(), String> {
    let output = Command::new("dotnet")
        .arg(&roslyn.csc)
        .args([
            "-nologo",
            "-nostdlib",
            "-noconfig",
            "-langversion:12.0",
            "-warnaserror+",
            "-warn:4",
            "-optimize+",
            "-deterministic",
        ])
        .arg(format!("@{}", roslyn.references.display()))
        .args(extra)
        .args(sources)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// `runtime/csharp` as `ZeroDds.Wire.dll`, compiled once per test
/// binary.
///
/// The path, or the reason there is none. Built into
/// [`scratch`] and kept for the run: every case below references the
/// same assembly, and compiling the runtime once per case is twelve
/// files of C# per case for no answer.
static RUNTIME: LazyLock<Result<PathBuf, String>> = LazyLock::new(|| {
    let Some(roslyn) = roslyn() else {
        return Err("no .NET SDK with a `csc.dll` was found".to_string());
    };
    let dir = scratch().join("runtime");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let out = dir.join("ZeroDds.Wire.dll");
    let source_dir = corpus_harness::repository_root().join("runtime/csharp/src");
    let mut sources: Vec<PathBuf> = std::fs::read_dir(&source_dir)
        .map_err(|error| format!("{}: {error}", source_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cs"))
        .collect();
    sources.sort();
    compile(
        &roslyn,
        &[
            "-target:library".to_string(),
            format!("-out:{}", out.display()),
        ],
        &sources,
    )
    .map_err(|why| format!("runtime/csharp did not compile:\n{why}"))?;
    Ok(out)
});

/// What a two-assembly case produced.
#[derive(Debug)]
struct Ran {
    /// The consumer's exit code.
    code: i32,
    /// What it wrote to standard output.
    stdout: String,
    /// What it wrote to standard error.
    stderr: String,
}

/// `library` into one assembly, `consumer` into a second that
/// references it, and the second one run.
///
/// The consumer's compilation names `ZdTypes.dll` and
/// `ZeroDds.Wire.dll` and no source of either: what it can reach is
/// what those two assemblies publish, which is the question.
fn two_assemblies(
    roslyn: &Roslyn,
    runtime: &Path,
    library: &str,
    consumer: &str,
    tag: &str,
) -> Result<Ran, String> {
    let dir = scratch().join(tag);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;

    let types_source = dir.join("ZdTypes.cs");
    std::fs::write(&types_source, library).map_err(|error| error.to_string())?;
    let types_dll = dir.join("ZdTypes.dll");
    compile(
        roslyn,
        &[
            "-target:library".to_string(),
            format!("-out:{}", types_dll.display()),
            format!("-r:{}", runtime.display()),
        ],
        &[types_source],
    )
    .map_err(|why| format!("the generated library did not compile:\n{why}"))?;

    let consumer_source = dir.join("Consumer.cs");
    std::fs::write(&consumer_source, consumer).map_err(|error| error.to_string())?;
    let consumer_dll = dir.join("Consumer.dll");
    compile(
        roslyn,
        &[
            "-target:exe".to_string(),
            "-main:Consumer".to_string(),
            format!("-out:{}", consumer_dll.display()),
            format!("-r:{}", runtime.display()),
            format!("-r:{}", types_dll.display()),
        ],
        &[consumer_source],
    )
    .map_err(|why| format!("the consuming assembly did not compile:\n{why}"))?;

    // What `dotnet Consumer.dll` needs to find a shared framework,
    // written here rather than by MSBuild for the reason
    // `wireprobe-zerodds-csharp` gives: MSBuild is the thing this
    // skips. `LatestMajor` because the installed runtime is the
    // measuring host's and not this tree's.
    std::fs::write(
        dir.join("Consumer.runtimeconfig.json"),
        "{\n  \"runtimeOptions\": {\n    \"tfm\": \"net8.0\",\n    \
         \"rollForward\": \"LatestMajor\",\n    \"framework\": { \"name\": \
         \"Microsoft.NETCore.App\", \"version\": \"8.0.0\" }\n  }\n}\n",
    )
    .map_err(|error| error.to_string())?;
    std::fs::copy(runtime, dir.join("ZeroDds.Wire.dll")).map_err(|error| error.to_string())?;

    let output = Command::new("dotnet")
        .arg(&consumer_dll)
        .output()
        .map_err(|error| error.to_string())?;
    // Kept only where a compiler refused, which is the case whose two
    // sources somebody will want to read. A corpus sweep that kept all
    // four hundred would leave a directory per case behind.
    let _ = std::fs::remove_dir_all(&dir);
    Ok(Ran {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// A case that has to come back green, with the whole of what the two
/// compilers and the program said on failure.
fn passes(roslyn: &Roslyn, runtime: &Path, library: &str, consumer: &str, tag: &str) {
    match two_assemblies(roslyn, runtime, library, consumer, tag) {
        Ok(ran) => assert_eq!(
            ran.code, 0,
            "{tag}: the consuming assembly answered {}:\n{}{}",
            ran.code, ran.stdout, ran.stderr
        ),
        Err(why) => panic!("{tag}: {why}"),
    }
}

/// The runtime assembly, or a printed skip.
///
/// A macro and not a function: what it does on absence is `return`,
/// and that has to happen in the test's own body.
macro_rules! runtime_or_skip {
    () => {
        match &*RUNTIME {
            Ok(path) => path.clone(),
            Err(why) => {
                eprintln!("SKIPPED: {why}.");
                return;
            }
        }
    };
}

/// Roslyn, or a printed skip.
macro_rules! roslyn_or_skip {
    () => {
        match roslyn() {
            Some(found) => found,
            None => {
                eprintln!("SKIPPED: no .NET SDK with a `csc.dll` was found.");
                return;
            }
        }
    };
}

// -- what every consumer opens with ---------------------------------------

/// The two runtime constructions every consumer below needs, built from
/// the layout key rather than written out, so a consumer cannot be
/// reading under a plan the library was not written under.
///
/// `MAXALIGN` is the encoding's own (XTypes 1.3 §7.4.1.1.2), and the
/// short-length-code rewrite is the key's. `PlCdr1Framing.Defaults()`
/// is the runtime's own default and is not reached by any of these
/// models: they are `@final`, which has no parameter list.
fn wire_prelude(key: LayoutKey) -> String {
    format!(
        "    static global::ZeroDds.Wire.Writer NewWriter()\n\
         \x20   {{\n\
         \x20       return new global::ZeroDds.Wire.Writer(\n\
         \x20           global::ZeroDds.Wire.Endianness.Little,\n\
         \x20           {align},\n\
         \x20           global::ZeroDds.Wire.PlCdr1Framing.Defaults(),\n\
         \x20           {short});\n\
         \x20   }}\n\n\
         \x20   static global::ZeroDds.Wire.Reader NewReader(byte[] bytes)\n\
         \x20   {{\n\
         \x20       return new global::ZeroDds.Wire.Reader(\n\
         \x20           bytes,\n\
         \x20           global::ZeroDds.Wire.Endianness.Little,\n\
         \x20           {align},\n\
         \x20           global::ZeroDds.Wire.PlCdr1Framing.Defaults());\n\
         \x20   }}\n\n\
         \x20   static int Fail(string why)\n\
         \x20   {{\n\
         \x20       System.Console.Error.WriteLine(why);\n\
         \x20       return 1;\n\
         \x20   }}\n",
        align = key.encoding.max_alignment(),
        short = key.length_codes.short_code_where_it_fits(),
    )
}

/// One consuming assembly: the prelude and `body` inside
/// `public static class Consumer`.
fn consumer(key: LayoutKey, body: &str) -> String {
    format!(
        "// A second assembly. It references ZdTypes.dll and \
         ZeroDds.Wire.dll\n// and carries no source of either.\n\n\
         public static class Consumer\n{{\n{prelude}\n{body}}}\n",
        prelude = wire_prelude(key),
        body = body,
    )
}

// -- the check has teeth --------------------------------------------------

/// A `private` member is unreachable across the reference, and a
/// `public` one is reachable.
///
/// Without this, a green result below could come from a harness that
/// never compiled the second assembly at all — and the property the
/// whole file rests on is precisely that Roslyn enforces accessibility
/// across an assembly reference, which is what the probe's
/// single-assembly compilation never asks.
#[test]
fn a_private_member_is_unreachable_from_the_second_assembly() {
    let roslyn = roslyn_or_skip!();
    let runtime = runtime_or_skip!();
    let library = "public static class Lib\n{\n\
                   \x20   public static int Shown() { return 2; }\n\
                   \x20   private static int Hidden() { return 1; }\n\
                   \x20   internal static int Kept() { return 3; }\n\
                   }\n\n\
                   internal static class Internals\n{\n\
                   \x20   public static int Shown() { return 4; }\n\
                   }\n";
    let reaches_the_public_one = "    public static int Main()\n\
                                  \x20   {\n\
                                  \x20       return global::Lib.Shown() == 2 ? 0 : Fail(\"Shown\");\n\
                                  \x20   }\n";
    passes(
        &roslyn,
        &runtime,
        library,
        &consumer(key(), reaches_the_public_one),
        "teeth-public",
    );

    let reaches_the_private_one = "    public static int Main()\n\
                                   \x20   {\n\
                                   \x20       return global::Lib.Hidden();\n\
                                   \x20   }\n";
    let why = two_assemblies(
        &roslyn,
        &runtime,
        library,
        &consumer(key(), reaches_the_private_one),
        "teeth-private",
    )
    .expect_err("the second assembly reached a private member");
    // **CS0117** and not CS0122, measured: across an assembly reference
    // a `private` member is not an inaccessible member of the type, it
    // is not a member of the type — the compiler drops it from the
    // referenced type's surface before name lookup runs. The code that
    // matters is that the name did not resolve and that the message
    // names it; a green run here with either code would be the harness
    // never having compiled the second assembly.
    assert!(
        why.contains("CS0117") && why.contains("Hidden"),
        "the refusal is not the one a private member gets:\n{why}"
    );

    // `internal`, the other way a member or a type stops being part of
    // the surface — and the one a C# emitter falls into by omission,
    // because `internal` is what a type with no access modifier is.
    for (name, expression, tag) in [
        ("Lib.Kept", "global::Lib.Kept()", "teeth-internal-member"),
        (
            "Internals",
            "global::Internals.Shown()",
            "teeth-internal-type",
        ),
    ] {
        let reaches_it = format!(
            "    public static int Main()\n\
             \x20   {{\n\
             \x20       return {expression};\n\
             \x20   }}\n"
        );
        let outcome = two_assemblies(
            &roslyn,
            &runtime,
            library,
            &consumer(key(), &reaches_it),
            tag,
        );
        let Err(why) = outcome else {
            panic!("the second assembly reached {name}");
        };
        assert!(
            why.contains("CS0122") || why.contains("CS0117") || why.contains("CS0103"),
            "the refusal is not one of the three a non-public name gets:\n{why}"
        );
    }

    // And a name that is not there at all, so the reference itself is
    // what resolves the ones that are.
    let names_nothing = "    public static int Main()\n\
                         \x20   {\n\
                         \x20       return global::NoSuchTypeExists.Anything();\n\
                         \x20   }\n";
    let why = two_assemblies(
        &roslyn,
        &runtime,
        library,
        &consumer(key(), names_nothing),
        "teeth-absent",
    )
    .expect_err("the second assembly resolved a name nothing declares");
    assert!(why.contains("NoSuchTypeExists"), "{why}");
}

// -- the codec is published -----------------------------------------------

/// The model `every_column_publishes_its_codec.rs` asks its one
/// question of.
const MAX: &str = "@final struct Max { long a; };";

/// A second assembly calls the encoder and the decoder, and the octets
/// are the ones the type says.
///
/// This is `every_column_publishes_its_codec.rs`'s C# row —
/// `public static void zd_enc_Max(` — asked as a consumer asks it.
/// `Modifier::Public` back to `Private` in `emit/csharp.rs` turns that
/// file red on a substring and this one red on `error CS0122`.
///
/// Four octets and `07 00 00 00`: an IDL `long` is a 32-bit signed
/// integer (IDL 4.2 §7.4.1.4.4.2.1) and XCDR2 writes it little-endian
/// here, which is also the row the ledger carries for a `long`.
#[test]
fn a_second_assembly_calls_the_generated_encoder_and_decoder() {
    let roslyn = roslyn_or_skip!();
    let runtime = runtime_or_skip!();
    let ast = model(MAX);
    let body = "    public static int Main()\n\
        \x20   {\n\
        \x20       global::Max sample = new global::Max();\n\
        \x20       sample.A = 7;\n\
        \x20       var w = NewWriter();\n\
        \x20       global::ZdCodec.zd_enc_Max(w, sample);\n\
        \x20       byte[] bytes = w.Finish();\n\
        \x20       if (bytes.Length != 4)\n\
        \x20       {\n\
        \x20           return Fail(\"a long took \" + bytes.Length + \" octets\");\n\
        \x20       }\n\
        \x20       if (bytes[0] != 7 || bytes[1] != 0 || bytes[2] != 0 || bytes[3] != 0)\n\
        \x20       {\n\
        \x20           return Fail(\"the octets are \" + System.BitConverter.ToString(bytes));\n\
        \x20       }\n\
        \x20       var r = NewReader(bytes);\n\
        \x20       global::Max back = global::ZdCodec.zd_dec_Max(\n\
        \x20           r, global::ZeroDds.Wire.TryConstruct.Discard);\n\
        \x20       if (r.ReaderStatus() != global::ZeroDds.Wire.Status.Ok)\n\
        \x20       {\n\
        \x20           return Fail(\"the reader answered \" + r.ReaderStatus());\n\
        \x20       }\n\
        \x20       if (back.A != 7)\n\
        \x20       {\n\
        \x20           return Fail(\"the member came back \" + back.A);\n\
        \x20       }\n\
        \x20       return 0;\n\
        \x20   }\n";
    passes(
        &roslyn,
        &runtime,
        &library_source(&ast, key()),
        &consumer(key(), body),
        "publishes-its-codec",
    );
}

// -- the interface carries its bases' operations and attributes -----------

/// A second assembly implements the generated interface — both
/// operations, both properties, one of them raising — and calls all
/// four.
///
/// `inherited_operations.rs::the_csharp_interface_carries_the_bases_operations`
/// asserts `public interface Derived` with the flattened members in its
/// body as a substring; a class in another assembly that declares
/// `: global::Derived` has to implement exactly what the interface
/// declares — a member missing is `CS0535`, one whose signature moved
/// is the same, one that is there but not accessible is `CS0122` on the
/// interface itself, and a property implemented as a method (or the
/// reverse) does not compile at all. So the implementation is the
/// assertion, run rather than only compiled: `Main` calls every member
/// through the interface type and checks the value each one answers
/// with, so a getter that silently returned the wrong field would fail
/// here and not only in `inherited_operations.rs`'s substring check.
#[test]
fn a_second_assembly_implements_the_generated_interface_operations_and_attributes() {
    let roslyn = roslyn_or_skip!();
    let runtime = runtime_or_skip!();
    let ast = model(
        "exception Overrun { long limit; };\n\
         interface Base {\n\
         \x20   void g();\n\
         \x20   readonly attribute long counter;\n\
         };\n\
         interface Derived : Base {\n\
         \x20   long f(in long x) raises (Overrun);\n\
         \x20   attribute long memory;\n\
         };\n",
    );
    let library = library_source(&ast, key());
    assert!(
        library.contains("public interface Derived"),
        "the interface did not materialise:\n{library}"
    );
    assert!(
        library.contains("int Counter"),
        "the inherited readonly attribute is not a property:\n{library}"
    );
    assert!(
        library.contains("int Memory"),
        "the own attribute is not a property:\n{library}"
    );
    assert!(
        library.contains("Raises, as the IDL declares it: Overrun"),
        "the raises clause is not documented on `f`:\n{library}"
    );

    let consumer = "public sealed class Impl : global::Derived\n{\n\
                    \x20   private int memory_;\n\
                    \x20   public void G() { }\n\
                    \x20   public int Counter { get { return 42; } }\n\
                    \x20   public int Memory\n\
                    \x20   {\n\
                    \x20       get { return memory_; }\n\
                    \x20       set { memory_ = value; }\n\
                    \x20   }\n\
                    \x20   public int F(int x) { return x * 2; }\n\
                    }\n\n\
                    public static class Consumer\n{\n\
                    \x20   public static int Main()\n\
                    \x20   {\n\
                    \x20       global::Derived d = new Impl();\n\
                    \x20       d.G();\n\
                    \x20       if (d.Counter != 42) return 1;\n\
                    \x20       d.Memory = 7;\n\
                    \x20       if (d.Memory != 7) return 2;\n\
                    \x20       if (d.F(5) != 10) return 3;\n\
                    \x20       return 0;\n\
                    \x20   }\n\
                    }\n";
    passes(
        &roslyn,
        &runtime,
        &library,
        consumer,
        "interface_operations_and_attributes",
    );
}

// -- the keyword collision is two members ---------------------------------

/// A second assembly assigns and reads both members of the collision.
///
/// `keyword_collision.rs::the_csharp_column_escapes_out_of_reach`
/// asserts `Class;` and `Class_;` in the text. Two fields of one name
/// would not compile at all; one field would fail here on the name that
/// is not there. What this adds is that they are two *distinct*
/// storage locations, which no substring can say: a mapping that gave
/// both IDL members the same field would satisfy both substrings and
/// lose a value.
#[test]
fn a_second_assembly_reaches_both_members_of_a_keyword_collision() {
    let roslyn = roslyn_or_skip!();
    let runtime = runtime_or_skip!();
    let ast = model("@final struct Collide { long class; long class_; };");
    let body = "    public static int Main()\n\
        \x20   {\n\
        \x20       global::Collide sample = new global::Collide();\n\
        \x20       sample.Class = 1;\n\
        \x20       sample.Class_ = 2;\n\
        \x20       if (sample.Class != 1 || sample.Class_ != 2)\n\
        \x20       {\n\
        \x20           return Fail(\"one field holds both members: \"\n\
        \x20               + sample.Class + \" and \" + sample.Class_);\n\
        \x20       }\n\
        \x20       var w = NewWriter();\n\
        \x20       global::ZdCodec.zd_enc_Collide(w, sample);\n\
        \x20       var r = NewReader(w.Finish());\n\
        \x20       global::Collide back = global::ZdCodec.zd_dec_Collide(\n\
        \x20           r, global::ZeroDds.Wire.TryConstruct.Discard);\n\
        \x20       if (r.ReaderStatus() != global::ZeroDds.Wire.Status.Ok)\n\
        \x20       {\n\
        \x20           return Fail(\"the reader answered \" + r.ReaderStatus());\n\
        \x20       }\n\
        \x20       if (back.Class != 1 || back.Class_ != 2)\n\
        \x20       {\n\
        \x20           return Fail(\"the round trip gave \"\n\
        \x20               + back.Class + \" and \" + back.Class_);\n\
        \x20       }\n\
        \x20       return 0;\n\
        \x20   }\n";
    passes(
        &roslyn,
        &runtime,
        &library_source(&ast, key()),
        &consumer(key(), body),
        "keyword-collision",
    );
}

// -- the bound belongs to the bounded member ------------------------------

/// A bounded member and an unbounded one of the same element type, and
/// a stream that is over four in one of them.
///
/// `a_bound_belongs_to_the_hoisted_routine.rs` asserts this as a name
/// (`seq_b4_long`) and a count (`zd_over` twice). Both are properties
/// of the text; what a consumer can see is the behaviour they are
/// there for. The stream is built with the **unbounded** encoder for
/// both members, because a bound is not on the wire: the octets are
/// the same either way, and what differs is what the decoder does with
/// them.
///
/// Two directions, and both are needed: five in the bounded member is
/// `Status.Discarded` under `TryConstruct.Discard` (XTypes 1.3
/// §7.2.2.7), and five in the unbounded member beside it is
/// `Status.Ok` — which is what says the bound did not leak onto the
/// routine they would share if the hoist collapsed them.
fn bound_case(
    roslyn: &Roslyn,
    runtime: &Path,
    idl: &str,
    setup: &str,
    over_in_unbounded: &str,
    over_in_bounded: &str,
    tag: &str,
) {
    let ast = model(idl);
    let body = format!(
        "    public static int Main()\n\
         \x20   {{\n{setup}\
         \x20       var wide = NewWriter();\n{over_in_unbounded}\
         \x20       var r1 = NewReader(wide.Finish());\n\
         \x20       global::ZdCodec.zd_dec_S(r1, global::ZeroDds.Wire.TryConstruct.Discard);\n\
         \x20       if (r1.ReaderStatus() != global::ZeroDds.Wire.Status.Ok)\n\
         \x20       {{\n\
         \x20           return Fail(\"the unbounded member was refused: \" + r1.ReaderStatus());\n\
         \x20       }}\n\
         \x20       var over = NewWriter();\n{over_in_bounded}\
         \x20       var r2 = NewReader(over.Finish());\n\
         \x20       global::ZdCodec.zd_dec_S(r2, global::ZeroDds.Wire.TryConstruct.Discard);\n\
         \x20       if (r2.ReaderStatus() != global::ZeroDds.Wire.Status.Discarded)\n\
         \x20       {{\n\
         \x20           return Fail(\"the bounded member took it: \" + r2.ReaderStatus());\n\
         \x20       }}\n\
         \x20       return 0;\n\
         \x20   }}\n"
    );
    passes(
        roslyn,
        runtime,
        &library_source(&ast, key()),
        &consumer(key(), &body),
        tag,
    );
}

#[test]
fn a_second_assembly_sees_a_sequence_bound_on_the_bounded_member_only() {
    let roslyn = roslyn_or_skip!();
    let runtime = runtime_or_skip!();
    bound_case(
        &roslyn,
        &runtime,
        "@final struct S { sequence<long> b; sequence<long, 4> a; };",
        "",
        "        global::ZdCodec.zd_enc_seq_long(wide, new int[] { 1, 2, 3, 4, 5 });\n\
         \x20       global::ZdCodec.zd_enc_seq_long(wide, new int[] { 1, 2, 3 });\n",
        "        global::ZdCodec.zd_enc_seq_long(over, new int[] { 1, 2, 3 });\n\
         \x20       global::ZdCodec.zd_enc_seq_long(over, new int[] { 1, 2, 3, 4, 5 });\n",
        "bound-sequence",
    );
}

#[test]
fn a_second_assembly_sees_a_string_bound_on_the_bounded_member_only() {
    let roslyn = roslyn_or_skip!();
    let runtime = runtime_or_skip!();
    bound_case(
        &roslyn,
        &runtime,
        "@final struct S { string b; string<4> a; };",
        "",
        "        global::ZdCodec.zd_enc_string(wide, \"abcdefgh\");\n\
         \x20       global::ZdCodec.zd_enc_string(wide, \"abc\");\n",
        "        global::ZdCodec.zd_enc_string(over, \"abc\");\n\
         \x20       global::ZdCodec.zd_enc_string(over, \"abcdefgh\");\n",
        "bound-string",
    );
}

#[test]
fn a_second_assembly_sees_a_wstring_bound_on_the_bounded_member_only() {
    let roslyn = roslyn_or_skip!();
    let runtime = runtime_or_skip!();
    bound_case(
        &roslyn,
        &runtime,
        "@final struct S { wstring b; wstring<4> a; };",
        "",
        "        global::ZdCodec.zd_enc_wstring(wide, \"abcdefgh\");\n\
         \x20       global::ZdCodec.zd_enc_wstring(wide, \"abc\");\n",
        "        global::ZdCodec.zd_enc_wstring(over, \"abc\");\n\
         \x20       global::ZdCodec.zd_enc_wstring(over, \"abcdefgh\");\n",
        "bound-wstring",
    );
}

#[test]
fn a_second_assembly_sees_a_map_bound_on_the_bounded_member_only() {
    let roslyn = roslyn_or_skip!();
    let runtime = runtime_or_skip!();
    // The entry type is generated too, and a consumer that could not
    // name it could not build a map at all.
    let five = "        global::ZdEntry_long_long[] five = new global::ZdEntry_long_long[5];\n\
                \x20       for (int i = 0; i < 5; i = i + 1)\n\
                \x20       {\n\
                \x20           five[i] = new global::ZdEntry_long_long();\n\
                \x20           five[i].Key = i;\n\
                \x20           five[i].Value = i;\n\
                \x20       }\n\
                \x20       global::ZdEntry_long_long[] three = new global::ZdEntry_long_long[3];\n\
                \x20       for (int i = 0; i < 3; i = i + 1)\n\
                \x20       {\n\
                \x20           three[i] = new global::ZdEntry_long_long();\n\
                \x20           three[i].Key = i;\n\
                \x20           three[i].Value = i;\n\
                \x20       }\n";
    bound_case(
        &roslyn,
        &runtime,
        "@final struct S { map<long, long> b; map<long, long, 4> a; };",
        five,
        "        global::ZdCodec.zd_enc_map_long_long(wide, five);\n\
         \x20       global::ZdCodec.zd_enc_map_long_long(wide, three);\n",
        "        global::ZdCodec.zd_enc_map_long_long(over, three);\n\
         \x20       global::ZdCodec.zd_enc_map_long_long(over, five);\n",
        "bound-map",
    );
}

// -- the whole corpus's public surface ------------------------------------

/// Every type the file declares, as the name a consumer writes.
fn declared_types(items: &[csharp::Item], prefix: &str, out: &mut Vec<String>) {
    for item in items {
        match item {
            csharp::Item::Namespace(node) => {
                let segments: Vec<String> = node
                    .name
                    .segments()
                    .iter()
                    .map(|segment| segment.as_str().to_string())
                    .collect();
                let inner = if prefix.is_empty() {
                    segments.join(".")
                } else {
                    format!("{prefix}.{}", segments.join("."))
                };
                declared_types(&node.items, &inner, out);
            }
            csharp::Item::Type(decl) => {
                let name = decl.name().as_str();
                out.push(if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{prefix}.{name}")
                });
            }
            csharp::Item::Comment(_) | csharp::Item::Verbatim(_) => {}
        }
    }
}

/// The names of the codec's `public` methods.
///
/// The `private` ones are the Table 9 defaulters, which the codec calls
/// and a consumer does not: `zd_def_…` is emitted `private static` and
/// is not part of the surface either file asserts.
fn published_methods(members: &[csharp::Member]) -> Vec<String> {
    members
        .iter()
        .filter_map(|member| match member {
            csharp::Member::Method(method)
                if method.modifiers.contains(&csharp::Modifier::Public) =>
            {
                Some(method.name.as_str().to_string())
            }
            _ => None,
        })
        .collect()
}

/// A consumer that names every published type and every published
/// codec method of one case.
///
/// `nameof` and not a call: it is the one form that asks C# for a
/// member of any signature, and it is refused with `CS0122` for a
/// member the assembly does not publish, exactly as a call would be.
/// The reflection beside it asks the same of the metadata, with
/// `BindingFlags.Public` alone, so a member the compiler could see and
/// the assembly does not carry is caught too.
fn surface_consumer(key: LayoutKey, types: &[String], methods: &[String]) -> String {
    let mut body = String::from("    public static int Main()\n    {\n");
    for name in types {
        body.push_str(&format!(
            "        if (!typeof(global::{name}).IsPublic && \
             !typeof(global::{name}).IsNestedPublic)\n\
             \x20       {{\n\
             \x20           return Fail(\"{name} is not public\");\n\
             \x20       }}\n"
        ));
    }
    for name in methods {
        body.push_str(&format!(
            "        if (nameof(global::ZdCodec.{name}).Length == 0\n\
             \x20           || typeof(global::ZdCodec).GetMethod(\n\
             \x20               \"{name}\",\n\
             \x20               System.Reflection.BindingFlags.Public\n\
             \x20                   | System.Reflection.BindingFlags.Static) == null)\n\
             \x20       {{\n\
             \x20           return Fail(\"ZdCodec.{name} is not published\");\n\
             \x20       }}\n"
        ));
    }
    body.push_str("        return 0;\n    }\n");
    consumer(key, &body)
}
/// How many corpus cases have to reach the second compiler for this
/// gate to mean anything. `csharp_corpus_compiles.rs`'s own number, for
/// the same corpus.
const MUST_REACH: usize = 350;

/// Every corpus case, as two assemblies.
///
/// The counterpart of `csharp_corpus_compiles.rs`'s corpus sweep, one
/// question further on: not only that the case's C# compiles, but that
/// everything the column declares `public` is reachable by name from
/// outside the assembly it is compiled into.
///
/// `#[ignore]` for the same reason its sibling carries one: two Roslyn
/// starts and one process start per corpus case.
#[test]
#[ignore = "two Roslyn starts and a process start per corpus case; run with --include-ignored"]
fn every_corpus_case_publishes_its_surface_to_a_second_assembly() {
    // Explicitly selecting this expensive gate requires its toolchain and
    // real runtime compilation. Missing prerequisites cannot pass as a skip.
    let roslyn = roslyn().expect("selected C# consumer corpus requires the .NET SDK and Roslyn");
    let runtime = RUNTIME
        .as_ref()
        .expect("selected C# consumer corpus requires a successfully compiled runtime")
        .clone();
    let key = key();
    let mut failures = Vec::new();
    let mut backend_refused = Vec::new();
    let mut total = 0_usize;
    let mut frontend_refused = 0_usize;
    let mut reached = 0_usize;
    let mut passed = 0_usize;
    for (name, path) in corpus_harness::corpus() {
        total += 1;
        let idl = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&idl, &path.display().to_string()) {
            frontend_refused += 1;
            continue;
        }
        // The product frontend accepted the fixture. Every later refusal is
        // a backend coverage gap, not an invisible success or frontend skip.
        macro_rules! require_stage {
            ($stage:literal, $result:expr) => {
                match $result {
                    Ok(value) => value,
                    Err(error) => {
                        let refusal = format!("{name}: {}: {error:?}", $stage);
                        eprintln!("BACKEND_REFUSED {refusal}");
                        backend_refused.push(refusal);
                        continue;
                    }
                }
            };
        }
        let parsed = zerodds_idl_parse::parse(&idl);
        if !parsed.diagnostics().is_empty() {
            let refusal = format!("{name}: parse: {:?}", parsed.diagnostics());
            eprintln!("BACKEND_REFUSED {refusal}");
            backend_refused.push(refusal);
            continue;
        }
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), &path.display().to_string());
        if !lowered.diagnostics.is_empty() {
            let refusal = format!("{name}: lower: {:?}", lowered.diagnostics);
            eprintln!("BACKEND_REFUSED {refusal}");
            backend_refused.push(refusal);
            continue;
        }
        let mut ast = lowered.ast;
        require_stage!("resolve", ast.resolve_all_by_name());
        let graph = ModelGraph::of(&ast);
        let applied = require_stage!("apply", apply(&graph, &rule::csharp::catalogue()));
        let mut file = require_stage!("materialise", adapter::csharp::csharp_file(&ast, &applied));
        let options = codec_options();
        let encoders = require_stage!(
            "encoder",
            zerodds_lang_rules::emit::encode::codec::<CSharp>(&ast, key, options)
        );
        let decoders = require_stage!(
            "decoder",
            zerodds_lang_rules::emit::decode::decoder::<CSharp>(&ast, key, options)
        );
        let defaulters = require_stage!(
            "defaulters",
            zerodds_lang_rules::default_csharp::csharp_defaulters_called_by(&ast, &decoders)
        );
        let mut members = encoders;
        members.extend(defaulters);
        members.extend(decoders);

        assert!(
            adapter::csharp::drop_support_namespace(&mut file),
            "{name}: the materialiser emitted no `ZeroDds.Idl` namespace"
        );
        let mut types = Vec::new();
        declared_types(&file.items, "", &mut types);
        let methods = published_methods(&members);
        let library = format!(
            "{}\n\n{}",
            zerodds_lang_render::csharp::file(&file),
            codec_class(members)
        );

        reached += 1;
        match two_assemblies(
            &roslyn,
            &runtime,
            &library,
            &surface_consumer(key, &types, &methods),
            &name,
        ) {
            Ok(ran) if ran.code == 0 => passed += 1,
            Ok(ran) => failures.push(format!(
                "{name}: the consuming assembly answered {}:\n{}{}",
                ran.code, ran.stdout, ran.stderr
            )),
            Err(why) => failures.push(format!("{name}: {why}")),
        }
    }
    eprintln!(
        "CORPUS_SUMMARY total={total} frontend_refused={frontend_refused} \
         backend_refused={} reached={reached} passed={passed} failed={}",
        backend_refused.len(),
        failures.len()
    );
    assert_eq!(total, frontend_refused + backend_refused.len() + reached);
    assert!(
        backend_refused.is_empty(),
        "unreviewed backend refusals must not count as passing consumer cases:\n{backend_refused:#?}"
    );
    assert!(
        failures.is_empty(),
        "{} of {reached} did not publish their surface:\n{failures:#?}",
        failures.len()
    );
    assert!(
        reached >= MUST_REACH,
        "only {reached} corpus cases reached the second compiler; the gate says \
         nothing below {MUST_REACH}"
    );
}
