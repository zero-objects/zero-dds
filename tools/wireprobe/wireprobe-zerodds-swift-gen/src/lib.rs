// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-swift-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-swift-gen` — the Swift program the Swift probe
//! builds and runs.
//!
//! One IDL file, one type, one JSON sample and one layout key in; one
//! `main.swift` holding the types, the codec, the sample and the
//! harness that writes the octets to standard output — or, with
//! `--in`, reads them back and writes the values as JSON where
//! `--values-out` says. `tools/wireprobe/wireprobe-zerodds-swift` is
//! the shell around it, and compiles `main.swift` together with
//! `runtime/swift/Sources/ZdWire/*.swift` as one module — which is why
//! the generated file needs no import to name `ZdWriter`.
//!
//! Nothing here writes a byte. The declarations come from the Swift
//! materialiser, the codec from the shared walk through
//! [`zerodds_lang_rules::emit::swift::Swift`], and the bytes come out
//! of `runtime/swift` when the program runs — a further reading of
//! DDS-XTypes 1.3 §7.4, calling nothing in Rust.
//!
//! What `--values-out` receives is plain JSON and the caller
//! canonicalises it in Rust, which is what the sibling columns do.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

mod dump;
mod fill;

use std::fmt::Write as _;
use std::path::PathBuf;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::default_swift::swift_defaulters_called_by;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::emit::swift::Swift;
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

const USAGE: &str = "\
wireprobe-zerodds-swift-gen — the Swift the ZeroDDS Swift probe builds and runs

    --idl <file>            the IDL source (required)
    --type <ScopedName>     the top-level type (required)
    --values <file>         the sample, as JSON (required, unless --in)
    --in <file>             bytes to read back instead of writing
    --values-out <file>     where the values read go (required with --in)
    --encoding <name>       xcdr1 | xcdr2 | plain-cdr1 | pl-cdr1 |
                            plain-cdr2 | delimited-cdr | pl-cdr2
    --endian le|be
    --emit <form>           payload | payload+encapsulation
    --dialect <name>        specification (the default) | cyclone | rti |
                            fastdds | opendds | ros-fastrtps |
                            ros-cyclone | omniorb | tao | jacorb
    --out-dir <dir>         where main.swift goes (required)

The defaults are `wireprobe-zerodds`'s, because a Swift column that
defaulted differently would report a divergence of its own making.
";

/// What the generator was asked for.
#[derive(Debug, Clone)]
pub struct Args {
    /// The IDL source.
    pub idl: PathBuf,
    /// The top-level type's scoped name.
    pub type_name: String,
    /// The sample to write. `None` in the read direction.
    pub values: Option<PathBuf>,
    /// The bytes to read. `None` in the write direction.
    pub input: Option<PathBuf>,
    /// Where the values read go.
    pub values_out: Option<PathBuf>,
    /// The encoding.
    pub encoding: Encoding,
    /// The byte order.
    pub endianness: Endianness,
    /// Whether the encapsulation header is written, or expected.
    pub encapsulation: bool,
    /// The peer whose answers to every wire question this reproduces.
    pub dialect: Dialect,
    /// Where the file goes.
    pub out_dir: PathBuf,
}

/// Why the generator could not produce a program, classified so the
/// probe's exit code is the verdict `docs/specs/zerodds-verdicts-1.0.md`
/// records.
#[derive(Debug)]
pub enum Refusal {
    /// The model is outside what the specification admits, in the
    /// planner's own words. Exit 1 — `refused` in
    /// `docs/specs/zerodds-verdicts-1.0.md`, "it was asked and said no".
    Refused(String),
    /// The command line. Exit 2.
    Usage(String),
    /// Swift has no form for the construct. Exit 3. The materialiser's
    /// answer and nobody else's.
    NoForm(String),
    /// A tool of ours has not written it. Exit 6.
    Gap(String),
}

