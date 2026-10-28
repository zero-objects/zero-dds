// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-zig-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-zig-gen` — the Zig program the Zig probe builds
//! and runs.
//!
//! One IDL file, one type, one JSON sample and one layout key in; one
//! `main.zig` holding the types, the codec, the sample and the harness
//! that writes the octets to standard output — or, with `--in`, reads
//! them back and writes the values as JSON where `--values-out` says.
//! `tools/wireprobe/wireprobe-zerodds-zig` is the shell around it, and
//! compiles `main.zig` beside `runtime/zig/zdwire.zig`.
//!
//! Nothing here writes a byte. The declarations come from the Zig
//! materialiser, the codec from the shared walk through
//! [`zerodds_lang_rules::emit::zig::Zig`], and the bytes come out of
//! `runtime/zig` when the program runs — a further reading of
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
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::zig;
use zerodds_lang_rules::default_zig::zig_defaulters_called_by;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::emit::zig::Zig;
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

const USAGE: &str = "\
wireprobe-zerodds-zig-gen — the Zig the ZeroDDS Zig probe builds and runs

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
    --out-dir <dir>         where main.zig goes (required)

The defaults are `wireprobe-zerodds`'s, because a Zig column that
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
    /// The command line. Exit 2.
    Usage(String),
    /// Zig has no form for the construct. Exit 3. The materialiser's
    /// answer and nobody else's.
    NoForm(String),
    /// The model is outside what the specification admits, in the
    /// refusing tool's own words. Exit 1 — `refused` in
    /// `docs/specs/zerodds-verdicts-1.0.md`, "it was asked and said no",
    /// which is what a type cycle IDL 4.2 §7.4.1.4.4.4.4 does not admit
    /// is. Measured: `recursive-array` and `recursive-optional-external`
    /// came back as exit 6 here and as exit 1 from the Rust probe, and
    /// exit 6 says `harness-gap`, "a tool of **ours** has not written
    /// it" — a claim about this tree that is not true of either case.
    Refused(String),
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
            Self::Usage(text) | Self::NoForm(text) | Self::Refused(text) | Self::Gap(text) => text,
        }
    }
}

