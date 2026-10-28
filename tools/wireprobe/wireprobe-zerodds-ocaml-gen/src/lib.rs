// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-ocaml-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-ocaml-gen` — the OCaml program the OCaml probe
//! builds and runs.
//!
//! One IDL file, one type, one JSON sample and one layout key in; one
//! `main.ml` holding the types, the codec, the sample and the harness
//! that writes the octets to standard output — or, with `--in`, reads
//! them back and writes the values as JSON where `--values-out` says.
//! `tools/wireprobe/wireprobe-zerodds-ocaml` is the shell around it.
//!
//! Nothing here writes a byte. The declarations come from the OCaml
//! materialiser, the codec from the shared walk through
//! [`zerodds_lang_rules::emit::ocaml::OCaml`], and the bytes come out of
//! `runtime/ocaml` (`zdwire.ml`) when the program runs. That runtime
//! implements DDS-XTypes 1.3 §7.4 from the clause and calls nothing in
//! Rust, which is what makes this a further reading of the
//! specification rather than a rendering of the first one's answer.
//!
//! ## One file, one module
//!
//! The generated program is one `main.ml` compiled beside a copy of
//! `zdwire.ml` — no dune, no findlib packages, nothing outside the
//! OCaml standard library — so the probe builds with `ocamlopt` or
//! `ocamlc` alone.
//!
//! ## Not canonical form
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
use zerodds_lang_rules::default_ocaml::ocaml_defaulters_called_by;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::emit::ocaml::{OCaml, ocaml_type_text};
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

const USAGE: &str = "\
wireprobe-zerodds-ocaml-gen — the OCaml the ZeroDDS OCaml probe builds and runs

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
    --out-dir <dir>         where main.ml goes (required)

The defaults are `wireprobe-zerodds`'s, because an OCaml column that
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
    /// The model is outside what the specification admits, in the
    /// planner's own words. Exit 1 — `refused` in
    /// `docs/specs/zerodds-verdicts-1.0.md`, "it was asked and said no".
    Refused(String),
    /// The command line. Exit 2.
    Usage(String),
    /// OCaml has no form for the construct. Exit 3. The materialiser's
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

/// Generate `main.ml`.
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
    let applied = apply(&graph, &rule::ocaml::catalogue())
        .map_err(|error| Refusal::Gap(format!("the OCaml rules do not apply: {error:?}")))?;
    // The materialiser is the authority on what OCaml has a form for,
    // so its refusal is exit 3 and every other stop is exit 6.
    let file = zerodds_lang_rules::adapter::ocaml::ocaml_file(&ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    let key = layout_key(args);
    let mut out = String::from(
        "(* Generated by wireprobe-zerodds-ocaml-gen. Do not edit.\n\n\
         \x20  The octets come out of runtime/ocaml (Zdwire) when this runs.\n\
         \x20  Nothing below decides a wire question. *)\n\n",
    );
    out.push_str(&zerodds_lang_render::ocaml::file(&file));

    // What the chosen direction calls, and nothing else: a codec the
    // probe never reaches is source the compiler still checks, and
    // generating both halves would double every case in the sweep.
    if args.input.is_some() {
        let decoders = decoder_from::<OCaml>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        // Table 9's defaulters, because a decoder opens with
        // `let v = ref (zd_def_T ()) in`.
        let defaulters =
            ocaml_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        out.push_str(&zerodds_lang_render::ocaml::value_chain(&defaulters));
        out.push_str(&zerodds_lang_render::ocaml::codec_chain(&decoders));
        out.push_str(&dump::dump(&ast, root, key.union_label).map_err(Refusal::Gap)?);
    } else {
        let codec = codec_from::<OCaml>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        out.push_str(&zerodds_lang_render::ocaml::codec_chain(&codec));
    }
    out.push_str(&harness(args, &ast, root, key, sample.as_ref())?);
    Ok(out)
}

/// Write `main.ml` where `--out-dir` says.
///
/// # Errors
/// As [`source`], plus any write failure.
pub fn run(args: &Args) -> Result<(), Refusal> {
    let source = source(args)?;
    std::fs::create_dir_all(&args.out_dir)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", args.out_dir.display())))?;
    let main = args.out_dir.join("main.ml");
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
        eprintln!("wireprobe-zerodds-ocaml-gen: {}", refusal.message());
        std::process::exit(refusal.code());
    }
}

