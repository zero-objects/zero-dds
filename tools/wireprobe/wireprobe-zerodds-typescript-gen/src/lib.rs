// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-typescript-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-typescript-gen` — the TypeScript translation unit
//! the TypeScript probe compiles.
//!
//! The TypeScript spelling and nothing else.
//! [`zerodds_wireprobe_gen`] is the stem: the command line, the IDL,
//! the model, the layout key, the sample, the files and the exit codes
//! are there, once, for all five columns.
//!
//! One `probe.ts` holding the declarations, the codec, and either the
//! sample with the statements that write it to standard output or the
//! statements that print one read back.
//! `tools/wireprobe/wireprobe-zerodds-typescript` is the shell around
//! it.
//!
//! One generated file and not two, which is where this differs from the
//! Java column's `ZdTypes.java` beside its `Probe.java`. Java needs the
//! split because a public class is a file; TypeScript has no such rule,
//! and a second file would need a second `import` line whose specifier
//! the module resolution has to agree with — one more thing between the
//! generator and the octets. The read direction's `zdj.ts` is not a
//! second generated file: the wrapper copies it verbatim, the way the
//! C# wrapper copies `Zdj.cs`, because the canonical-JSON dialect is
//! shared and exists once.
//!
//! Nothing here writes a byte. The declarations come from the TypeScript
//! materialiser, the codec from the shared walk through
//! [`zerodds_lang_rules::emit::typescript::TypeScript`], and the bytes
//! come out of `runtime/typescript` when the compiled program runs.
//! `runtime/typescript` implements DDS-XTypes 1.3 §7.4 from the clause
//! and calls nothing in Rust, which is what makes this a sixth reading
//! of the specification rather than a rendering of the first one's
//! answer.
//!
//! ## Both directions
//!
//! `--values` generates the write direction: the sample is filled from
//! the JSON and the octets go to standard output. `--read` generates the
//! read direction: the octets arrive on standard input and the values go
//! to standard output as the canonical JSON of
//! `crates/wire-probe/src/canonical.rs`, through the helpers in
//! `tools/wireprobe/zdj.ts`. The two are exclusive and the stem is what
//! refuses both at once.
//!
//! This column refused `--read` as `harness-gap` while
//! `emit/typescript.rs`'s `CodecTarget::default_into` had no
//! `default_typescript` behind it; `crates/lang-rules/src/default_typescript.rs`
//! supplies Table 9 now and the refusal with its reason is gone with it.
#![forbid(unsafe_code)]

mod dump;
mod fill;

use std::fmt::Write as _;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Endianness, KeyHolderNesting, LayoutKey, PidFlagPolicy, Purpose};
use zerodds_lang_rules::default_typescript::typescript_defaulters_called_by;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{codec_from, codec_name, key_holder};
use zerodds_lang_rules::emit::typescript::TypeScript;
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};
use zerodds_wireprobe_gen::{Column, Emit, ExtraOption, File, Refusal, Session, gap};

/// The module specifier a generated file imports the runtime from.
///
/// Relative and with a `.js` extension because the tree compiles under
/// `\"module\": \"NodeNext\"`, where a relative specifier names the file
/// that will exist at run time and not the one the compiler reads.
const DEFAULT_RUNTIME: &str = "./wire/index.js";

/// The flag that overrides it.
const RUNTIME_FLAG: &str = "--runtime";

/// The TypeScript column.
#[derive(Debug, Clone, Copy)]
pub struct Renderer;

impl Column for Renderer {
    const BINARY: &'static str = "wireprobe-zerodds-typescript-gen";
    const LANGUAGE: &'static str = "TypeScript";
    const OUTPUT: &'static str = "probe.ts goes";
    const FORMS: &'static [Emit] = &[Emit::Payload, Emit::PayloadWithEncapsulation, Emit::KeyHash];
    const EXTRA: &'static [ExtraOption] = &[ExtraOption {
        flag: RUNTIME_FLAG,
        value: "<specifier>",
        help: &[
            "the module specifier the generated file",
            "imports the runtime from; the default is",
            "./wire/index.js, which is where the probe",
            "puts it",
        ],
    }];

    fn render(session: &Session<'_>) -> Result<Vec<File>, Refusal> {
        Ok(vec![File::new("probe.ts", source(session)?)])
    }
}

