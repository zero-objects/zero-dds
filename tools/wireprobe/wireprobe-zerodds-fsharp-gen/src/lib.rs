// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-fsharp-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-fsharp-gen` — the F# the F# probe runs under
//! `dotnet fsi`.
//!
//! One IDL file, one type, one JSON sample and one layout key in; two
//! files out: `Types.fs` holding the declarations and the codec, and
//! `Main.fsx` holding the sample (or, with `--in`, the dumpers) and the
//! harness that writes the octets to standard output or reads them
//! back and writes the values as JSON where `--values-out` says.
//! `tools/wireprobe/wireprobe-zerodds-fsharp` is the shell around it,
//! and runs `Main.fsx` under `dotnet fsi --exec` beside
//! `runtime/fsharp/ZdWire.fs`.
//!
//! Two files and not one, unlike every compiled column's generator:
//! `crates/lang-rules/tests/fsharp_end_to_end.rs`'s own module
//! documentation carries the measurement this split answers — a
//! top-level `module rec ZdTypes` with no `=` is refused by `dotnet
//! fsi --exec` when it is the *script's own* content, and compiles
//! cleanly the moment the same text is a file the script `#load`s.
//! `Types.fs` is therefore always the loaded file and `Main.fsx`
//! always the script, in that order, which is also why `Main.fsx`
//! needs no `open ZdWire` of its own to reach `Types.fs`'s codec: F#
//! compiles a session's `#load`ed files in the order they are loaded,
//! and a later one sees an earlier one's public names without
//! repeating the load.
//!
//! Nothing here writes a byte. The declarations come from the F#
//! materialiser, the codec from the shared walk through
//! [`zerodds_lang_rules::emit::fsharp::Fsharp`], and the bytes come out
//! of `runtime/fsharp` when the program runs — a further reading of
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
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, PidFlagPolicy, Purpose};
use zerodds_lang_cst::common::{Doc, Ident};
use zerodds_lang_cst::fsharp;
use zerodds_lang_rules::default_fsharp::fsharp_defaulters_called_by;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::emit::fsharp::Fsharp;
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

const USAGE: &str = "\
wireprobe-zerodds-fsharp-gen — the F# the ZeroDDS F# probe runs under `dotnet fsi`

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
    --out-dir <dir>         where Types.fs and Main.fsx go (required)

The defaults are `wireprobe-zerodds`'s, because an F# column that
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
    /// Where the files go.
    pub out_dir: PathBuf,
}

