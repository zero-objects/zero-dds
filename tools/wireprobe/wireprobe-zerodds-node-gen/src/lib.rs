// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-node-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-node-gen` — the JavaScript program the Node probe
//! runs.
//!
//! One IDL file, one type, one JSON sample and one layout key in; one
//! `probe.js` holding the declarations, the codec, the sample and the
//! harness that writes the octets to standard output — or, with `--in`,
//! reads them back and writes the values as JSON where `--values-out`
//! says. `tools/wireprobe/wireprobe-zerodds-node` is the shell around
//! it, and runs `probe.js` under `node` beside `runtime/node/zdwire.js`.
//!
//! Nothing here writes a byte. The declarations come from the Node
//! materialiser (`crates/lang-rules/src/adapter/node.rs`), the codec
//! from the shared walk through
//! [`zerodds_lang_rules::emit::node::Node`], and the bytes come out of
//! `runtime/node/zdwire.js` when the program runs — a further reading
//! of DDS-XTypes 1.3 §7.4, calling nothing in Rust.
//!
//! ## No compile step, and what stands in for one
//!
//! Plain JavaScript is not compiled: a syntax error in the generated
//! file would otherwise surface as "the probe did not run" — exit 6,
//! `harness-gap`, a claim about this tree — when it is this generator's
//! own defect. `tools/wireprobe/wireprobe-zerodds-node` runs
//! `node --check probe.js` before it runs the file, exactly as
//! `tools/wireprobe/wireprobe-zerodds-julia` parses before it evaluates,
//! and answers exit 7 where that fails.
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
use zerodds_lang_rules::default_node::node_defaulters_called_by;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::emit::node::Node;
use zerodds_lang_rules::encode::EncodeError;

const USAGE: &str = "\
wireprobe-zerodds-node-gen — the JavaScript the ZeroDDS Node probe runs

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
    --out-dir <dir>         where probe.js goes (required)

The defaults are `wireprobe-zerodds`'s, because a Node column that
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
    /// JavaScript has no form for the construct. Exit 3. The
    /// materialiser's answer and nobody else's.
    NoForm(String),
    /// The model is outside what the specification admits, in the
    /// refusing tool's own words. Exit 1.
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
/// Reads `EncodeError::is_spec_refusal` rather than matching a variant
/// by name: the planner stops for reasons that are this tree's and not
/// the specification's, and those are gaps; a clause the specification
/// itself refuses is `refused`.
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

/// Generate `probe.js`.
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

    // The materialiser is the authority on what JavaScript has a form
    // for, so its refusal is exit 3 and every other stop is exit 6.
    // Unlike the Zig column this needs no `ModelGraph`/`apply`: the
    // module documentation of `crates/lang-rules/src/adapter/node.rs`
    // says why this column does not join the seesaw rule catalogue.
    let declarations = zerodds_lang_rules::adapter::node::node_items(&ast)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    let key = layout_key(args);
    let codec = if args.input.is_some() {
        let decoders = decoder_from::<Node>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        let mut all =
            node_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else {
        codec_from::<Node>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?
    };

    let mut out = String::from(
        "\"use strict\";\n\
         // Generated by wireprobe-zerodds-node-gen. Do not edit.\n\
         //\n\
         // The octets come out of runtime/node/zdwire.js when this runs.\n\
         // Nothing below decides a wire question.\n\n\
         const zdwire = require(\"./zdwire.js\");\n",
    );
    for decl in &declarations {
        out.push('\n');
        out.push_str(&zerodds_lang_render::node::stat(decl));
    }
    for decl in &codec {
        out.push('\n');
        out.push_str(&zerodds_lang_render::node::stat(decl));
    }
    if args.input.is_some() {
        out.push_str(&dump::dump(&ast, root, key.union_label).map_err(Refusal::Gap)?);
    }
    out.push_str(&harness(args, &ast, root, key, sample.as_ref())?);
    Ok(out)
}

/// Write `probe.js` where `--out-dir` says.
///
/// # Errors
/// As [`source`], plus any write failure.
pub fn run(args: &Args) -> Result<(), Refusal> {
    let source_text = source(args)?;
    std::fs::create_dir_all(&args.out_dir)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", args.out_dir.display())))?;
    let main = args.out_dir.join("probe.js");
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
        eprintln!("wireprobe-zerodds-node-gen: {}", refusal.message());
        std::process::exit(refusal.code());
    }
}