impl Refusal {
    /// The exit code this refusal answers in.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::Refused(_) => 1,
            Self::Usage(_) => 2,
            Self::NoForm(_) => 3,
            Self::Gap(_) => 6,
        }
    }

    /// The message, for standard error.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Refused(text) | Self::Usage(text) | Self::NoForm(text) | Self::Gap(text) => text,
        }
    }
}

/// Parse a command line.
///
/// # Errors
/// [`Refusal::Usage`], naming the option, or the usage text when
/// `--help` is given.
#[allow(clippy::too_many_lines)]
pub fn parse(argv: Vec<String>) -> Result<Args, Refusal> {
    let mut idl = None;
    let mut type_name = None;
    let mut values = None;
    let mut input = None;
    let mut values_out = None;
    let mut encoding = Encoding::PlainCdr2;
    let mut endianness = Endianness::Little;
    let mut encapsulation = false;
    let mut dialect = Dialect::Specification;
    let mut out_dir = None;

    let mut rest = argv.into_iter().skip(1);
    while let Some(flag) = rest.next() {
        let mut take = || {
            rest.next()
                .ok_or_else(|| Refusal::Usage(format!("{flag}: the option takes a value")))
        };
        match flag.as_str() {
            "--help" | "-h" => return Err(Refusal::Usage(USAGE.to_string())),
            "--idl" => idl = Some(PathBuf::from(take()?)),
            "--type" => type_name = Some(take()?),
            "--values" => values = Some(PathBuf::from(take()?)),
            "--in" => input = Some(PathBuf::from(take()?)),
            "--values-out" => values_out = Some(PathBuf::from(take()?)),
            "--out-dir" => out_dir = Some(PathBuf::from(take()?)),
            "--encoding" => {
                let name = take()?;
                encoding = match name.as_str() {
                    "xcdr1" | "plain-cdr1" => Encoding::PlainCdr1,
                    "pl-cdr1" => Encoding::PlCdr1,
                    "xcdr2" | "plain-cdr2" => Encoding::PlainCdr2,
                    "delimited-cdr" => Encoding::DelimitedCdr,
                    "pl-cdr2" => Encoding::PlCdr2,
                    other => {
                        return Err(Refusal::Usage(format!(
                            "--encoding: `{other}` is not one of them"
                        )));
                    }
                };
            }
            "--endian" => {
                endianness = match take()?.as_str() {
                    "le" => Endianness::Little,
                    "be" => Endianness::Big,
                    other => {
                        return Err(Refusal::Usage(format!(
                            "--endian: `{other}` is not le or be"
                        )));
                    }
                };
            }
            "--emit" => {
                encapsulation = match take()?.as_str() {
                    "payload" => false,
                    "payload+encapsulation" => true,
                    other => {
                        return Err(Refusal::Usage(format!(
                            "--emit: `{other}` — this column writes payload or \
                             payload+encapsulation"
                        )));
                    }
                };
            }
            "--dialect" => {
                let name = take()?;
                dialect = Dialect::ALL
                    .into_iter()
                    .find(|candidate| candidate.as_str() == name)
                    .ok_or_else(|| {
                        Refusal::Usage(format!("--dialect: `{name}` is not one of them"))
                    })?;
            }
            other => return Err(Refusal::Usage(format!("`{other}` is not an option"))),
        }
    }

    if input.is_some() && values.is_some() {
        return Err(Refusal::Usage(
            "--in reads bytes and --values writes them; pick one".to_string(),
        ));
    }
    if input.is_none() && values.is_none() {
        return Err(Refusal::Usage(
            "--values is required, or --in to read bytes back".to_string(),
        ));
    }
    if input.is_some() && values_out.is_none() {
        return Err(Refusal::Usage(
            "--values-out is required with --in".to_string(),
        ));
    }

    Ok(Args {
        idl: idl.ok_or_else(|| Refusal::Usage("--idl is required".to_string()))?,
        type_name: type_name.ok_or_else(|| Refusal::Usage("--type is required".to_string()))?,
        values,
        input,
        values_out,
        encoding,
        endianness,
        encapsulation,
        dialect,
        out_dir: out_dir.ok_or_else(|| Refusal::Usage("--out-dir is required".to_string()))?,
    })
}