/// The OCaml expression building the PL_CDR1 framing record the layout
/// key adds up to — the same fields the sibling harnesses name, so a
/// framing that moves is a compile error naming the field.
fn framing_record(key: LayoutKey) -> String {
    let flag = |value: bool| if value { "true" } else { "false" };
    let headers = key.pl_cdr1_headers;
    format!(
        "{{ Zdwire.pid_flags = {}; highest_short_id = {}; pad_declared_length = {}; \
         pad_declared_optional_length = {}; member_must_understand = {}; pad_after_value = {} }}",
        fill::ocaml_string_literal(key.pid_flags.as_str()),
        key.short_pid_range.highest_short_id(),
        flag(headers.pads_declared_length()),
        flag(headers.pads_declared_optional_length()),
        flag(key.member_pid_carries_must_understand()),
        flag(headers.pads_after_value()),
    )
}

/// The tail of `main.ml`: the sample and the stream, or the bytes and
/// the values.
#[allow(clippy::too_many_lines)]
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    sample: Option<&serde_json::Value>,
) -> Result<String, Refusal> {
    let order = match args.endianness {
        Endianness::Little => "Zdwire.Little",
        Endianness::Big => "Zdwire.Big",
    };
    let max_align = args.encoding.max_alignment();
    let short_codes = if key.length_codes.short_code_where_it_fits() {
        "true"
    } else {
        "false"
    };

    let mut out = String::from(
        "\n(* -- the probe ----------------------------------------------------\n\n\
         \x20  Generated by wireprobe-zerodds-ocaml-gen. Do not edit.\n\
         \x20  The octets come out of runtime/ocaml (Zdwire) when this runs;\n\
         \x20  nothing below decides a wire question. *)\n\n",
    );

    if let Some(input) = &args.input {
        let decoder = decoder_name::<OCaml>(ast, root);
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        let _ = write!(
            out,
            "let () =\n\
             \x20 try\n\
             \x20   let zd_ch = open_in_bin {input} in\n\
             \x20   let zd_len = in_channel_length zd_ch in\n\
             \x20   let zd_input = really_input_string zd_ch zd_len in\n\
             \x20   close_in zd_ch;\n",
            input = fill::ocaml_string_literal(&input.to_string_lossy())
        );
        if args.encapsulation {
            // The encapsulation identifier is not re-read for the
            // encoding: the caller named one. The options word is read,
            // because §7.6.3.1.2 says a receiver shall use its low two
            // bits to find where the serialized data ended.
            out.push_str(
                "    if String.length zd_input < 4 then begin\n\
                 \x20     prerr_endline \"wireprobe-zerodds-ocaml: fewer than the four-byte \
                 encapsulation header\";\n\
                 \x20     exit 1\n\
                 \x20   end;\n\
                 \x20   let zd_declared = Char.code zd_input.[3] land 3 in\n\
                 \x20   let zd_body = String.sub zd_input 4 (String.length zd_input - 4) in\n\
                 \x20   if String.length zd_body < zd_declared then begin\n\
                 \x20     prerr_endline \"wireprobe-zerodds-ocaml: the options field declares \
                 more padding than follows the header\";\n\
                 \x20     exit 1\n\
                 \x20   end;\n\
                 \x20   let zd_body = String.sub zd_body 0 (String.length zd_body - zd_declared) \
                 in\n",
            );
        } else {
            out.push_str("    let zd_body = zd_input in\n");
        }
        let _ = write!(
            out,
            "    let zd_framing = {framing} in\n\
             \x20   let r = Zdwire.new_reader zd_body {order} {max_align} zd_framing in\n\
             \x20   (* The top-level type is not a member of anything, so no\n\
             \x20      @try_construct governs it and 7.2.2.7's default stands. *)\n\
             \x20   let zd_value = {decoder} r Zdwire.Discard in\n\
             \x20   (match Zdwire.reader_status r with\n\
             \x20    | Zdwire.St_discarded ->\n\
             \x20      (* Not a failed decode: the receiver drops the object and\n\
             \x20         carries on. Exit code 3, the Rust probe's. *)\n\
             \x20      prerr_endline (\"wireprobe-zerodds-ocaml: at \" ^ string_of_int \
             (Zdwire.reader_position r) ^ \": discarded\");\n\
             \x20      exit 3\n\
             \x20    | Zdwire.St_ok -> ()\n\
             \x20    | zd_s ->\n\
             \x20      prerr_endline (\"wireprobe-zerodds-ocaml: at \" ^ string_of_int \
             (Zdwire.reader_position r) ^ \": \" ^ Zdwire.status_text zd_s);\n\
             \x20      exit 1);\n\
             \x20   (* Nothing may be left over that is not padding: a decoder that\n\
             \x20      stopped short and said nothing is the silent-drop shape. *)\n\
             \x20   let zd_left = Zdwire.reader_remaining r in\n\
             \x20   let zd_pad = ({max_align} - Zdwire.reader_position r mod {max_align}) mod \
             {max_align} in\n\
             \x20   if zd_left > zd_pad then begin\n\
             \x20     prerr_endline \"wireprobe-zerodds-ocaml: bytes left past the padding at \
             the sample's end\";\n\
             \x20     exit 1\n\
             \x20   end;\n\
             \x20   let zd_out = open_out {values_out} in\n\
             \x20   output_string zd_out ({dumper} zd_value \"sample\");\n\
             \x20   output_string zd_out \"\\n\";\n\
             \x20   close_out zd_out\n\
             \x20 with Zdwire.Wire_error zd_msg ->\n\
             \x20   prerr_endline (\"wireprobe-zerodds-ocaml: \" ^ zd_msg);\n\
             \x20   exit 1\n",
            framing = framing_record(key),
            dumper = dump::dumper_name(ast, root),
            values_out = fill::ocaml_string_literal(&values_out.to_string_lossy()),
        );
        return Ok(out);
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let sample_type = match zerodds_lang_rules::adapter::ocaml::ocaml_value_type(ast, root) {
        Ok(type_) => ocaml_type_text(&type_),
        Err(error) => {
            return Err(Refusal::NoForm(format!(
                "the sample's type has no OCaml form: {error:?}"
            )));
        }
    };
    let filled = fill::fill(ast, root, sample).map_err(Refusal::Gap)?;
    let encoder = codec_name::<OCaml>(ast, root, Purpose::Data);
    let _ = write!(
        out,
        "let () =\n\
         \x20 set_binary_mode_out stdout true;\n\
         \x20 try\n\
         \x20   let zd_framing = {framing} in\n\
         \x20   let w = Zdwire.new_writer {order} {max_align} zd_framing {short_codes} in\n\
         \x20   let zd_sample : {sample_type} = {filled} in\n\
         \x20   {encoder} w zd_sample;\n\
         \x20   let zd_body = Zdwire.finish w in\n",
        framing = framing_record(key),
    );
    if args.encapsulation {
        let header = encapsulation_header(args.encoding, args.endianness);
        let _ = write!(
            out,
            "    (* The encapsulation identifier 7.6.3.1.2 gives this encoding\n\
             \x20      and byte order. The options field is zero here. *)\n\
             \x20   print_string \"\\x{:02x}\\x{:02x}\\x{:02x}\\x{:02x}\";\n",
            header[0], header[1], header[2], header[3]
        );
    }
    out.push_str(
        "    output_bytes stdout zd_body\n\
         \x20 with Zdwire.Wire_error zd_msg ->\n\
         \x20   prerr_endline (\"wireprobe-zerodds-ocaml: \" ^ zd_msg);\n\
         \x20   exit 1\n",
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

/// Read the IDL the way the dialect reads it.
///
/// Two answers stand before any plan is made and neither has a default
/// that holds across vendors: whether a `@hashid` member advances the
/// sequential member-id counter, and which spelling of an escaped name
/// the hash is taken over. `zerodds_idl_lower::lower` picks
/// `Options::default()` for both and so silently answers for one vendor
/// only; `Dialect` holds the measured facts.
///
/// `zerodds_wire_probe::model_from_idl_in_dialect` is the same reading
/// on the Rust side.
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