/// The tail of `probe.js`: the sample and the two streams, or the bytes
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
        Endianness::Little => "zdwire.Endianness.Little",
        Endianness::Big => "zdwire.Endianness.Big",
    };
    let max_align = args.encoding.max_alignment();
    let headers = key.pl_cdr1_headers;
    let flag = |value: bool| if value { "true" } else { "false" };
    let framing = format!(
        "{{\n\
         \x20 pidFlags: {},\n\
         \x20 shortMaxId: {},\n\
         \x20 padsDeclaredLength: {},\n\
         \x20 padsOptionalLength: {},\n\
         \x20 memberMustUnderstand: {},\n\
         \x20 padAfterValue: {},\n\
         }}",
        pid_flags_expression(key.pid_flags),
        key.short_pid_range.highest_short_id(),
        flag(headers.pads_declared_length()),
        flag(headers.pads_declared_optional_length()),
        flag(key.member_pid_carries_must_understand()),
        flag(headers.pads_after_value()),
    );

    let mut out = String::new();

    if let Some(input) = &args.input {
        let decoder = decoder_name::<Node>(ast, root);
        let dumper = dump::dumper_name(ast, root);
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        let _ = write!(
            out,
            "\n\
             const zdFraming = {framing};\n\
             const zdInput = require(\"fs\").readFileSync({});\n",
            js_string_literal(&input.to_string_lossy())
        );
        if args.encapsulation {
            out.push_str(
                "if (zdInput.length < 4) {\n\
                 \x20 process.stderr.write(\"wireprobe-zerodds-node: fewer than the four-byte \
                 encapsulation header\\n\");\n\
                 \x20 process.exit(1);\n\
                 }\n\
                 const zdDeclared = zdInput[3] & 3;\n\
                 let zdBody = zdInput.subarray(4);\n\
                 if (zdBody.length < zdDeclared) {\n\
                 \x20 process.stderr.write(\"wireprobe-zerodds-node: the options field declares \
                 more padding than follows the header\\n\");\n\
                 \x20 process.exit(1);\n\
                 }\n\
                 zdBody = zdBody.subarray(0, zdBody.length - zdDeclared);\n",
            );
        } else {
            out.push_str("const zdBody = zdInput;\n");
        }
        let _ = write!(
            out,
            "const zdReader = new zdwire.Reader(zdBody, {order}, {max_align}, zdFraming);\n\
             let zdValue;\n\
             try {{\n\
             \x20 // The top-level type is not a member of anything, so no\n\
             \x20 // @try_construct governs it and §7.2.2.7's default stands.\n\
             \x20 zdValue = {decoder}(zdReader, zdwire.TryConstruct.Discard);\n\
             }} catch (zdError) {{\n\
             \x20 const zdMessage = zdError instanceof Error ? zdError.message : String(zdError);\n\
             \x20 process.stderr.write(`wireprobe-zerodds-node: ${{zdMessage}}\\n`);\n\
             \x20 process.exit(1);\n\
             }}\n\
             if (zdReader.readerStatus() === zdwire.Status.Discarded) {{\n\
             \x20 // Not a failed decode: DDS-XTypes 1.3 §7.2.2.7's receiver drops the\n\
             \x20 // object and carries on. Exit code 3, the Rust probe's.\n\
             \x20 process.stderr.write(`wireprobe-zerodds-node: at ${{zdReader.readerPosition()}}: discarded\\n`);\n\
             \x20 process.exit(3);\n\
             }}\n\
             if (zdReader.readerStatus() !== zdwire.Status.Ok) {{\n\
             \x20 process.stderr.write(`wireprobe-zerodds-node: at ${{zdReader.readerPosition()}}: ${{zdReader.readerStatus()}}\\n`);\n\
             \x20 process.exit(1);\n\
             }}\n\
             // Nothing may be left over that is not padding: a decoder that\n\
             // stopped short and said nothing is the silent-drop shape.\n\
             const zdLeft = zdReader.readerRemaining();\n\
             const zdPad = ({max_align} - (zdReader.readerPosition() % {max_align})) % {max_align};\n\
             if (zdLeft > zdPad) {{\n\
             \x20 process.stderr.write(`wireprobe-zerodds-node: ${{zdLeft}} byte(s) left at offset ${{zdReader.readerPosition()}}, of which at most ${{zdPad}} could be padding\\n`);\n\
             \x20 process.exit(1);\n\
             }}\n\
             let zdJson;\n\
             try {{\n\
             \x20 zdJson = JSON.stringify({dumper}(zdValue)).split(JSON.stringify(ZD_NEGATIVE_ZERO)).join(\"-0\");\n\
             }} catch (zdError) {{\n\
             \x20 const zdMessage = zdError instanceof Error ? zdError.message : String(zdError);\n\
             \x20 process.stderr.write(`wireprobe-zerodds-node: ${{zdMessage}}\\n`);\n\
             \x20 process.exit(1);\n\
             }}\n\
             require(\"fs\").writeFileSync({}, `${{zdJson}}\\n`);\n",
            js_string_literal(&values_out.to_string_lossy())
        );
        return Ok(out);
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let filled = fill::fill(ast, root, sample).map_err(Refusal::Gap)?;
    let encoder = codec_name::<Node>(ast, root, Purpose::Data);
    let short_codes = flag(key.length_codes.short_code_where_it_fits());

    let _ = write!(
        out,
        "\n\
         const zdFraming = {framing};\n\
         const zdWriter = new zdwire.Writer({order}, {max_align}, zdFraming, {short_codes});\n\
         const zdSample = {};\n\
         // A value the type cannot hold is the sample's own refusal and not the\n\
         // harness's — `writeChar` throwing on a code point above U+00FF is\n\
         // §7.2.2.2.1.2.2, and the Rust probe answers 1 for the same sample.\n\
         // Exit 6 here would file it as `harness-gap`, a tool of ours that has\n\
         // not been written, which this is not. docs/specs/zerodds-verdicts-1.0.md.\n\
         try {{\n\
         \x20 {encoder}(zdWriter, zdSample);\n\
         }} catch (zdError) {{\n\
         \x20 const zdMessage = zdError instanceof Error ? zdError.message : String(zdError);\n\
         \x20 process.stderr.write(`wireprobe-zerodds-node: ${{zdMessage}}\\n`);\n\
         \x20 process.exit(1);\n\
         }}\n\
         const zdBytes = Buffer.from(zdWriter.finish());\n",
        zerodds_lang_render::node::expr(&filled),
    );
    if args.encapsulation {
        let header = encapsulation_header(args.encoding, args.endianness);
        let _ = write!(
            out,
            "// The encapsulation identifier §7.6.3.1.2 gives this encoding and\n\
             // byte order; the padding count sits in the low two bits of the\n\
             // options field.\n\
             const zdPad = (4 - (zdBytes.length & 3)) & 3;\n\
             process.stdout.write(Buffer.from([0x{:02x}, 0x{:02x}, 0x{:02x}, 0x{:02x} | zdPad]));\n\
             process.stdout.write(zdBytes);\n\
             process.stdout.write(Buffer.alloc(zdPad));\n",
            header[0], header[1], header[2], header[3]
        );
    } else {
        out.push_str("process.stdout.write(zdBytes);\n");
    }
    Ok(out)
}

/// The `PidFlags` object literal a layout key's `pid_flags` axis takes,
/// spelled from `runtime/node/zdwire.js`'s own exported constants.
fn pid_flags_expression(policy: zerodds_ast_layout::PidFlagPolicy) -> &'static str {
    match policy {
        zerodds_ast_layout::PidFlagPolicy::SentinelMustUnderstand => {
            "zdwire.PID_FLAGS_SENTINEL_MUST_UNDERSTAND"
        }
        zerodds_ast_layout::PidFlagPolicy::Bare => "zdwire.PID_FLAGS_BARE",
        zerodds_ast_layout::PidFlagPolicy::ImplExtensionOnMembers => {
            "zdwire.PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS"
        }
    }
}

/// The layout key the flags add up to.
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
/// for. `zerodds_wire_probe::model_from_idl_in_dialect` is the same
/// reading on the Rust side; `Dialect` holds the vendor facts.
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

/// A JavaScript double-quoted string literal, through the renderer's own
/// escaping.
fn js_string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    out.push_str(&zerodds_lang_render::node::string_text(text));
    out.push('"');
    out
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
