// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-elixir-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-elixir-gen` — the Elixir script the Elixir probe
//! runs.
//!
//! One IDL file, one type, one JSON sample and one layout key in; one
//! `main.exs` holding the type modules, the codec module, the sample
//! and the statements that write it to standard output — or, under
//! `--in`, read the named bytes back and write the values as JSON.
//! `tools/wireprobe/wireprobe-zerodds-elixir` is the shell around it.
//!
//! Nothing here writes a byte. The declarations come from the Elixir
//! materialiser, the codec from the shared walk through
//! [`zerodds_lang_rules::emit::elixir::Elixir`], and the bytes come out
//! of `runtime/elixir` when the script runs. That runtime implements
//! DDS-XTypes 1.3 §7.4 from the clause and calls nothing in Rust,
//! which is what makes this a further reading of the specification
//! rather than a rendering of the first one's answer.
//!
//! ## The policy flags
//!
//! The full set `wireprobe-zerodds` takes, so a case runs through both
//! by substituting the program name: the dialect is applied first as a
//! preset and each flag overrides one of its answers, whatever order
//! the flags were written in.
//!
//! ## Not canonical form
//!
//! What `--values-out` receives is plain JSON and the caller
//! canonicalises it in Rust, which is what the C and Python columns
//! do: a second canonicaliser in a second language would be a second
//! chance for two columns to part over a spelling rather than over a
//! value.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

mod dump;
mod fill;

use std::fmt::Write as _;
use std::path::PathBuf;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::{
    CollectionDelimiterPolicy, EnumHolderWidth, EnumeratorValue, FixedDigitCount, LayoutKey,
    LengthCodePolicy, MapDelimiter, MapEntryOrder, MutableUnionFraming, PidFlagPolicy,
    PlCdr1HeaderPolicy, ShortPidRange, UnionLabelMatch, WcharFraming, WcharWidth,
    WstringLengthPolicy, WstringUnitOrder,
};
use zerodds_ast_layout::{Dialect, Encoding, Endianness, Purpose};
use zerodds_lang_rules::default_elixir::elixir_defaulters_called_by;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::elixir::Elixir;
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

const USAGE: &str = "\
wireprobe-zerodds-elixir-gen — the Elixir the ZeroDDS Elixir probe runs

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
    --out-dir <dir>         where main.exs goes (required)

Each policy flag of `wireprobe-zerodds` is taken as well and overrides
the dialect's answer for that one axis: --length-codes,
--collection-delimiters, --wstring-length, --wchar-framing,
--wchar-width, --map-delimiter, --wstring-units, --pl-cdr1-headers,
--pid-flags, --enum-holder, --union-labels, --map-entries,
--enumerator-value, --union-framing, --short-pid-range, --fixed-digits,
--map-length.
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
    /// The peer whose answers this reproduces, plus any per-axis
    /// overrides, already folded into one key.
    pub key: LayoutKey,
    /// The same peer, unfolded. Kept beside the key because two of its
    /// answers are settled before any plan exists and so cannot travel
    /// in one — see `model_from_idl`.
    pub dialect: Dialect,
    /// Whether a delimited `map` also carries its entry count.
    pub map_length_with_delimiter: bool,
    /// Where the file goes.
    pub out_dir: PathBuf,
}

/// Why the generator could not produce a file — three kinds and not one
/// string, because the probe's exit code is the verdict
/// `docs/specs/zerodds-verdicts-1.0.md` records.
#[derive(Debug)]
pub enum Refusal {
    /// The model is outside what the specification admits, in the
    /// planner's own words. Exit 1 — `refused` in
    /// `docs/specs/zerodds-verdicts-1.0.md`, "it was asked and said no".
    Refused(String),
    /// The command line. Exit 2.
    Usage(String),
    /// Elixir has no form for the construct. Exit 3.
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

    let mut length_codes: Option<LengthCodePolicy> = None;
    let mut collection_delimiters: Option<CollectionDelimiterPolicy> = None;
    let mut wstring_length: Option<WstringLengthPolicy> = None;
    let mut wchar_framing: Option<WcharFraming> = None;
    let mut wchar_width: Option<WcharWidth> = None;
    let mut map_delimiter: Option<MapDelimiter> = None;
    let mut wstring_units: Option<WstringUnitOrder> = None;
    let mut pl_cdr1_headers: Option<PlCdr1HeaderPolicy> = None;
    let mut pid_flags: Option<PidFlagPolicy> = None;
    let mut enum_holder: Option<EnumHolderWidth> = None;
    let mut union_label: Option<UnionLabelMatch> = None;
    let mut map_entries: Option<MapEntryOrder> = None;
    let mut enumerator_value: Option<EnumeratorValue> = None;
    let mut union_framing: Option<MutableUnionFraming> = None;
    let mut short_pid_range: Option<ShortPidRange> = None;
    let mut fixed_digits: Option<FixedDigitCount> = None;
    let mut map_length: Option<bool> = None;