/// A codec walk's refusal, in the verdict it belongs to.
///
/// [`EncodeError::NoLayout`] is the layout crate saying the model is not
/// serialisable — a cycle that does not pass through a sequence or a
/// map, a `@key` set it cannot resolve — and that is the model's
/// property, not a hole in this generator. Every other variant is this
/// tree's own walk saying it has no writer or no reader for something,
/// which is exactly `harness-gap`.
/// The verdict a codec error answers in.
///
/// Reads `EncodeError::is_spec_refusal` rather than matching
/// `NoLayout`: the planner stops for reasons that are this tree's and
/// not the specification's — a type graph that nests past
/// `MAX_TYPECODE_DEPTH`, a reference it could not resolve — and those
/// are gaps. Before the planner carried the distinction in its type,
/// this arm called all of them `refused`.
fn from_codec(error: &EncodeError) -> Refusal {
    if error.is_spec_refusal() {
        Refusal::Refused(format!("{error}"))
    } else {
        Refusal::Gap(format!("{error}"))
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

/// Generate `main.zig`.
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
    let applied = apply(&graph, &rule::zig::catalogue())
        .map_err(|error| Refusal::Gap(format!("the Zig rules do not apply: {error:?}")))?;
    // The materialiser is the authority on what Zig has a form for, so
    // its refusal is exit 3 and every other stop is exit 6.
    let items = zerodds_lang_rules::adapter::zig::zig_items(&ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    let key = layout_key(args);
    let codec = if args.input.is_some() {
        let decoders = decoder_from::<Zig>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        let mut all =
            zig_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else {
        codec_from::<Zig>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?
    };

    let mut file = zig::File {
        doc: Doc::lines(&[
            "Generated by wireprobe-zerodds-zig-gen. Do not edit.",
            "",
            "The octets come out of runtime/zig (zdwire) when this runs.",
            "Nothing below decides a wire question.",
        ]),
        decls: vec![import("std", "std"), import("zdwire", "zdwire.zig")],
    };
    file.decls.extend(items);
    file.decls.extend(codec);

    let mut out = zerodds_lang_render::zig::file(&file);
    if args.input.is_some() {
        out.push_str(&dump::dump(&ast, root, key.union_label).map_err(Refusal::Gap)?);
    }
    out.push_str(&harness(args, &ast, root, key, sample.as_ref())?);
    Ok(out)
}

/// `const <name> = @import("<what>");`
fn import(name: &str, what: &str) -> zig::Decl {
    zig::Decl::Const(zig::ConstDecl {
        body_text: zerodds_lang_cst::common::BodyText::none(),
        doc: Doc::none(),
        is_pub: false,
        name: Ident::new(name),
        type_: None,
        value: zig::Expr::call(
            "@import",
            vec![zig::Expr::Literal(Literal::Str(what.to_string()))],
        ),
    })
}

/// Write `main.zig` where `--out-dir` says.
///
/// # Errors
/// As [`source`], plus any write failure.
pub fn run(args: &Args) -> Result<(), Refusal> {
    let source_text = source(args)?;
    std::fs::create_dir_all(&args.out_dir)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", args.out_dir.display())))?;
    let main = args.out_dir.join("main.zig");
    std::fs::write(&main, source_text)
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
        eprintln!("wireprobe-zerodds-zig-gen: {}", refusal.message());
        std::process::exit(refusal.code());
    }
}

/// The tail of `main.zig`: the sample and the two streams, or the bytes
/// and the values.
#[allow(clippy::too_many_lines)]
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    sample: Option<&serde_json::Value>,
) -> Result<String, Refusal> {
    let order = match args.endianness {
        Endianness::Little => "zdwire.Endianness.little",
        Endianness::Big => "zdwire.Endianness.big",
    };
    let max_align = args.encoding.max_alignment();

    let mut out = String::from(
        "\n// -- the probe ------------------------------------------------------\n\
         //\n\
         // Generated by wireprobe-zerodds-zig-gen. Do not edit.\n\
         //\n\
         // The octets come out of runtime/zig (zdwire) when this runs.\n\
         // Nothing below decides a wire question.\n\n\
         // Zig hands `main` the process environment, and the `Io` in it is\n\
         // what every file and stream operation is performed against.\n\
         pub fn main(zd_init: std.process.Init) u8 {\n\
         \x20   zdMain(zd_init) catch |zd_e| {\n\
         \x20       zdComplain(zd_init.io, \"wireprobe-zerodds-zig: {s}\\n\", \
         .{@errorName(zd_e)});\n\
         \x20       return 1;\n\
         \x20   };\n\
         \x20   return 0;\n\
         }\n\n\
         // A `File` writer needs a buffer of its own and has to be flushed,\n\
         // so the one-line complaints go through here rather than repeating\n\
         // the sequence at every exit.\n\
         fn zdComplain(zd_io: std.Io, comptime zd_fmt: []const u8, zd_args: anytype) void {\n\
         \x20   var zd_buf: [512]u8 = undefined;\n\
         \x20   var zd_err = std.Io.File.stderr().writer(zd_io, &zd_buf);\n\
         \x20   zd_err.interface.print(zd_fmt, zd_args) catch {};\n\
         \x20   zd_err.interface.flush() catch {};\n\
         }\n\n\
         fn zdMain(zd_init: std.process.Init) !void {\n\
         \x20   const zd_io = zd_init.io;\n\
         \x20   const zd_heap = zd_init.arena.allocator();\n",
    );

    if let Some(input) = &args.input {
        let decoder = decoder_name::<Zig>(ast, root);
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        let _ = writeln!(
            out,
            "    const zd_input = try std.Io.Dir.cwd().readFileAlloc(zd_io, {}, zd_heap, .unlimited);",
            fill::zig_string_literal(&input.to_string_lossy())
        );
        if args.encapsulation {
            // The encapsulation identifier is not re-read for the
            // encoding: the caller named one. The options word is read
            // — §7.6.3.1.2's low two bits say where the data ended.
            out.push_str(
                "    if (zd_input.len < 4) {\n\
                 \x20       zdComplain(zd_io, \"wireprobe-zerodds-zig: fewer than the \
                 four-byte encapsulation header\\n\", .{});\n\
                 \x20       std.process.exit(1);\n\
                 \x20   }\n\
                 \x20   const zd_declared: usize = zd_input[3] & 3;\n\
                 \x20   var zd_body = zd_input[4..];\n\
                 \x20   if (zd_body.len < zd_declared) {\n\
                 \x20       zdComplain(zd_io, \"wireprobe-zerodds-zig: the options field \
                 declares more padding than follows the header\\n\", .{});\n\
                 \x20       std.process.exit(1);\n\
                 \x20   }\n\
                 \x20   zd_body = zd_body[0 .. zd_body.len - zd_declared];\n",
            );
        } else {
            out.push_str("    const zd_body = zd_input;\n");
        }
        let _ = write!(
            out,
            "    var zd_reader = zdwire.Reader.init(zd_heap, zd_body, {order}, {max_align}, {}, {});\n\
             \x20   // The top-level type is not a member of anything, so no\n\
             \x20   // @try_construct governs it and 7.2.2.7's default stands.\n\
             \x20   const zd_value = try {decoder}(&zd_reader, zdwire.TryConstruct.discard);\n\
             \x20   if (zd_reader.readerStatus() == zdwire.Status.discarded) {{\n\
             \x20       // Not a failed decode: the receiver drops the object and\n\
             \x20       // carries on. Exit code 3, the Rust probe's.\n\
             \x20       zdComplain(zd_io, \"wireprobe-zerodds-zig: at {{d}}: \
             discarded\\n\", .{{zd_reader.readerPosition()}});\n\
             \x20       std.process.exit(3);\n\
             \x20   }}\n\
             \x20   if (zd_reader.readerStatus() != zdwire.Status.ok) {{\n\
             \x20       zdComplain(zd_io, \"wireprobe-zerodds-zig: at {{d}}: {{s}}\\n\", \
             .{{ zd_reader.readerPosition(), zd_reader.readerStatus().text() }});\n\
             \x20       std.process.exit(1);\n\
             \x20   }}\n\
             \x20   // Nothing may be left over that is not padding: a decoder\n\
             \x20   // that stopped short and said nothing is the silent-drop\n\
             \x20   // shape.\n\
             \x20   const zd_left = zd_reader.readerRemaining();\n\
             \x20   const zd_pad = ({max_align} - zd_reader.readerPosition() % {max_align}) \
             % {max_align};\n\
             \x20   if (zd_left > zd_pad) {{\n\
             \x20       zdComplain(zd_io, \"wireprobe-zerodds-zig: bytes left past the \
             padding at the sample's end\\n\", .{{}});\n\
             \x20       std.process.exit(1);\n\
             \x20   }}\n\
             \x20   var zd_doc: std.ArrayList(u8) = .empty;\n\
             \x20   try {}(zd_heap, &zd_doc, zd_value);\n\
             \x20   try zd_doc.append(zd_heap, '\\n');\n\
             \x20   try std.Io.Dir.cwd().writeFile(zd_io, \
             .{{ .sub_path = {}, .data = zd_doc.items }});\n\
             }}\n",
            fill::zig_string_literal(key.pid_flags.as_str()),
            key.short_pid_range.highest_short_id(),
            dump::dumper_name(ast, root),
            fill::zig_string_literal(&values_out.to_string_lossy())
        );
        return Ok(out);
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let sample_type = fill::type_text(ast, root, "the sample's type").map_err(Refusal::NoForm)?;
    let filled = fill::fill(ast, root, sample).map_err(Refusal::Gap)?;
    let encoder = codec_name::<Zig>(ast, root, Purpose::Data);
    let headers = key.pl_cdr1_headers;
    let flag = |value: bool| if value { "true" } else { "false" };
    let short_codes = flag(key.length_codes.short_code_where_it_fits());

    let _ = write!(
        out,
        "    const zd_framing = zdwire.PlCdr1Framing{{\n\
         \x20       .pid_flags = {},\n\
         \x20       .highest_short_id = {},\n\
         \x20       .pad_declared_length = {},\n\
         \x20       .pad_declared_optional_length = {},\n\
         \x20       .member_must_understand = {},\n\
         \x20       .pad_after_value = {},\n\
         \x20   }};\n\
         \x20   var zd_writer = zdwire.Writer.init(zd_heap, {order}, {max_align}, zd_framing, {short_codes});\n\
         \x20   const zd_sample: {sample_type} = {filled};\n\
         \x20   try {encoder}(&zd_writer, zd_sample);\n\
         \x20   const zd_bytes = zd_writer.finish();\n\
         \x20   const zd_stdout = std.Io.File.stdout();\n",
        fill::zig_string_literal(key.pid_flags.as_str()),
        key.short_pid_range.highest_short_id(),
        flag(headers.pads_declared_length()),
        flag(headers.pads_declared_optional_length()),
        flag(key.member_pid_carries_must_understand()),
        flag(headers.pads_after_value()),
    );
    if args.encapsulation {
        let header = encapsulation_header(args.encoding, args.endianness);
        let _ = write!(
            out,
            "    // The encapsulation identifier 7.6.3.1.2 gives this encoding\n\
             \x20   // and byte order; the options field is zero here.\n\
             \x20   try zd_stdout.writeStreamingAll(zd_io, &[_]u8{{ {}, {}, {}, {} }});\n",
            header[0], header[1], header[2], header[3]
        );
    }
    out.push_str(
        "    try zd_stdout.writeStreamingAll(zd_io, zd_bytes);\n\
         }\n",
    );
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

/// Parse and lower one IDL source under the dialect the run was asked
/// for.
///
/// Most of a dialect reaches the bytes through the layout plan, which
/// the `LayoutKey` carries. Two answers do not: whether a `@hashid`
/// member advances the sequential counter the next unannotated member is
/// numbered from, and which spelling of an escaped name the hash reads.
/// Both are settled in `zerodds_idl_lower::complete`, before any plan
/// exists, so a `LayoutKey` cannot carry them and a generator that
/// lowers with `Options::default()` writes the specification's member
/// ids under every dialect.
/// `zerodds_wire_probe::model_from_idl_in_dialect` is the same reading
/// on the Rust side; `Dialect` holds the vendor facts.
fn model_from_idl(source_text: &str, path: &str, dialect: Dialect) -> Result<Ast, Refusal> {
    let parsed = zerodds_idl_parse::parse(source_text);
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