/// Generate `probe.ts`.
///
/// # Errors
/// [`Refusal`], classified so the command can answer in the verdict's
/// own exit code.
pub fn source(session: &Session<'_>) -> Result<String, Refusal> {
    let ast = &session.ast;
    let graph = ModelGraph::of(ast);
    let applied = apply(&graph, &rule::typescript::catalogue())
        .map_err(|error| gap!("the TypeScript rules do not apply: {error:?}"))?;
    // The materialiser is the authority on what TypeScript has a form
    // for, so its refusal is exit 3 and every other stop is exit 6.
    let declarations = zerodds_lang_rules::adapter::typescript::ts_items(ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    // What the chosen direction calls, and nothing else. A function the
    // probe never reaches is dead source, and generating both halves
    // would double the compile of every case in the corpus sweep.
    let codec = if session.args.read {
        // The Table 9 defaulters first, so a reader reads in the order
        // it is built: `zerodds_lang_rules::decode_c` prepends C's in
        // the same place and for the same reason.
        let decoders =
            decoder_from::<TypeScript>(ast, session.root, session.key, session.codec_options)
                .map_err(|error| from_codec(&error))?;
        let mut all =
            typescript_defaulters_called_by(ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else if session.args.emit == Emit::KeyHash {
        key_hash_codec(session)?
    } else {
        codec_from::<TypeScript>(ast, session.root, session.key, session.codec_options)
            .map_err(|error| from_codec(&error))?
    };

    let mut decls = zerodds_lang_rules::adapter::typescript::prelude();
    decls.extend(declarations);
    decls.extend(codec);

    let mut imports = vec![runtime_import(
        session.args.extra_or(RUNTIME_FLAG, DEFAULT_RUNTIME),
    )];
    if session.args.read {
        // The canonical-JSON helpers, the same contract `Zdj.cs`,
        // `Zdj.java` and `dumpjson.h` carry: the JSON dialect is shared
        // and not a wire rule, and two backends holding two copies of
        // it is how they come to disagree about what a sample is. The
        // shell wrapper puts `zdj.ts` beside the probe.
        imports.push(zdj_import());
    }

    let file = zerodds_lang_cst::typescript::File {
        doc: zerodds_lang_cst::common::Doc::lines(&[
            "Generated by wireprobe-zerodds-typescript-gen. Do not edit.",
            "",
            "The bytes and the values come out of runtime/typescript when",
            "this runs. Nothing here decides a wire question.",
        ]),
        imports,
        decls,
    };
    let mut out = zerodds_lang_render::typescript::file(&file);
    out.push_str(&if session.args.read {
        read_harness(session)?
    } else if session.args.emit == Emit::KeyHash {
        key_hash_harness(session)?
    } else {
        harness(session)?
    });
    Ok(out)
}

/// The command, for `main`.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `zerodds_wireprobe_gen::cli_main`, a different function in
/// another crate; `dds_bounded_recursion` compares only the last
/// segment of a call path, so the delegation reads as a self-call.
pub fn cli_main() {
    zerodds_wireprobe_gen::cli_main::<Renderer>();
}

/// Everything the generated file names from the runtime.
///
/// Imported wholesale rather than per case, for the reason the Java
/// probe imports wholesale: which policy a configuration reaches is the
/// layout key's answer, and a file that imported only what one key
/// needed would not compile for another. `tsc` has no unused-import
/// diagnostic under the settings this tree compiles with, and an unused
/// import of a `type` is erased entirely.
fn runtime_import(module: &str) -> zerodds_lang_cst::typescript::Import {
    use zerodds_lang_cst::common::Ident;
    use zerodds_lang_cst::typescript::{Import, ImportName};
    let names = [
        ("Endianness", false),
        ("FixedDigitCount", false),
        ("ParameterRole", false),
        ("PID_FLAGS_BARE", false),
        ("PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS", false),
        ("PID_FLAGS_SENTINEL_MUST_UNDERSTAND", false),
        ("PID_SHORT_MAX_ID", false),
        ("TryConstruct", false),
        ("WcharFraming", false),
        ("WcharWidth", false),
        ("Writer", false),
        ("WstringLengthPolicy", false),
        ("WstringUnitOrder", false),
        ("float128FromDecimalString", false),
        ("noDheader", false),
        // The union-overlay patterns `emit/encode.rs` reads a
        // suppressed arm's bits through.
        ("bitPatternBool", false),
        ("bitPatternF32", false),
        ("bitPatternF64", false),
        ("bitPatternInt", false),
        // The three holder conversions `emit/typescript.rs`'s `cast`
        // writes. `as` between `number` and `bigint` is TS2352 — the
        // two do not overlap — so a narrowing the walk asks for is a
        // call, and a bitmask, a bitset field or an enumerator reaches
        // one on every case that has them.
        ("zdBigInt", false),
        ("zdChar", false),
        ("zdNumber", false),
        // The read direction's names: the reader the harness constructs,
        // the status its one `if` at the top reads, and the measures a
        // generated decoder holds a `@try_construct` bound against.
        // Unused in a write probe, which is the wholesale-import
        // argument above.
        ("Reader", false),
        // Aliased on import: `Status` is the one runtime export whose
        // name an IDL declaration in the corpus also carries
        // (`enum-sparse-values` declares `enum Status`), and the probe
        // and the type live in one file — the bare import is TS2440
        // against the generated declaration. The corpus holds no other
        // collision with this list (measured against every `type` and
        // every declaration in matrix/cases, 2026-08-25); a new one
        // fails loudly here as the same TS2440.
        ("Status", false),
        ("pidIsUserMember", false),
        ("utf8Octets", false),
        ("utf16Units", false),
        // The handles the generated codec declares locals of. `type`
        // bindings, because they have no run-time form at all and an
        // `import` of one under `verbatimModuleSyntax` is an error.
        ("DHeader", true),
        ("EmHeader", true),
        ("Pid", true),
        ("PlCdr1Framing", true),
        ("EmHeaderRead", true),
        ("PidRead", true),
        ("ReaderMark", true),
    ];
    Import {
        module: module.to_string(),
        type_only: false,
        default: None,
        names: names
            .into_iter()
            .map(|(name, type_only)| ImportName {
                name: Ident::new(name),
                alias: (name == "Status").then(|| Ident::new("ZdReaderStatus")),
                type_only,
            })
            .collect(),
        namespace: None,
    }
}

/// The canonical-JSON helpers, from the file the wrapper copies beside
/// the probe.
///
/// Relative and with a `.js` extension for the reason
/// [`DEFAULT_RUNTIME`] is: under `"module": "NodeNext"` a relative
/// specifier names the file that will exist at run time.
fn zdj_import() -> zerodds_lang_cst::typescript::Import {
    use zerodds_lang_cst::common::Ident;
    use zerodds_lang_cst::typescript::{Import, ImportName};
    Import {
        module: "./zdj.js".to_string(),
        type_only: false,
        default: None,
        names: vec![ImportName {
            name: Ident::new("Zdj"),
            alias: None,
            type_only: false,
        }],
        namespace: None,
    }
}

/// The tail of `probe.ts` for the read direction: bytes on standard
/// input, canonical JSON on standard output.
///
/// Standard input and not a path baked into the generated TypeScript:
/// the path would be part of the program, so a probe compiled for one
/// payload could not be run against another, and the shell wrapper
/// would have to regenerate for every byte string it wanted read.
fn read_harness(session: &Session<'_>) -> Result<String, Refusal> {
    let args = session.args;
    let ast: &Ast = &session.ast;
    let root: Id<TypeNode> = session.root;
    let key: LayoutKey = session.key;
    let sample_type = zerodds_lang_render::typescript::type_(
        &<TypeScript as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let decoder = decoder_name::<TypeScript>(ast, root);
    let order = match args.endianness {
        Endianness::Little => "Endianness.Little",
        Endianness::Big => "Endianness.Big",
    };
    let framing = framing_expression(key);
    let max_align = args.encoding.max_alignment();
    let dump = dump::dump(ast, root, key.union_label)?;

    let mut out = String::new();
    out.push_str(
        "\n/**\n\
         \x20* The two streams and the exit code, as the only things this\n\
         \x20* program uses Node for.\n\
         \x20*\n\
         \x20* Declared here and not imported, for the reason the write harness\n\
         \x20* gives: `runtime/typescript` depends on nothing from Node and the\n\
         \x20* probe compiles without `@types/node`. Standard input is read\n\
         \x20* through the stream's own async iteration rather than through\n\
         \x20* `node:fs`, because an import of a module no installed declaration\n\
         \x20* describes is TS2307 — the stream needs only the one shape stated\n\
         \x20* here.\n\
         \x20*/\n\
         declare const process: {\n\
         \x20 readonly stdin: AsyncIterable<Uint8Array>;\n\
         \x20 readonly stdout: { write(chunk: Uint8Array): boolean };\n\
         \x20 readonly stderr: { write(chunk: string): boolean };\n\
         \x20 exit(code: number): never;\n\
         };\n\n",
    );
    let _ = write!(out, "const zdFraming: PlCdr1Framing = {framing};\n\n");
    if !dump.functions.is_empty() {
        out.push_str(
            "// The types on a cycle, each printed by a function of its own: an\n\
             // inline expansion of a cycle has no end.\n",
        );
        out.push_str(&dump.functions);
        out.push('\n');
    }
    out.push_str(
        "// Everything on standard input. A payload is one message and the\n\
         // probe is handed the whole of it.\n\
         const zdChunks: Uint8Array[] = [];\n\
         let zdTotal = 0;\n\
         for await (const zdChunk of process.stdin) {\n\
         \x20 zdChunks.push(zdChunk);\n\
         \x20 zdTotal += zdChunk.length;\n\
         }\n\
         const zdInput = new Uint8Array(zdTotal);\n\
         {\n\
         \x20 let zdFill = 0;\n\
         \x20 for (const zdChunk of zdChunks) {\n\
         \x20   zdInput.set(zdChunk, zdFill);\n\
         \x20   zdFill += zdChunk.length;\n\
         \x20 }\n\
         }\n\
         let zdAt = 0;\n\
         let zdLen = zdInput.length;\n",
    );
    if args.encapsulation() {
        // The encapsulation identifier is not re-read for the encoding:
        // the caller named one, and a probe that took the stream's word
        // for it could not be pointed at a stream to test what it does
        // with a header it disagrees with.
        //
        // The **options** word is read, because §7.6.3.1.2 says a
        // receiver shall use its low two bits to find where the
        // serialized data ended. Without them the trailing padding is
        // handed to the decoder as data.
        out.push_str(
            "if (zdLen < 4) {\n\
             \x20 process.stderr.write(\"wireprobe-zerodds-typescript: fewer than the four-byte encapsulation header\\n\");\n\
             \x20 process.exit(1);\n\
             }\n\
             const zdDeclared = (zdInput[3] ?? 0) & 3;\n\
             zdAt += 4;\n\
             zdLen -= 4;\n\
             if (zdLen < zdDeclared) {\n\
             \x20 process.stderr.write(`wireprobe-zerodds-typescript: the options field declares ${zdDeclared} padding byte(s) and only ${zdLen} follow the header\\n`);\n\
             \x20 process.exit(1);\n\
             }\n\
             zdLen -= zdDeclared;\n",
        );
    }
    let _ = write!(
        out,
        "const zdBody = zdInput.subarray(zdAt, zdAt + zdLen);\n\
         const r = new Reader(zdBody, {order}, {max_align}, zdFraming);\n\
         let zdSample: {sample_type};\n\
         try {{\n\
         \x20 // The top-level type is not a member of anything, so no\n\
         \x20 // `@try_construct` governs it and §7.2.2.7's default stands.\n\
         \x20 zdSample = {decoder}(r, TryConstruct.Discard);\n\
         }} catch (error) {{\n\
         \x20 const message = error instanceof Error ? error.message : String(error);\n\
         \x20 process.stderr.write(`wireprobe-zerodds-typescript: ${{message}}\\n`);\n\
         \x20 process.exit(1);\n\
         }}\n\
         if (r.readerStatus() === ZdReaderStatus.Discarded) {{\n\
         \x20 // Not a failed decode. DDS-XTypes 1.3 §7.2.2.7: the receiver\n\
         \x20 // drops the object and carries on, and a caller that cannot\n\
         \x20 // tell that from a broken peer cannot tell a narrowed type\n\
         \x20 // from a corrupt stream. Exit code 3, the Rust probe's.\n\
         \x20 process.stderr.write(`wireprobe-zerodds-typescript: at ${{r.readerPosition()}}: Discarded\\n`);\n\
         \x20 process.exit(3);\n\
         }}\n\
         if (r.readerStatus() !== ZdReaderStatus.Ok) {{\n\
         \x20 process.stderr.write(`wireprobe-zerodds-typescript: at ${{r.readerPosition()}}: ${{r.readerStatus()}}\\n`);\n\
         \x20 process.exit(1);\n\
         }}\n\
         // Nothing may be left over that is not padding. A decoder that\n\
         // stopped short and said nothing is the silent-drop shape: the\n\
         // sample comes back looking plausible with its last members\n\
         // missing. Padding is the one thing a writer legitimately leaves\n\
         // behind, and it is what alignment asks for from this offset and\n\
         // never more.\n\
         const zdLeft = r.readerRemaining();\n\
         const zdPad = ({max_align} - (r.readerPosition() % {max_align})) % {max_align};\n\
         if (zdLeft > zdPad) {{\n\
         \x20 process.stderr.write(`wireprobe-zerodds-typescript: ${{zdLeft}} byte(s) left at offset ${{r.readerPosition()}}, of which at most ${{zdPad}} could be padding\\n`);\n\
         \x20 process.exit(1);\n\
         }}\n"
    );
    out.push_str(&dump.body);
    out.push_str(
        "Zdj.flush();\n\
         // No `process.exit` after the flush: standard output is a pipe here\n\
         // and Node's write to a pipe is asynchronous, so exiting would\n\
         // truncate the values. The process ends when the write has drained.\n",
    );
    Ok(out)
}

/// The tail of `probe.ts`: the sample, the writer, and standard output.
/// The key holder and the codec it calls: the C column's KeyHash run in
/// TypeScript, planned under the holder's own layout key, plus a
/// data-planned set under the dialect that writes a keyless nested
/// aggregate whole. A type with no `@key` member has one instance and
/// no KeyHash, and every column answers that the same way.
fn key_hash_codec(
    session: &Session<'_>,
) -> Result<Vec<zerodds_lang_cst::typescript::Decl>, Refusal> {
    let ast = &session.ast;
    let root = session.root;
    let holder_key = session.holder_key;
    let mut decls = codec_from::<TypeScript>(ast, root, holder_key, session.codec_options)
        .map_err(|error| from_codec(&error))?;
    if holder_key.key_holder_nesting == KeyHolderNesting::KeylessNestedWhole {
        let as_data = LayoutKey {
            purpose: Purpose::Data,
            ..holder_key
        };
        decls.extend(
            codec_from::<TypeScript>(ast, root, as_data, session.codec_options)
                .map_err(|error| from_codec(&error))?,
        );
    }
    let Some(holder) =
        key_holder::<TypeScript>(ast, root, holder_key).map_err(|error| from_codec(&error))?
    else {
        return Err(gap!("{} declares no @key member", session.args.type_name));
    };
    decls.push(holder);
    Ok(decls)
}

/// The harness of a KeyHash run: the sample, its key holder written
/// big-endian under PLAIN_CDR2's alignment cap (XTypes 1.3 §7.6.8), and
/// the sixteen octets DDSI-RTPS 2.5 §9.6.3.8 makes of it. A value the
/// type cannot hold is refused with exit 1, as in the payload run.
fn key_hash_harness(session: &Session<'_>) -> Result<String, Refusal> {
    let ast: &Ast = &session.ast;
    let root: Id<TypeNode> = session.root;
    let sample_type = zerodds_lang_render::typescript::type_(
        &<TypeScript as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let filled = fill::fill(ast, root, session.sample()?)?;
    let holder = format!(
        "zd_keyholder_{}",
        <TypeScript as zerodds_lang_rules::emit::CodecTarget>::type_name(ast, root)
    );
    let form = match zerodds_ast_layout::keyhash::key_hash_form_for(ast, root, session.holder_key)
        .map_err(|error| gap!("key-holder maximum size: {error}"))?
    {
        zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "zeroPadded",
        zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "md5",
    };
    let mut out = String::new();
    out.push_str(
        "\n/** The two streams and the exit code; see the payload harness. */\n\
         declare const process: {\n\
         \x20 readonly stdout: { write(chunk: Uint8Array): boolean };\n\
         \x20 readonly stderr: { write(chunk: string): boolean };\n\
         \x20 exit(code: number): never;\n\
         };\n\n",
    );
    let _ = write!(out, "const zdSample: {sample_type} = {filled};\n\n");
    let _ = write!(
        out,
        "// The holder is PLAIN_CDR2 big-endian whatever the sample is, so its\n\
         // alignment cap is PLAIN_CDR2's. XTypes 1.3 7.6.8.\n\
         const zdWriter = new Writer(Endianness.Big, 4);\n\
         try {{\n\
         \x20 {holder}(zdWriter, zdSample);\n\
         }} catch (error) {{\n\
         \x20 const message = error instanceof Error ? error.message : String(error);\n\
         \x20 process.stderr.write(`wireprobe-zerodds-typescript: ${{message}}\\n`);\n\
         \x20 process.exit(1);\n\
         }}\n\
         process.stdout.write(zdWriter.{form}());\n"
    );
    Ok(out)
}

fn harness(session: &Session<'_>) -> Result<String, Refusal> {
    let args = session.args;
    let ast: &Ast = &session.ast;
    let root: Id<TypeNode> = session.root;
    let key: LayoutKey = session.key;
    let sample_type = zerodds_lang_render::typescript::type_(
        &<TypeScript as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let filled = fill::fill(ast, root, session.sample()?)?;
    let encoder = codec_name::<TypeScript>(ast, root, Purpose::Data);
    let order = match args.endianness {
        Endianness::Little => "Endianness.Little",
        Endianness::Big => "Endianness.Big",
    };
    let framing = framing_expression(key);
    let short_codes = key.length_codes.short_code_where_it_fits();
    let max_align = args.encoding.max_alignment();

    let mut out = String::new();
    out.push_str(
        "\n/**\n\
         \x20* The two streams and the exit code, as the only things this\n\
         \x20* program uses Node for.\n\
         \x20*\n\
         \x20* Declared here and not imported: `runtime/typescript` depends on\n\
         \x20* nothing from Node because it has to run in a browser too, and a\n\
         \x20* `@types/node` in the compile would say otherwise. The probe is\n\
         \x20* not the runtime and may use the host it runs on.\n\
         \x20*/\n\
         declare const process: {\n\
         \x20 readonly stdout: { write(chunk: Uint8Array): boolean };\n\
         \x20 readonly stderr: { write(chunk: string): boolean };\n\
         \x20 exit(code: number): never;\n\
         };\n\n",
    );
    let _ = write!(out, "const zdSample: {sample_type} = {filled};\n\n");
    let _ = write!(out, "const zdFraming: PlCdr1Framing = {framing};\n\n");
    let _ = write!(
        out,
        "const zdWriter = new Writer({order}, {max_align}, zdFraming, {short_codes});\n\
         // A value the type cannot hold is the sample's refusal and not the\n\
         // harness's: `char above U+00FF` is §7.2.2.2.1.2.2, and the Rust\n\
         // probe answers 1 for the same sample. Exit 6 here would file it as\n\
         // `harness-gap` — a tool of ours that has not been written — which\n\
         // is a different thing from a runtime that read the clause and said\n\
         // no. `docs/specs/zerodds-verdicts-1.0.md`.\n\
         try {{\n\
         \x20 {encoder}(zdWriter, zdSample);\n\
         }} catch (error) {{\n\
         \x20 const message = error instanceof Error ? error.message : String(error);\n\
         \x20 process.stderr.write(`wireprobe-zerodds-typescript: ${{message}}\\n`);\n\
         \x20 process.exit(1);\n\
         }}\n\
         const zdBody = zdWriter.finish();\n"
    );
    if args.encapsulation() {
        let header = session.encapsulation_header();
        // §7.4.1.1 pads the serialized data to the next four-octet
        // boundary with zeros, and §7.6.3.1.2 puts the count of those
        // octets in the low two bits of the encapsulation options so a
        // receiver can find where the data ended. Both halves or
        // neither: padding without the count hands a conforming
        // receiver up to three octets of zero as data, and the count
        // without the padding tells it to cut octets that are data.
        //
        // This column wrote neither, and said in a comment that the
        // caller would write the count — no caller does. Measured on
        // `primitive-octet` under plain-cdr2, little-endian: C, C# and
        // Java write `00070002 ab01 0000` and this wrote
        // `00070000 ab01`. The Rust column —
        // `wire-codec/src/encode.rs`, `Emit::PayloadWithEncapsulation`
        // — and the Python column agree with the three.
        //
        // `--emit payload` stays unpadded: that is the serialized data
        // itself, and it is what the ledger records.
        let _ = write!(
            out,
            "// The encapsulation identifier §7.6.3.1.2 gives this encoding and\n\
             // byte order, with the padding count in the low two bits of the\n\
             // options field and the padding itself behind the payload.\n\
             const zdPad = (4 - (zdBody.length & 3)) & 3;\n\
             const zdHead = Uint8Array.from([0x{:02x}, 0x{:02x}, 0x{:02x}, 0x{:02x} | zdPad]);\n\
             process.stdout.write(zdHead);\n",
            header[0], header[1], header[2], header[3]
        );
    }
    out.push_str(
        "// No `process.exit` after the write: standard output is a pipe here\n\
         // and Node's write to a pipe is asynchronous, so exiting would\n\
         // truncate the payload. The process ends when the write has drained.\n\
         process.stdout.write(zdBody);\n",
    );
    if args.encapsulation() {
        out.push_str("process.stdout.write(new Uint8Array(zdPad));\n");
    }
    Ok(out)
}

/// The PL_CDR1 framing one layout key adds up to, as the TypeScript
/// expression that builds it.
///
/// The flag words are named and not computed here: which bits ride on a
/// parameter id at which position is a measured divergence between
/// implementations, and `runtime/typescript` is where this column keeps
/// its answer. A generator that wrote `0x4000` would have moved a wire
/// decision out of the runtime and into the Rust tree, which is the one
/// thing a second column may not do.
fn framing_expression(key: LayoutKey) -> String {
    let headers = key.pl_cdr1_headers;
    format!(
        "{{\n\
         \x20 pidFlags: {},\n\
         \x20 shortMaxId: {},\n\
         \x20 padsDeclaredLength: {},\n\
         \x20 padsOptionalLength: {},\n\
         \x20 memberMustUnderstand: {},\n\
         \x20 padAfterValue: {},\n\
         }}",
        match key.pid_flags {
            PidFlagPolicy::SentinelMustUnderstand => "PID_FLAGS_SENTINEL_MUST_UNDERSTAND",
            PidFlagPolicy::Bare => "PID_FLAGS_BARE",
            PidFlagPolicy::ImplExtensionOnMembers => "PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS",
        },
        if key.short_pid_range.highest_short_id() == 0x3f00 {
            "PID_SHORT_MAX_ID".to_string()
        } else {
            format!("{}", key.short_pid_range.highest_short_id())
        },
        headers.pads_declared_length(),
        headers.pads_declared_optional_length(),
        key.member_pid_carries_must_understand(),
        headers.pads_after_value()
    )
}

/// The verdict a codec error answers in.
///
/// One place per generator, reading `EncodeError::is_spec_refusal`: a
/// clause of the specification is `refused` (exit 1) and everything else
/// is this tree's own gap (exit 6).
fn from_codec(error: &EncodeError) -> Refusal {
    if error.is_spec_refusal() {
        Refusal::Refused(format!("{error}"))
    } else {
        Refusal::Gap(format!("{error}"))
    }
}