/// Generate `main.swift`.
///
/// # Errors
/// [`Refusal`], classified so the probe can answer in the verdict's own
/// exit code.
pub fn source(args: &Args) -> Result<String, Refusal> {
    let text = std::fs::read_to_string(&args.idl)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", args.idl.display())))?;
    let ast = model_from_idl(&text, &args.idl.to_string_lossy(), args.dialect)?;
    let root = find_type(&ast, &args.type_name)?;

    let sample = match &args.values {
        Some(path) => {
            let sample_text = std::fs::read_to_string(path)
                .map_err(|error| Refusal::Gap(format!("{}: {error}", path.display())))?;
            Some(
                serde_json::from_str::<serde_json::Value>(&sample_text)
                    .map_err(|error| Refusal::Gap(format!("{}: {error}", path.display())))?,
            )
        }
        None => None,
    };

    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::swift::catalogue())
        .map_err(|error| Refusal::Gap(format!("the Swift rules do not apply: {error:?}")))?;
    // The materialiser is the authority on what Swift has a form for,
    // so its refusal is exit 3 and every other stop is exit 6.
    let items = zerodds_lang_rules::adapter::swift::swift_items(&ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    let key = layout_key(args);
    let codec = if args.input.is_some() {
        let decoders = decoder_from::<Swift>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        let mut all =
            swift_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else {
        codec_from::<Swift>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?
    };

    let mut file = zerodds_lang_cst::swift::File {
        doc: zerodds_lang_cst::common::Doc::lines(&[
            "Generated by wireprobe-zerodds-swift-gen. Do not edit.",
            "",
            "The octets come out of runtime/swift (ZdWire) when this runs;",
            "the runtime's sources compile beside this file as one module.",
            "Nothing below decides a wire question.",
        ]),
        imports: vec![zerodds_lang_cst::common::Ident::new("Foundation")],
        decls: items,
    };
    file.decls.extend(codec);

    let mut out = zerodds_lang_render::swift::file(&file);
    if args.input.is_some() {
        out.push_str(&dump::dump(&ast, root, key.union_label).map_err(Refusal::Gap)?);
    }
    out.push_str(&harness(args, &ast, root, key, sample.as_ref())?);
    Ok(out)
}

/// Write `main.swift` where `--out-dir` says.
///
/// # Errors
/// As [`source`], plus any write failure.
pub fn run(args: &Args) -> Result<(), Refusal> {
    let source = source(args)?;
    std::fs::create_dir_all(&args.out_dir)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", args.out_dir.display())))?;
    let main = args.out_dir.join("main.swift");
    std::fs::write(&main, source)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", main.display())))
}

/// The command, for `main`.
pub fn cli_main() {
    let args = match parse(std::env::args().collect()) {
        Ok(args) => args,
        Err(refusal) => {
            eprintln!("{}", refusal.message());
            std::process::exit(refusal.code());
        }
    };
    if let Err(refusal) = run(&args) {
        eprintln!("wireprobe-zerodds-swift-gen: {}", refusal.message());
        std::process::exit(refusal.code());
    }
}

/// The `ZdPlCdr1Framing(..)` expression one layout key adds up to —
/// the same six answers the writer and the reader are built over, so a
/// caller cannot set one and forget the other.
fn framing_expression(key: LayoutKey) -> String {
    let headers = key.pl_cdr1_headers;
    let flag = |value: bool| if value { "true" } else { "false" };
    format!(
        "ZdPlCdr1Framing({}, {}, {}, {}, {}, {})",
        fill::swift_string_literal(key.pid_flags.as_str()),
        key.short_pid_range.highest_short_id(),
        flag(headers.pads_declared_length()),
        flag(headers.pads_declared_optional_length()),
        flag(key.member_pid_carries_must_understand()),
        flag(headers.pads_after_value()),
    )
}

/// The tail of `main.swift`: the sample and the stream out, or the
/// bytes in and the values out. Top-level code, which is why the file
/// has to be called `main.swift`.
#[allow(clippy::too_many_lines)]
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    sample: Option<&serde_json::Value>,
) -> Result<String, Refusal> {
    let order = match args.endianness {
        Endianness::Little => "ZdEndianness.little",
        Endianness::Big => "ZdEndianness.big",
    };
    let max_align = args.encoding.max_alignment();
    let framing = framing_expression(key);

    let mut out = String::from(
        "\n// -- the probe ------------------------------------------------------\n\
         //\n\
         // Generated by wireprobe-zerodds-swift-gen. Do not edit.\n\
         //\n\
         // The octets come out of runtime/swift (ZdWire) when this runs.\n\
         // Nothing below decides a wire question.\n\n\
         // The runtime throws a ZdWireError on a refusal — the exit code\n\
         // belongs to this probe, not to the library, so it is made here:\n\
         // a caught error and the harness's own checks both end in this.\n\
         func zdFail(_ message: String) -> Never {\n    \
         FileHandle.standardError.write(Data((\"wireprobe-zerodds-swift: \" \
         + message + \"\\n\").utf8))\n    \
         exit(1)\n}\n",
    );

    if let Some(input) = &args.input {
        let decoder = decoder_name::<Swift>(ast, root);
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        let _ = write!(
            out,
            "guard let zd_data = FileManager.default.contents(atPath: {}) else {{\n    \
             zdFail(\"the input bytes are unreadable\")\n}}\n\
             let zd_input = [UInt8](zd_data)\n",
            fill::swift_string_literal(&input.to_string_lossy())
        );
        if args.encapsulation {
            // The encapsulation identifier is not re-read for the
            // encoding: the caller named one. The options word is read
            // — §7.6.3.1.2's low two bits say where the data ended.
            out.push_str(
                "if zd_input.count < 4 {\n    \
                 zdFail(\"fewer than the four-byte encapsulation header\")\n}\n\
                 let zd_declared = Int(zd_input[3] & 3)\n\
                 var zd_body = Array(zd_input[4...])\n\
                 if zd_body.count < zd_declared {\n    \
                 zdFail(\"the options field declares more padding than follows the header\")\n}\n\
                 zd_body.removeLast(zd_declared)\n",
            );
        } else {
            out.push_str("let zd_body = zd_input\n");
        }
        // Decode, dump and the file write all run inside one `do`: a
        // decoder that refuses and a dumper that refuses are the same
        // sample's refusal either way, and both end in `zdFail` here
        // rather than propagating out of `main.swift` uncaught.
        let _ = write!(
            out,
            "let zd_reader = ZdReader(zd_body, {order}, {max_align}, {framing})\n\
             do {{\n    \
             // The top-level type is not a member of anything, so no\n    \
             // @try_construct governs it and 7.2.2.7's default stands.\n    \
             let zd_value = try {decoder}(zd_reader, ZdTryConstruct.discard)\n    \
             if zd_reader.readerStatus() == ZdStatus.discarded {{\n        \
             // Not a failed decode: the receiver drops the object and\n        \
             // carries on. Exit code 3, the Rust probe's.\n        \
             FileHandle.standardError.write(Data(\"wireprobe-zerodds-swift: at \
             \\(zd_reader.readerPosition()): discarded\\n\".utf8))\n        \
             exit(3)\n    }}\n    \
             if zd_reader.readerStatus() != ZdStatus.ok {{\n        \
             zdFail(\"at \\(zd_reader.readerPosition()): \
             \\(zdStatusText(zd_reader.readerStatus()))\")\n    }}\n    \
             // Nothing may be left over that is not padding: a decoder\n    \
             // that stopped short and said nothing is the silent-drop\n    \
             // shape.\n    \
             let zd_left = zd_reader.readerRemaining()\n    \
             let zd_pad = ({max_align} - zd_reader.readerPosition() % {max_align}) % \
             {max_align}\n    \
             if zd_left > zd_pad {{\n        \
             zdFail(\"bytes left past the padding at the sample's end\")\n    }}\n    \
             let zd_doc = try {}(zd_value, \"sample\")\n    \
             do {{\n        \
             try (zd_doc + \"\\n\").write(toFile: {}, atomically: true, encoding: .utf8)\n    \
             }} catch {{\n        \
             zdFail(\"the values file cannot be written: \\(error)\")\n    }}\n\
             }} catch {{\n    \
             zdFail(\"\\(error)\")\n}}\n",
            dump::dumper_name(ast, root),
            fill::swift_string_literal(&values_out.to_string_lossy())
        );
        return Ok(out);
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let sample_type = match zerodds_lang_rules::adapter::swift::swift_value_type(ast, root) {
        Ok(type_) => zerodds_lang_render::swift::type_(&type_),
        Err(error) => {
            return Err(Refusal::NoForm(format!(
                "the sample's type has no Swift form: {error:?}"
            )));
        }
    };
    let filled = fill::fill(ast, root, sample).map_err(Refusal::Gap)?;
    let encoder = codec_name::<Swift>(ast, root, Purpose::Data);
    let short_codes = if key.length_codes.short_code_where_it_fits() {
        "true"
    } else {
        "false"
    };
    // `filled` is a memberwise-initialiser call, not a generated codec
    // function, so it only throws where it names one of the runtime's
    // checked code-point conversions — the one refusal a sample's
    // literal values can carry before a byte of it is written.
    let sample_try = if filled.contains("zdCheckedChar(") || filled.contains("zdCheckedWchar(") {
        "try "
    } else {
        ""
    };

    // The sample and the encode call share one `do`: either can be the
    // sample's own refusal, and both end in `zdFail` here rather than
    // propagating out of `main.swift` uncaught.
    let _ = write!(
        out,
        "let zd_writer = ZdWriter({order}, {max_align}, {framing}, {short_codes})\n\
         do {{\n    \
         let zd_sample: {sample_type} = {sample_try}{filled}\n    \
         try {encoder}(zd_writer, zd_sample)\n\
         }} catch {{\n    \
         zdFail(\"\\(error)\")\n}}\n\
         let zd_body = zd_writer.finish()\n"
    );
    if args.encapsulation {
        // §7.4.1.1 pads the serialized data to the next four-byte
        // boundary with zeros, and §7.6.3.1.2 puts the count of those
        // bytes in the encapsulation options so a receiver can find
        // where the data ended. Both halves or neither; the measured
        // implementations write both. `payload` stays unpadded: that
        // is the serialized data itself, and it is what the ledger
        // records.
        let header = encapsulation_header(args.encoding, args.endianness);
        let _ = write!(
            out,
            "let zd_pad = zdEncapsulationPadding(zd_body.count)\n\
             var zd_head: [UInt8] = [{}, {}, {}, {}]\n\
             zd_head[3] |= UInt8(zd_pad & 3)\n\
             FileHandle.standardOutput.write(Data(zd_head))\n\
             FileHandle.standardOutput.write(Data(zd_body))\n\
             FileHandle.standardOutput.write(Data([UInt8](repeating: 0, count: zd_pad)))\n",
            header[0], header[1], header[2], header[3]
        );
    } else {
        out.push_str("FileHandle.standardOutput.write(Data(zd_body))\n");
    }
    Ok(out)
}

/// The layout key the flags add up to, read off the same place the
/// sibling columns read it.
fn layout_key(args: &Args) -> LayoutKey {
    LayoutKey::in_dialect(args.dialect, args.encoding, args.endianness, Purpose::Data)
}

/// The four octets of an encapsulation header.
const fn encapsulation_header(encoding: Encoding, endianness: Endianness) -> [u8; 4] {
    let identifier: u16 = match (encoding, endianness) {
        (Encoding::PlainCdr1, Endianness::Big) => 0x0000,
        (Encoding::PlainCdr1, Endianness::Little) => 0x0001,
        (Encoding::PlCdr1, Endianness::Big) => 0x0002,
        (Encoding::PlCdr1, Endianness::Little) => 0x0003,
        (Encoding::PlainCdr2, Endianness::Big) => 0x0006,
        (Encoding::PlainCdr2, Endianness::Little) => 0x0007,
        (Encoding::DelimitedCdr, Endianness::Big) => 0x0008,
        (Encoding::DelimitedCdr, Endianness::Little) => 0x0009,
        (Encoding::PlCdr2, Endianness::Big) => 0x000a,
        (Encoding::PlCdr2, Endianness::Little) => 0x000b,
    };
    [
        #[allow(clippy::cast_possible_truncation)]
        ((identifier >> 8) as u8),
        #[allow(clippy::cast_possible_truncation)]
        (identifier as u8),
        0,
        0,
    ]
}

/// The model, lowered in the dialect of the run.
///
/// Most of a dialect reaches the bytes through the layout plan, which
/// the `LayoutKey` carries. Two answers do not: whether a `@hashid`
/// member advances the sequential counter the next unannotated member
/// is numbered from, and which spelling of an escaped name the hash
/// reads. Both are settled in `zerodds_idl_lower::complete`, before any
/// plan exists, so a `LayoutKey` cannot carry them and a generator that
/// lowers with `Options::default()` writes the specification's member
/// ids under every dialect. The same reading as the d, go, lua and
/// python generators and as `zerodds_wire_probe::model_from_idl_in_dialect`.
fn model_from_idl(source: &str, path: &str, dialect: Dialect) -> Result<Ast, Refusal> {
    let parsed = zerodds_idl_parse::parse(source);
    if !parsed.diagnostics().is_empty() {
        return Err(Refusal::Gap(format!("{path}: {:?}", parsed.diagnostics())));
    }
    let options = zerodds_idl_lower::Options {
        hashed_id_advances_the_counter: if dialect.hashed_id_advances_the_counter() {
            zerodds_idl_lower::HashedIdCounter::Advances
        } else {
            zerodds_idl_lower::HashedIdCounter::Skipped
        },
        hashed_name_spelling: if dialect.hashid_reads_the_escaped_spelling() {
            zerodds_idl_lower::HashedNameSpelling::AsWritten
        } else {
            zerodds_idl_lower::HashedNameSpelling::Stripped
        },
        ..zerodds_idl_lower::Options::default()
    };
    let lowered = zerodds_idl_lower::lower_with(&parsed.syntax(), path, options);
    if !lowered.diagnostics.is_empty() {
        // A clause of the specification refusing the model is the verdict
        // `refused` (exit 1), not a gap of this tree (exit 6).
        let text = format!("{path}: {:?}", lowered.diagnostics);
        return Err(
            if zerodds_idl_parse::refused_by_a_clause(&lowered.diagnostics) {
                Refusal::Refused(text)
            } else {
                Refusal::Gap(text)
            },
        );
    }
    let mut ast = lowered.ast;
    ast.resolve_all_by_name()
        .map_err(|error| Refusal::Gap(format!("{path}: {error}")))?;
    Ok(ast)
}

fn find_type(ast: &Ast, name: &str) -> Result<Id<TypeNode>, Refusal> {
    let simple = name.rsplit("::").next().unwrap_or(name);
    ast.types()
        .find(|(_, node)| node.name.as_str() == simple)
        .map(|(id, _)| id)
        .ok_or_else(|| Refusal::Gap(format!("`{name}` is not a type in this model")))
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

/// Whether a delimited `map` also carries its entry count, as the
/// dialect being reproduced writes it.
///
/// `CodecOptions::default()` is the sequence-equivalence reading of
/// XTypes 1.3 §7.4.1.1.4 — a `DHEADER` and an `O.length`. OpenDDS
/// follows rule (15) of §7.4.3.5.3 and writes the pairs with no count,
/// and a column that answers the default there drifts by those four
/// octets. `zerodds-wireprobe-gen` asks the dialect the same way for
/// the columns that share it.
fn codec_options(args: &Args) -> CodecOptions {
    CodecOptions {
        map_length_with_delimiter: !args.dialect.map_count_omitted_when_delimited(),
    }
}
