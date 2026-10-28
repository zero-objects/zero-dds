// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-julia-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-julia-gen` — the Julia program the Julia probe
//! runs.
//!
//! One IDL file, one type, one JSON sample and one layout key in; one
//! `probe.jl` holding the declared types, the codec, the sample and the
//! statements that write it to standard output — or, under `--in`, read
//! bytes back and write the values as JSON.
//! `tools/wireprobe/wireprobe-zerodds-julia` is the shell around it.
//!
//! Nothing here writes a byte. The declarations come from the Julia
//! materialiser (`adapter/julia.rs`), the codec from the shared walk
//! through [`zerodds_lang_rules::emit::julia::Julia`], and the bytes
//! come out of `runtime/julia/zerodds_wire.jl` when the program runs.
//! That runtime implements DDS-XTypes 1.3 §7.4 from the clause and
//! calls nothing in Rust, which is what makes this a further reading of
//! the specification rather than a rendering of the first one's answer.
//!
//! What `--values-out` receives is plain JSON in the dialect
//! `crates/wire-probe/src/lib.rs` documents and not canonical form: the
//! caller canonicalises in Rust, exactly as the C, C# and Python
//! columns have it, so two columns can only part over a value and never
//! over a spelling.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

mod dump;
mod fill;

use std::fmt::Write as _;
use std::path::PathBuf;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_render::julia::string_literal;
use zerodds_lang_rules::default_julia::julia_defaulters_called_by;
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::emit::julia::Julia;
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

const USAGE: &str = "\
wireprobe-zerodds-julia-gen — the Julia the ZeroDDS Julia probe runs

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
    --out-dir <dir>         where probe.jl goes (required)

The defaults are `wireprobe-zerodds`'s, because a Julia column that
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