    // One shape per policy flag: parse it with the axis's own `parse`,
    // refuse an unknown value by name. A macro would hide the flag
    // names this file exists to accept.
    // `axis` rather than `parse`: a parameter named after the enclosing
    // function reads to `zerodds-lint` as a recursive call, and the
    // report then names two functions that never call themselves.
    fn policy<T>(flag: &str, name: &str, axis: impl Fn(&str) -> Option<T>) -> Result<T, Refusal> {
        axis(name).ok_or_else(|| Refusal::Usage(format!("{flag}: `{name}` is not one of them")))
    }

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
            "--length-codes" => {
                length_codes = Some(policy("--length-codes", &take()?, LengthCodePolicy::parse)?);
            }
            "--collection-delimiters" => {
                collection_delimiters = Some(policy(
                    "--collection-delimiters",
                    &take()?,
                    CollectionDelimiterPolicy::parse,
                )?);
            }
            "--wstring-length" => {
                wstring_length = Some(policy(
                    "--wstring-length",
                    &take()?,
                    WstringLengthPolicy::parse,
                )?);
            }
            "--wchar-framing" => {
                wchar_framing = Some(policy("--wchar-framing", &take()?, WcharFraming::parse)?);
            }
            "--wchar-width" => {
                wchar_width = Some(policy("--wchar-width", &take()?, WcharWidth::parse)?);
            }
            "--map-delimiter" => {
                map_delimiter = Some(policy("--map-delimiter", &take()?, MapDelimiter::parse)?);
            }
            "--wstring-units" => {
                wstring_units = Some(policy(
                    "--wstring-units",
                    &take()?,
                    WstringUnitOrder::parse,
                )?);
            }
            "--pl-cdr1-headers" => {
                pl_cdr1_headers = Some(policy(
                    "--pl-cdr1-headers",
                    &take()?,
                    PlCdr1HeaderPolicy::parse,
                )?);
            }
            "--pid-flags" => {
                pid_flags = Some(policy("--pid-flags", &take()?, PidFlagPolicy::parse)?);
            }
            "--enum-holder" => {
                enum_holder = Some(policy("--enum-holder", &take()?, EnumHolderWidth::parse)?);
            }
            "--union-labels" => {
                union_label = Some(policy("--union-labels", &take()?, UnionLabelMatch::parse)?);
            }
            "--map-entries" => {
                map_entries = Some(policy("--map-entries", &take()?, MapEntryOrder::parse)?);
            }
            "--enumerator-value" => {
                enumerator_value = Some(policy(
                    "--enumerator-value",
                    &take()?,
                    EnumeratorValue::parse,
                )?);
            }
            "--union-framing" => {
                union_framing = Some(policy(
                    "--union-framing",
                    &take()?,
                    MutableUnionFraming::parse,
                )?);
            }
            "--short-pid-range" => {
                short_pid_range =
                    Some(policy("--short-pid-range", &take()?, ShortPidRange::parse)?);
            }
            "--fixed-digits" => {
                fixed_digits = Some(policy("--fixed-digits", &take()?, FixedDigitCount::parse)?);
            }
            "--map-length" => {
                map_length = Some(match take()?.as_str() {
                    "sequence-equivalent" => true,
                    "rule-15-omits" => false,
                    other => {
                        return Err(Refusal::Usage(format!(
                            "--map-length: `{other}` is not a map-length policy"
                        )));
                    }
                });
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

    // The dialect first, then each flag that was given: a preset and
    // not a patch, so applying it afterwards would discard the flags.
    let mut key = LayoutKey::in_dialect(dialect, encoding, endianness, Purpose::Data);
    if let Some(policy) = length_codes {
        key.length_codes = policy;
    }
    if let Some(policy) = collection_delimiters {
        key.collection_delimiters = policy;
    }
    if let Some(policy) = wstring_length {
        key.wstring_length = policy;
    }
    if let Some(policy) = wchar_framing {
        key.wchar_framing = policy;
    }
    if let Some(policy) = wchar_width {
        key.wchar_width = policy;
    }
    if let Some(policy) = map_delimiter {
        key.map_delimiter = policy;
    }
    if let Some(policy) = wstring_units {
        key.wstring_unit_order = policy;
    }
    if let Some(policy) = pl_cdr1_headers {
        key.pl_cdr1_headers = policy;
    }
    if let Some(policy) = pid_flags {
        key.pid_flags = policy;
    }
    if let Some(policy) = enum_holder {
        key.enum_holder = policy;
    }
    if let Some(policy) = union_label {
        key.union_label = policy;
    }
    if let Some(policy) = map_entries {
        key.map_entries = policy;
    }
    if let Some(policy) = enumerator_value {
        key.enumerator_value = policy;
    }
    if let Some(policy) = union_framing {
        key.union_framing = policy;
    }
    if let Some(policy) = short_pid_range {
        key.short_pid_range = policy;
    }
    if let Some(policy) = fixed_digits {
        key.fixed_digits = policy;
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
        key,
        dialect,
        // The dialect's own answer, unless the flag named one:
        // OpenDDS's rows measured rule (15)'s reading.
        map_length_with_delimiter: map_length
            .unwrap_or(!dialect.map_count_omitted_when_delimited()),
        out_dir: out_dir.ok_or_else(|| Refusal::Usage("--out-dir is required".to_string()))?,
    })
}