/// Why the generator could not produce a program, classified so the
/// probe's exit code is the verdict `docs/specs/zerodds-verdicts-1.0.md`
/// records.
#[derive(Debug)]
pub enum Refusal {
    /// The command line. Exit 2.
    Usage(String),
    /// F# has no form for the construct. Exit 3. The materialiser's
    /// answer and nobody else's.
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
/// Reads [`EncodeError::is_spec_refusal`] rather than matching
/// `NoLayout`, exactly as `wireprobe-zerodds-zig-gen`'s own `from_codec`
/// does and for the reason its documentation gives: the planner stops
/// for reasons that are this tree's and not the specification's, and
/// those are gaps.
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

/// The two files a run produces.
pub struct Generated {
    /// `Types.fs`'s content.
    pub types: String,
    /// `Main.fsx`'s content.
    pub main: String,
}

/// Generate `Types.fs` and `Main.fsx`.
///
/// # Errors
/// [`Refusal`], classified so the probe can answer in the verdict's own
/// exit code.
pub fn source(args: &Args) -> Result<Generated, Refusal> {
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
    let applied = apply(&graph, &rule::fsharp::catalogue())
        .map_err(|error| Refusal::Gap(format!("the F# rules do not apply: {error:?}")))?;
    // The materialiser is the authority on what F# has a form for, so
    // its refusal is exit 3 and every other stop is exit 6.
    let items = zerodds_lang_rules::adapter::fsharp::fsharp_items(&ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    let key = layout_key(args);
    let codec = if args.input.is_some() {
        let decoders = decoder_from::<Fsharp>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        let mut all =
            fsharp_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else {
        codec_from::<Fsharp>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?
    };

    let mut types_file = fsharp::File {
        doc: Doc::lines(&[
            "Generated by wireprobe-zerodds-fsharp-gen. Do not edit.",
            "",
            "The octets come out of runtime/fsharp (ZdWire) when Main.fsx runs.",
            "Nothing below decides a wire question.",
        ]),
        module: Ident::new(zerodds_lang_rules::emit::fsharp::module_name()),
        decls: Vec::new(),
    };
    types_file.decls.extend(items);
    types_file.decls.extend(codec);
    let mut types_text = zerodds_lang_render::fsharp::file(&types_file);

    if args.input.is_some() {
        // The dumpers join `Types.fs` and not `Main.fsx`: a `.fsx`
        // script processes its own top-level bindings in the order
        // they are written, with no `module rec` to give a later one
        // visibility into an earlier one's caller — measured (`dotnet
        // fsi`, .NET SDK 9.0.106), a dumper that calls one written
        // after it in the same script is `error FS0039`, "the value or
        // constructor … is not defined". `Types.fs` already carries
        // `module rec` for the codec's own mutual recursion, and the
        // dumpers are exactly as mutually recursive as the types they
        // print.
        types_text.push_str(&dump::dump(&ast, root, key.union_label).map_err(Refusal::Gap)?);
    }

    let main_text = harness(args, &ast, root, key, sample.as_ref())?;
    Ok(Generated {
        types: types_text,
        main: main_text,
    })
}

/// Write `Types.fs` and `Main.fsx` where `--out-dir` says.
///
/// # Errors
/// As [`source`], plus any write failure.
pub fn run(args: &Args) -> Result<(), Refusal> {
    let generated = source(args)?;
    std::fs::create_dir_all(&args.out_dir)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", args.out_dir.display())))?;
    let types = args.out_dir.join("Types.fs");
    std::fs::write(&types, generated.types)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", types.display())))?;
    let main = args.out_dir.join("Main.fsx");
    std::fs::write(&main, generated.main)
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
        eprintln!("wireprobe-zerodds-fsharp-gen: {}", refusal.message());
        std::process::exit(refusal.code());
    }
}

/// `Main.fsx`: the `#load`s, the sample and the two streams, or the
/// bytes and the values.
#[allow(clippy::too_many_lines)]
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    sample: Option<&serde_json::Value>,
) -> Result<String, Refusal> {
    let order = match args.endianness {
        Endianness::Little => "ZdWire.LE",
        Endianness::Big => "ZdWire.BE",
    };
    let xcdr = match args.encoding.max_alignment() {
        8 => "ZdWire.XCDR1",
        _ => "ZdWire.XCDR2",
    };

    let mut out = String::from(
        "// -- the probe ------------------------------------------------------\n\
         //\n\
         // Generated by wireprobe-zerodds-fsharp-gen. Do not edit.\n\
         //\n\
         // The octets come out of runtime/fsharp (ZdWire) when this runs.\n\
         // Nothing below decides a wire question.\n\n\
         #load \"ZdWire.fs\"\n\
         #load \"Types.fs\"\n\
         open ",
    );
    out.push_str(zerodds_lang_rules::emit::fsharp::module_name());
    out.push_str("\n\n");

    if let Some(input) = &args.input {
        let decoder = decoder_name::<Fsharp>(ast, root);
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        let _ = writeln!(
            out,
            "let zdMain () =\n    \
             let zdInput = System.IO.File.ReadAllBytes({})",
            fill::fsharp_string_literal(&input.to_string_lossy())
        );
        if args.encapsulation {
            // The encapsulation identifier is not re-read for the
            // encoding: the caller named one. The options word is
            // read — §7.6.3.1.2's low two bits say where the data
            // ended.
            out.push_str(
                "    if zdInput.Length < 4 then\n\
                 \x20       eprintfn \"wireprobe-zerodds-fsharp: fewer than the four-byte \
                 encapsulation header\"\n\
                 \x20       exit 1\n\
                 \x20   let zdDeclared = int (zdInput.[3] &&& 3uy)\n\
                 \x20   let zdRawBody = zdInput.[4 ..]\n\
                 \x20   if zdRawBody.Length < zdDeclared then\n\
                 \x20       eprintfn \"wireprobe-zerodds-fsharp: the options field declares \
                 more padding than follows the header\"\n\
                 \x20       exit 1\n\
                 \x20   let zdBody = zdRawBody.[.. zdRawBody.Length - zdDeclared - 1]\n",
            );
        } else {
            out.push_str("    let zdBody = zdInput\n");
        }
        let max_align = args.encoding.max_alignment();
        let _ = write!(
            out,
            "    let zdReader = ZdWire.Reader(zdBody, {order}, {xcdr})\n\
             \x20   // The top-level type is not a member of anything, so no\n\
             \x20   // @try_construct governs it and 7.2.2.7's default stands.\n\
             \x20   let zdValue = {decoder} zdReader ZdWire.TryConstruct.Discard\n\
             \x20   if zdReader.ReaderStatus() = ZdWire.Status.Discarded then\n\
             \x20       // Not a failed decode: the receiver drops the object and\n\
             \x20       // carries on. Exit code 3, the Rust probe's.\n\
             \x20       eprintfn \"wireprobe-zerodds-fsharp: at %d: discarded\" (zdReader.ReaderPosition())\n\
             \x20       exit 3\n\
             \x20   if zdReader.ReaderStatus() <> ZdWire.Status.Ok then\n\
             \x20       eprintfn \"wireprobe-zerodds-fsharp: at %d: %A\" (zdReader.ReaderPosition()) (zdReader.ReaderStatus())\n\
             \x20       exit 1\n\
             \x20   // Nothing may be left over that is not padding: a decoder\n\
             \x20   // that stopped short and said nothing is the silent-drop\n\
             \x20   // shape.\n\
             \x20   let zdMaxAlign = {max_align}\n\
             \x20   let zdPad = (zdMaxAlign - zdReader.ReaderPosition() % zdMaxAlign) % zdMaxAlign\n\
             \x20   if zdReader.ReaderRemaining() > zdPad then\n\
             \x20       eprintfn \"wireprobe-zerodds-fsharp: bytes left past the padding at \
             the sample's end\"\n\
             \x20       exit 1\n\
             \x20   let zdDoc = System.Text.StringBuilder()\n\
             \x20   {} zdDoc zdValue\n\
             \x20   System.IO.File.WriteAllText({}, zdDoc.ToString() + \"\\n\")\n",
            dump::dumper_name(ast, root),
            fill::fsharp_string_literal(&values_out.to_string_lossy())
        );
        out.push_str(
            "\ntry\n    zdMain ()\n    exit 0\nwith\n\
             | :? ZdWire.WireError as zdE ->\n    \
             eprintfn \"wireprobe-zerodds-fsharp: %s\" zdE.Message\n    exit 1\n\
             | zdE ->\n    eprintfn \"wireprobe-zerodds-fsharp: %s\" zdE.Message\n    exit 6\n",
        );
        return Ok(out);
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let sample_expr = fill::fill(ast, root, sample).map_err(Refusal::Gap)?;
    let sample_type = fill::type_text(ast, root, "the sample's type").map_err(Refusal::NoForm)?;
    let sample_text = zerodds_lang_render::fsharp::expr(&sample_expr);
    let encoder = codec_name::<Fsharp>(ast, root, Purpose::Data);
    let headers = key.pl_cdr1_headers;
    let flag = |value: bool| if value { "true" } else { "false" };
    // The XCDR2 half of the length-code question, beside the framing and
    // not inside it — the shape the other nineteen columns' harnesses
    // use. `LengthCodePolicy` answers the other half in the plan.
    let short_codes = flag(key.length_codes.short_code_where_it_fits());

    let _ = write!(
        out,
        "let zdFraming : ZdWire.PlCdr1Framing =\n\
         \x20   {{ PidFlags = {}\n\
         \x20     MemberMustUnderstand = {}\n\
         \x20     PadDeclaredLength = {}\n\
         \x20     PadDeclaredOptionalLength = {}\n\
         \x20     PadAfterValue = {}\n\
         \x20     HighestShortId = {} }}\n\n\
         let zdMain () =\n\
         \x20   let zdWriter = ZdWire.Writer({order}, {xcdr}, zdFraming, {short_codes})\n\
         \x20   let zdSample : {sample_type} = {sample_text}\n\
         \x20   {encoder} zdWriter zdSample\n\
         \x20   let zdBytes = zdWriter.Finish()\n\
         \x20   use zdStdout = System.Console.OpenStandardOutput()\n",
        pid_flag_policy_case(key.pid_flags),
        flag(key.member_pid_carries_must_understand()),
        flag(headers.pads_declared_length()),
        flag(headers.pads_declared_optional_length()),
        flag(headers.pads_after_value()),
        key.short_pid_range.highest_short_id(),
    );
    if args.encapsulation {
        let header = encapsulation_header(args.encoding, args.endianness);
        let _ = write!(
            out,
            "    // The encapsulation identifier 7.6.3.1.2 gives this encoding\n\
             \x20   // and byte order; the options field is zero here.\n\
             \x20   zdStdout.Write([| {}uy; {}uy; {}uy; {}uy |], 0, 4)\n",
            header[0], header[1], header[2], header[3]
        );
    }
    out.push_str(
        "    zdStdout.Write(zdBytes, 0, zdBytes.Length)\n\n\
         try\n    zdMain ()\n    exit 0\nwith\n\
         | :? ZdWire.WireError as zdE ->\n    \
         eprintfn \"wireprobe-zerodds-fsharp: %s\" zdE.Message\n    exit 1\n\
         | zdE ->\n    eprintfn \"wireprobe-zerodds-fsharp: %s\" zdE.Message\n    exit 6\n",
    );
    Ok(out)
}

/// The `ZdWire.PidFlagPolicy` case one layout key's PID flag axis names.
///
/// Written out as a union case and not as a string the runtime parses:
/// a misspelled case is a compile error in the probe, where a misspelled
/// string would only be found when the probe ran.
const fn pid_flag_policy_case(policy: PidFlagPolicy) -> &'static str {
    match policy {
        PidFlagPolicy::SentinelMustUnderstand => "ZdWire.SentinelMustUnderstand",
        PidFlagPolicy::Bare => "ZdWire.Bare",
        PidFlagPolicy::ImplExtensionOnMembers => "ZdWire.ImplExtensionOnMembers",
    }
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
/// for. The same reading `wireprobe-zerodds-zig-gen`'s own
/// `model_from_idl` is, for the reason its documentation gives.
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