/// Why the generator could not produce a file.
///
/// Three kinds and not one string, because the probe's exit code is the
/// verdict `docs/specs/zerodds-verdicts-1.0.md` records and a caller
/// cannot re-derive it from a message.
#[derive(Debug)]
pub enum Refusal {
    /// The model is outside what the specification admits, in the
    /// planner's own words. Exit 1 — `refused` in
    /// `docs/specs/zerodds-verdicts-1.0.md`, "it was asked and said no".
    Refused(String),
    /// The command line. Exit 2.
    Usage(String),
    /// Julia has no form for the construct. Exit 3. The materialiser's
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

/// Generate `probe.jl`.
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
    let applied = apply(&graph, &rule::julia::catalogue())
        .map_err(|error| Refusal::Gap(format!("the Julia rules do not apply: {error:?}")))?;
    // The materialiser is the authority on what Julia has a form for,
    // so its refusal is exit 3 and every other stop is exit 6.
    let mut file = zerodds_lang_rules::adapter::julia::julia_file(&ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    let key = layout_key(args);
    // What the chosen direction calls, and nothing else: a codec the
    // probe never reaches is source the interpreter still loads, and
    // generating both halves would double every case in the sweep.
    let codec = if args.input.is_some() {
        let decoders = decoder_from::<Julia>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        // Table 9's defaulters first, because a decoder opens with
        // `v = zd_def_T()`. Julia resolves a global at the call and not
        // at the definition, so the *presence* is what the file needs.
        let mut all =
            julia_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else {
        codec_from::<Julia>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?
    };
    file.decls.extend(codec);

    let mut out = prelude();
    out.push_str(&zerodds_lang_render::julia::file(&file));
    out.push_str(&harness(args, &ast, root, key, sample.as_ref())?);
    Ok(out)
}

/// Write the file where `--out-dir` says.
///
/// # Errors
/// As [`source`], plus any write failure.
pub fn run(args: &Args) -> Result<(), Refusal> {
    let source = source(args)?;
    std::fs::create_dir_all(&args.out_dir)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", args.out_dir.display())))?;
    let path = args.out_dir.join("probe.jl");
    std::fs::write(&path, source)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", path.display())))
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
        eprintln!("wireprobe-zerodds-julia-gen: {}", refusal.message());
        std::process::exit(refusal.code());
    }
}

/// The lines above every declaration: the runtime, loaded by the path
/// the driver names.
///
/// `include` and `using`, not a package: a probe is one file in a
/// temporary directory, and a `Project.toml` between the generator and
/// the octets would be one more thing to go stale. The runtime path
/// comes from the environment because the generator does not know where
/// the checkout is and the driver does; a missing variable is exit 6 —
/// a tool of ours, not the case.
fn prelude() -> String {
    String::from(
        "# Generated by wireprobe-zerodds-julia-gen. Do not edit.\n\
         #\n\
         # The octets come out of runtime/julia/zerodds_wire.jl when this runs.\n\
         # Nothing below decides a wire question.\n\
         let zd_runtime = get(ENV, \"ZERODDS_JULIA_RUNTIME\", \"\")\n\
         \x20   if isempty(zd_runtime)\n\
         \x20       println(stderr, \"wireprobe-zerodds-julia: ZERODDS_JULIA_RUNTIME is not set\")\n\
         \x20       exit(6)\n\
         \x20   end\n\
         \x20   include(zd_runtime)\n\
         end\n\
         using .ZeroddsWire\n",
    )
}

/// The tail of `probe.jl`: the sample and the octets, or the bytes and
/// the values.
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    sample: Option<&serde_json::Value>,
) -> Result<String, Refusal> {
    let order = match args.endianness {
        Endianness::Little => "ZD_LITTLE",
        Endianness::Big => "ZD_BIG",
    };
    let max_align = args.encoding.max_alignment();
    let mut out =
        String::from("\n# -- the probe ------------------------------------------------------\n");
    let _ = write!(out, "\nzd_framing = {}\n", framing_expression(key));

    if let Some(input) = &args.input {
        let decoder = decoder_name::<Julia>(ast, root);
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        let _ = write!(
            out,
            "\nzd_input = read({})\n",
            string_literal(&input.to_string_lossy())
        );
        if args.encapsulation {
            // The encapsulation identifier is not re-read for the
            // encoding: the caller named one. The options word is read,
            // because §7.6.3.1.2 says its low two bits locate where the
            // serialized data ended, and §7.4.1.1 says the padding
            // behind it is zero.
            out.push_str(
                "if length(zd_input) < 4\n\
                 \x20   println(stderr, \"wireprobe-zerodds-julia: fewer than the four-byte \
                 encapsulation header\")\n\
                 \x20   exit(1)\n\
                 end\n\
                 zd_declared = Int64(zd_input[4] & 0x03)\n\
                 zd_body = zd_input[5:end]\n\
                 if length(zd_body) < zd_declared\n\
                 \x20   println(stderr, string(\"wireprobe-zerodds-julia: the options field declares \", zd_declared, \" padding byte(s) and only \", length(zd_body), \" follow the header\"))\n\
                 \x20   exit(1)\n\
                 end\n\
                 zd_end = length(zd_body) - zd_declared\n\
                 if any(zd_body[(zd_end + 1):end] .!= 0x00)\n\
                 \x20   println(stderr, string(\"wireprobe-zerodds-julia: the \", zd_declared, \" trailing byte(s) are not the zeros 7.4.1.1 has\"))\n\
                 \x20   exit(1)\n\
                 end\n\
                 zd_body = zd_body[1:zd_end]\n",
            );
        } else {
            out.push_str("zd_body = zd_input\n");
        }
        let _ = writeln!(
            out,
            "zd_reader = ZdReader(zd_body, {order}, {max_align}, zd_framing)"
        );
        out.push_str(&dump::dump(ast, root, key.union_label).map_err(Refusal::Gap)?);
        let _ = write!(
            out,
            "\n# The top-level type is not a member of anything, so no\n\
             # `@try_construct` governs it and 7.2.2.7's default stands.\n\
             zd_value = try\n\
             \x20   {decoder}(zd_reader, ZD_TRY_CONSTRUCT_DISCARD)\n\
             catch zd_error\n\
             \x20   zd_error isa ZdWireError || rethrow()\n\
             \x20   println(stderr, string(\"wireprobe-zerodds-julia: \", sprint(showerror, zd_error)))\n\
             \x20   exit(1)\n\
             end\n\
             if reader_status(zd_reader) === ZD_STATUS_DISCARDED\n\
             \x20   # Not a failed decode. 7.2.2.7: the receiver drops the object\n\
             \x20   # and carries on. Exit code 3, the Rust probe's.\n\
             \x20   println(stderr, string(\"wireprobe-zerodds-julia: at \", reader_position(zd_reader), \": discarded\"))\n\
             \x20   exit(3)\n\
             end\n\
             if reader_status(zd_reader) !== ZD_STATUS_OK\n\
             \x20   println(stderr, string(\"wireprobe-zerodds-julia: at \", reader_position(zd_reader), \": \", status_text(reader_status(zd_reader))))\n\
             \x20   exit(1)\n\
             end\n\
             # Nothing may be left over that is not padding: a decoder that\n\
             # stopped short and said nothing is the silent-drop shape.\n\
             zd_left = reader_remaining(zd_reader)\n\
             zd_pad = mod({max_align} - mod(reader_position(zd_reader), {max_align}), {max_align})\n\
             if zd_left > zd_pad\n\
             \x20   println(stderr, string(\"wireprobe-zerodds-julia: \", zd_left, \" byte(s) left at offset \", reader_position(zd_reader), \", of which at most \", zd_pad, \" could be padding\"))\n\
             \x20   exit(1)\n\
             end\n\
             # Plain JSON in the dialect crates/wire-probe/src/lib.rs documents\n\
             # and not canonical form: the caller canonicalises in Rust.\n\
             open({}, \"w\") do zd_out\n\
             \x20   write(zd_out, {}(zd_value, \"sample\"))\n\
             \x20   write(zd_out, \"\\n\")\n\
             end\n",
            string_literal(&values_out.to_string_lossy()),
            dump::dumper_name(ast, root)
        );
        return Ok(out);
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let sample_type =
        zerodds_lang_render::julia::type_(&<Julia as CodecTarget>::value_type(ast, root));
    let filled = fill::fill(ast, root, sample).map_err(Refusal::Gap)?;
    let encoder = codec_name::<Julia>(ast, root, Purpose::Data);
    let short_codes = if key.length_codes.short_code_where_it_fits() {
        "true"
    } else {
        "false"
    };

    let _ = write!(
        out,
        "\n# A value the type cannot hold is the sample's refusal and not the\n\
         # harness's: the freeze converts into the declared widths and a value\n\
         # outside one is an error here, before any octet — the same exit the\n\
         # Rust probe answers for the same sample.\n\
         zd_sample = try\n\
         \x20   zd_freeze({sample_type}, {filled})\n\
         catch zd_error\n\
         \x20   println(stderr, string(\"wireprobe-zerodds-julia: the sample is outside the type: \", sprint(showerror, zd_error)))\n\
         \x20   exit(1)\n\
         end\n\
         zd_writer = ZdWriter({order}, {max_align}, zd_framing, {short_codes})\n\
         try\n\
         \x20   {encoder}(zd_writer, zd_sample)\n\
         catch zd_error\n\
         \x20   zd_error isa ZdWireError || rethrow()\n\
         \x20   println(stderr, string(\"wireprobe-zerodds-julia: \", sprint(showerror, zd_error)))\n\
         \x20   exit(1)\n\
         end\n\
         zd_octets = zd_writer_bytes(zd_writer)\n"
    );
    if args.encapsulation {
        let header = encapsulation_header(args.encoding, args.endianness);
        let _ = write!(
            out,
            "# The encapsulation identifier 7.6.3.1.2 gives this encoding and\n\
             # byte order; the options field is zero here and the caller that\n\
             # wants a padding count writes it.\n\
             write(stdout, UInt8[{:#04x}, {:#04x}, {:#04x}, {:#04x}])\n",
            header[0], header[1], header[2], header[3]
        );
    }
    out.push_str("write(stdout, zd_octets)\nflush(stdout)\n");
    Ok(out)
}

/// The PL_CDR1 framing one layout key adds up to, as the Julia
/// expression that builds it. Keyword arguments, so a field that moves
/// in the runtime is a loud `MethodError` rather than a transposition.
fn framing_expression(key: LayoutKey) -> String {
    let headers = key.pl_cdr1_headers;
    let flag = |value: bool| if value { "true" } else { "false" };
    format!(
        "ZdPlCdr1Framing(\n\
         \x20   pid_flags = {},\n\
         \x20   highest_short_id = {},\n\
         \x20   pad_declared_length = {},\n\
         \x20   pad_declared_optional_length = {},\n\
         \x20   member_must_understand = {},\n\
         \x20   pad_after_value = {},\n\
         )",
        string_literal(key.pid_flags.as_str()),
        key.short_pid_range.highest_short_id(),
        flag(headers.pads_declared_length()),
        flag(headers.pads_declared_optional_length()),
        flag(key.member_pid_carries_must_understand()),
        flag(headers.pads_after_value())
    )
}

/// The layout key the flags add up to, read off the same place the C
/// and Python columns read it.
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