/// Generate `main.exs`.
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
    let applied = apply(&graph, &rule::elixir::catalogue())
        .map_err(|error| Refusal::Gap(format!("the Elixir rules do not apply: {error:?}")))?;
    // The materialiser is the authority on what Elixir has a form for,
    // so its refusal is exit 3 and every other stop is exit 6.
    let declarations = zerodds_lang_rules::adapter::elixir::elixir_file(&ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    let options = CodecOptions {
        map_length_with_delimiter: args.map_length_with_delimiter,
    };
    // What the chosen direction calls, and nothing else: a codec the
    // probe never reaches is source the compiler still expands.
    let codec = if args.input.is_some() {
        let decoders = decoder_from::<Elixir>(&ast, root, args.key, options)
            .map_err(|error| from_codec(&error))?;
        // Table 9's defaulters, because a decoder opens with
        // `v = zd_def_T()`; presence is what makes the name resolve.
        let mut all =
            elixir_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else {
        codec_from::<Elixir>(&ast, root, args.key, options).map_err(|error| from_codec(&error))?
    };

    let mut out = String::from(
        "# Generated by wireprobe-zerodds-elixir-gen. Do not edit.\n\
         #\n\
         # The octets come out of runtime/elixir when this runs. Nothing in\n\
         # this file decides a wire question.\n",
    );
    out.push_str(&zerodds_lang_render::elixir::file(&declarations));

    let mut module = String::new();
    for function in &codec {
        module.push('\n');
        module.push_str(&indent(&zerodds_lang_render::elixir::codec_function(
            function,
        )));
    }
    if args.input.is_some() {
        module.push_str(&dump::dump(&ast, root, args.key.union_label).map_err(Refusal::Gap)?);
    }

    out.push_str("\ndefmodule ZdCodec do\n");
    out.push_str(
        "  @moduledoc \"The generated codec: the shared walk's decisions, spelled in Elixir.\"\n",
    );
    // `import Bitwise` only where the walk emitted a bitwise operator:
    // an unused import is a warning, and warnings are failures in this
    // column's gates.
    if ["&&&", "|||", ">>>", "<<<"]
        .iter()
        .any(|operator| module.contains(operator))
    {
        out.push_str("  import Bitwise\n");
    }
    out.push_str(&module);
    out.push_str("end\n");
    out.push_str(&harness(args, &ast, root, sample.as_ref())?);
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
    let path = args.out_dir.join("main.exs");
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
        eprintln!("wireprobe-zerodds-elixir-gen: {}", refusal.message());
        std::process::exit(refusal.code());
    }
}

/// Two spaces in front of every non-empty line: the codec functions
/// render at the top level and live inside `defmodule ZdCodec`.
fn indent(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The PL_CDR1 framing one layout key adds up to, as the Elixir
/// expression that builds it. The flag policy is the measured name and
/// not a computed word, for the reason the Python generator writes
/// down: which bits ride on a parameter id is the runtime's answer.
fn framing_expression(key: LayoutKey) -> String {
    let headers = key.pl_cdr1_headers;
    let flag = |value: bool| if value { "true" } else { "false" };
    format!(
        "%ZeroddsWire.PlCdr1Framing{{\n\
         \x20 pid_flags: :\"{}\",\n\
         \x20 highest_short_id: {},\n\
         \x20 pad_declared_length: {},\n\
         \x20 pad_declared_optional_length: {},\n\
         \x20 member_must_understand: {},\n\
         \x20 pad_after_value: {}\n\
         }}",
        key.pid_flags.as_str(),
        key.short_pid_range.highest_short_id(),
        flag(headers.pads_declared_length()),
        flag(headers.pads_declared_optional_length()),
        flag(key.member_pid_carries_must_understand()),
        flag(headers.pads_after_value())
    )
}

/// The tail of `main.exs`: the sample and the stream out, or the bytes
/// in and the values out.
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    sample: Option<&serde_json::Value>,
) -> Result<String, Refusal> {
    let order = match args.endianness {
        Endianness::Little => ":le",
        Endianness::Big => ":be",
    };
    let max_align = args.encoding.max_alignment();
    let short_codes = if args.key.length_codes.short_code_where_it_fits() {
        "true"
    } else {
        "false"
    };

    // The harness lives in a module of its own and never at the top
    // level: a struct literal in a script's top-level context cannot
    // see a module the same file defines — "cannot access struct …",
    // measured on Elixir 1.20.2 — while a later module in the same
    // file can.
    let mut out = String::new();
    let _ = write!(out, "\nzd_framing = {}\n", framing_expression(args.key));

    if let Some(input) = &args.input {
        let decoder = decoder_name::<Elixir>(ast, root);
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        let _ = write!(
            out,
            "\nzd_input = File.read!({})\n",
            zerodds_lang_render::elixir::string_literal(&input.to_string_lossy())
        );
        if args.encapsulation {
            // The identifier is not re-read for the encoding — the
            // caller named one — but §7.6.3.1.2's options word is: its
            // low two bits say where the serialized data ended.
            out.push_str(
                "if byte_size(zd_input) < 4 do\n\
                 \x20 IO.write(:stderr, \"wireprobe-zerodds-elixir: fewer than the four-byte \
                 encapsulation header\\n\")\n\
                 \x20 System.halt(1)\n\
                 end\n\
                 zd_declared = Bitwise.band(:binary.at(zd_input, 3), 3)\n\
                 zd_body = binary_part(zd_input, 4, byte_size(zd_input) - 4)\n\
                 if byte_size(zd_body) < zd_declared do\n\
                 \x20 IO.write(:stderr, \"wireprobe-zerodds-elixir: the options field declares \
                 #{zd_declared} padding byte(s) and fewer follow the header\\n\")\n\
                 \x20 System.halt(1)\n\
                 end\n\
                 zd_body = binary_part(zd_body, 0, byte_size(zd_body) - zd_declared)\n",
            );
        } else {
            out.push_str("zd_body = zd_input\n");
        }
        let _ = writeln!(
            out,
            "zd_reader = ZeroddsWire.Reader.new(zd_body, {order}, {max_align}, zd_framing)"
        );
        let _ = write!(
            out,
            "# The top-level type is not a member of anything, so no @try_construct\n\
             # governs it and §7.2.2.7's default stands.\n\
             {{zd_value, zd_reader}} =\n\
             \x20 try do\n\
             \x20   ZdCodec.{decoder}(zd_reader, :discard)\n\
             \x20 rescue\n\
             \x20   zd_error in ZeroddsWire.WireError ->\n\
             \x20     IO.write(:stderr, \"wireprobe-zerodds-elixir: \" <> \
             Exception.message(zd_error) <> \"\\n\")\n\
             \x20     System.halt(1)\n\
             \x20 end\n\
             case ZeroddsWire.Reader.reader_status(zd_reader) do\n\
             \x20 :ok ->\n\
             \x20   :ok\n\
             \x20 :discarded ->\n\
             \x20   # Not a failed decode: §7.2.2.7, the receiver drops the object\n\
             \x20   # and carries on. Exit code 3, the Rust probe's.\n\
             \x20   IO.write(:stderr, \"wireprobe-zerodds-elixir: at \
             #{{ZeroddsWire.Reader.reader_position(zd_reader)}}: discarded\\n\")\n\
             \x20   System.halt(3)\n\
             \x20 zd_status ->\n\
             \x20   IO.write(:stderr, \"wireprobe-zerodds-elixir: at \
             #{{ZeroddsWire.Reader.reader_position(zd_reader)}}: #{{zd_status}}\\n\")\n\
             \x20   System.halt(1)\n\
             end\n\
             # Nothing may be left over that is not padding: a decoder that\n\
             # stopped short and said nothing is the silent-drop shape.\n\
             zd_left = ZeroddsWire.Reader.reader_remaining(zd_reader)\n\
             zd_pad = rem({max_align} - rem(ZeroddsWire.Reader.reader_position(zd_reader), \
             {max_align}), {max_align})\n\
             if zd_left > zd_pad do\n\
             \x20 IO.write(:stderr, \"wireprobe-zerodds-elixir: #{{zd_left}} byte(s) left at \
             offset #{{ZeroddsWire.Reader.reader_position(zd_reader)}}, of which at most \
             #{{zd_pad}} could be padding\\n\")\n\
             \x20 System.halt(1)\n\
             end\n"
        );
        let _ = write!(
            out,
            "zd_json =\n\
             \x20 try do\n\
             \x20   ZdCodec.{dumper}(zd_value, \"sample\")\n\
             \x20 rescue\n\
             \x20   zd_error in ArgumentError ->\n\
             \x20     IO.write(:stderr, \"wireprobe-zerodds-elixir: \" <> \
             Exception.message(zd_error) <> \"\\n\")\n\
             \x20     System.halt(1)\n\
             \x20 end\n\
             File.write!({path}, ZeroddsWire.Json.encode(zd_json) <> \"\\n\")\n",
            dumper = dump::dumper_name(ast, root),
            path = zerodds_lang_render::elixir::string_literal(&values_out.to_string_lossy())
        );
        return Ok(wrap_probe(&out));
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let filled = fill::fill(ast, root, sample).map_err(Refusal::Gap)?;
    let encoder = codec_name::<Elixir>(ast, root, Purpose::Data);

    let _ = write!(out, "\nzd_sample = {filled}\n\n");
    let _ = write!(
        out,
        "zd_writer = ZeroddsWire.Writer.new({order}, {max_align}, zd_framing, {short_codes})\n\
         # A value the type cannot hold is the sample's refusal and not the\n\
         # harness's: exit 1, the Rust probe's answer for the same sample.\n\
         zd_writer =\n\
         \x20 try do\n\
         \x20   ZdCodec.{encoder}(zd_writer, zd_sample)\n\
         \x20 rescue\n\
         \x20   zd_error in ZeroddsWire.WireError ->\n\
         \x20     IO.write(:stderr, \"wireprobe-zerodds-elixir: \" <> \
         Exception.message(zd_error) <> \"\\n\")\n\
         \x20     System.halt(1)\n\
         \x20 end\n\
         zd_body = ZeroddsWire.Writer.finish(zd_writer)\n\
         # Standard output leaves unicode mode first: the default device\n\
         # re-encodes octets above 0x7F as UTF-8, and the payload is not\n\
         # text.\n\
         :ok = :io.setopts(:standard_io, encoding: :latin1)\n"
    );
    if args.encapsulation {
        let header = encapsulation_header(args.encoding, args.endianness);
        let _ = write!(
            out,
            "# The encapsulation identifier §7.6.3.1.2 gives this encoding and\n\
             # byte order; the options field is zero here and the caller that\n\
             # wants a padding count writes it.\n\
             IO.binwrite(:stdio, <<{}, {}, {}, {}>>)\n",
            header[0], header[1], header[2], header[3]
        );
    }
    out.push_str("IO.binwrite(:stdio, zd_body)\n");
    Ok(wrap_probe(&out))
}

/// The probe body inside `defmodule ZdProbe … def main`, and the call
/// that runs it.
fn wrap_probe(body: &str) -> String {
    let mut out = String::from(
        "\n# -- the probe ------------------------------------------------------\n\
         \ndefmodule ZdProbe do\n\
         \x20 @moduledoc false\n\
         \x20 def main do\n",
    );
    for line in body.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("  end\nend\n\nZdProbe.main()\n");
    out
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
        (Encoding::PlCdr2, Endianness::Big) => 0x000A,
        (Encoding::PlCdr2, Endianness::Little) => 0x000B,
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

/// The model this column generates from, lowered **in the dialect** it
/// will be planned for.
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
