// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-python-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-python-gen` — the Python module the Python probe
//! runs.
//!
//! One IDL file, one type, one JSON sample and one layout key in; one
//! `probe.py` holding the dataclasses, the codec, the sample and the
//! statements that write it to standard output.
//! `tools/wireprobe/wireprobe-zerodds-python-walk` is the shell around
//! it, and that script's header says why it is not called
//! `wireprobe-zerodds-python`: that name belongs to the column that
//! reaches the same runtime through `crates/lang-python`, which carries
//! a second copy of the walk.
//!
//! One file, because a Python module is a file and a second one would
//! need a package directory and a `sys.path` entry to find it — one more
//! thing between the generator and the octets.
//!
//! Nothing here writes a byte. The declarations come from the Python
//! materialiser, the codec from the shared walk through
//! [`zerodds_lang_rules::emit::python::Python`], and the bytes come out
//! of `runtime/python/zerodds_wire` when the module runs.
//! `zerodds_wire` implements DDS-XTypes 1.3 §7.4 from the clause and
//! calls nothing in Rust, which is what makes this a further reading of
//! the specification rather than a rendering of the first one's answer.
//!
//! ## Both directions
//!
//! `--values` generates the write direction: the sample is filled from
//! the JSON and the octets go to standard output. `--in` generates the
//! read direction: the octets are read from the named file and the
//! values go to the file `--values-out` names, as plain JSON in the
//! dialect `crates/wire-probe/src/lib.rs` documents.
//!
//! This is where the column differs from the TypeScript one, which has
//! no reader at all: `emit/python.rs`'s [`CodecTarget::default_into`]
//! answers through `zerodds_lang_rules::default_python`, so Table 9 has
//! a Python spelling and `decoder_from` has somewhere to start each
//! sample from.
//!
//! ## Not canonical form
//!
//! What `--values-out` receives is plain JSON and the caller
//! canonicalises it in Rust, which is what the C and C# columns do. A
//! second canonicaliser in a second language would be a second chance
//! for two columns to part over a spelling rather than over a value.
//!
//! [`CodecTarget::default_into`]: zerodds_lang_rules::emit::CodecTarget::default_into
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

mod dump;
mod fill;

use std::fmt::Write as _;
use std::path::PathBuf;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{
    CollectionDelimiterPolicy, Dialect, Encoding, Endianness, EnumHolderWidth, EnumeratorValue,
    FixedDigitCount, LayoutKey, LengthCodePolicy, MapDelimiter, MapEntryOrder, MutableUnionFraming,
    PidFlagPolicy, Purpose, ShortPidRange, UnionLabelMatch, WcharFraming, WcharWidth,
    WstringLengthPolicy, WstringUnitOrder,
};
use zerodds_lang_rules::default_python::python_defaulters_called_by;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::emit::python::Python;
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

const USAGE: &str = "\
wireprobe-zerodds-python-gen — the Python the ZeroDDS Python probe runs

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
    --length-codes <name>   compact | universal-nextint
    --collection-delimiters <name>
                            specification | enum-is-primitive | never
    --wchar-framing <name>  fixed | giop-length-prefixed
    --wchar-width <2|4>     how wide a scalar `wchar` is, which is not
                            the width of a `wstring`'s code units
    --map-delimiter <name>  either-half | value-only
    --pid-flags <name>      sentinel-must-understand | bare |
                            impl-extension-members
    --enum-holder <name>    declared | bit-bound-dropped
    --union-labels <name>   sign-extended | negative-unmatched-below-32
    --enumerator-value <name>
                            declared | ordinal
    --union-framing <name>  specification | branches-from-zero
    --short-pid-range <name>
                            up-to-reserved | all-fourteen-bits
    --map-entries <name>    declaration | ascending-key
    --fixed-digits <name>   declared | value-precision
    --wstring-units <name>  stream | big-endian-always | byte-order-mark
    --wstring-length <name> specification | character-count |
                            character-count-terminated
    --map-length <name>     sequence-equivalent | rule-15-omits
    --out-dir <dir>         where probe.py goes (required)

A dialect sets every wire question the implementations answer
differently at once, and each policy flag above overrides one of its
answers — the same two-step `zerodds-wireprobe-gen`'s shared `parse<C>`
takes for the C#, Java, C, C++ and TypeScript columns.

The defaults are `wireprobe-zerodds`'s, because a Python column that
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
    /// Overrides the dialect's EMHEADER length-code answer, when given.
    /// `wireprobe-python-matrix`'s `universal-nextint` configuration is
    /// the one caller that sets this outside the dialect presets.
    pub length_codes: Option<LengthCodePolicy>,
    /// Which collections carry a delimiter.
    pub collection_delimiters: Option<CollectionDelimiterPolicy>,
    /// How a `wstring`'s length is counted.
    pub wstring_length: Option<WstringLengthPolicy>,
    /// How a scalar `wchar` is framed.
    pub wchar_framing: Option<WcharFraming>,
    /// How wide a scalar `wchar` is.
    pub wchar_width: Option<WcharWidth>,
    /// Which of a `map`'s halves decides whether the map is delimited.
    pub map_delimiter: Option<MapDelimiter>,
    /// In what byte order a `wstring`'s code units go out.
    pub wstring_unit_order: Option<WstringUnitOrder>,
    /// What flags a PL_CDR1 parameter id carries, per position.
    pub pid_flags: Option<PidFlagPolicy>,
    /// How wide an enumeration's holder is.
    pub enum_holder: Option<EnumHolderWidth>,
    /// Whether a negative case label of a narrow signed discriminator
    /// selects its branch.
    pub union_label: Option<UnionLabelMatch>,
    /// In what order a `map`'s entries go on the wire.
    pub map_entries: Option<MapEntryOrder>,
    /// Which integer an enumerator serialises as.
    pub enumerator_value: Option<EnumeratorValue>,
    /// How a `@mutable` union numbers its branches, and whether its
    /// XCDR1 parameter list is terminated.
    pub union_framing: Option<MutableUnionFraming>,
    /// Which member ids fit a PL_CDR1 short parameter header.
    pub short_pid_range: Option<ShortPidRange>,
    /// Whether a `fixed<P,S>` carries its declared digit count or the
    /// value's.
    pub fixed_digits: Option<FixedDigitCount>,
    /// Overrides whether a delimited `map` also carries its entry count,
    /// when given. `wireprobe-python-matrix`'s `rule-15-omits`
    /// configuration is the one caller that sets this outside the
    /// dialect presets.
    pub map_length_with_delimiter: Option<bool>,
    /// Where the file goes.
    pub out_dir: PathBuf,
}

impl Default for Args {
    /// The defaults `wireprobe-zerodds` has, because a column that
    /// defaulted differently would report a divergence of its own
    /// making — the same defaults `zerodds-wireprobe-gen::Args::default`
    /// gives the other five columns.
    fn default() -> Self {
        Self {
            idl: PathBuf::new(),
            type_name: String::new(),
            values: None,
            input: None,
            values_out: None,
            encoding: Encoding::PlainCdr2,
            endianness: Endianness::Little,
            encapsulation: false,
            dialect: Dialect::Specification,
            length_codes: None,
            collection_delimiters: None,
            wstring_length: None,
            wchar_framing: None,
            wchar_width: None,
            map_delimiter: None,
            wstring_unit_order: None,
            pid_flags: None,
            enum_holder: None,
            union_label: None,
            map_entries: None,
            enumerator_value: None,
            union_framing: None,
            short_pid_range: None,
            fixed_digits: None,
            map_length_with_delimiter: None,
            out_dir: PathBuf::new(),
        }
    }
}

/// Why the generator could not produce a file.
///
/// Three kinds and not one string, because the probe's exit code is the
/// verdict `docs/specs/zerodds-verdicts-1.0.md` records and a caller
/// cannot re-derive it from a message. A generator that answered one
/// code for all three would put "Python has no form for this" and "our
/// walk stopped" in one bucket, which is the distinction the whole
/// ledger turns on.
#[derive(Debug)]
pub enum Refusal {
    /// The model is outside what the specification admits, in the
    /// planner's own words. Exit 1 — `refused` in
    /// `docs/specs/zerodds-verdicts-1.0.md`, "it was asked and said no".
    Refused(String),
    /// The command line. Exit 2.
    Usage(String),
    /// Python has no form for the construct. Exit 3.
    ///
    /// The materialiser's answer and nobody else's: it is the one place
    /// that decides what an IDL construct becomes here, and it names
    /// what it refused rather than leaving the declaration out.
    NoForm(String),
    /// A tool of ours has not written it. Exit 6.
    ///
    /// The shared walk stopping, the sample not matching the model, the
    /// IDL not parsing — everything that is a gap in this tree rather
    /// than a property of Python.
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
    let mut length_codes = None;
    let mut collection_delimiters = None;
    let mut wstring_length = None;
    let mut wchar_framing = None;
    let mut wchar_width = None;
    let mut map_delimiter = None;
    let mut wstring_unit_order = None;
    let mut pid_flags = None;
    let mut enum_holder = None;
    let mut union_label = None;
    let mut map_entries = None;
    let mut enumerator_value = None;
    let mut union_framing = None;
    let mut short_pid_range = None;
    let mut fixed_digits = None;
    let mut map_length_with_delimiter = None;
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
                // `Dialect::parse` and not a hand-rolled scan over
                // `Dialect::ALL`: it is the same lookup
                // `zerodds-wireprobe-gen`'s shared `parse<C>` and
                // `wireprobe-zerodds`'s own CLI use, so a dialect this
                // column accepts is a dialect every other caller of the
                // canonical parser accepts too, under the one spelling.
                dialect = Dialect::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--dialect: `{name}` is not one of them"))
                })?;
            }
            "--length-codes" => {
                let name = take()?;
                length_codes = Some(LengthCodePolicy::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--length-codes: `{name}` is not one of them"))
                })?);
            }
            "--map-length" => {
                let name = take()?;
                map_length_with_delimiter = Some(match name.as_str() {
                    "sequence-equivalent" => true,
                    "rule-15-omits" => false,
                    other => {
                        return Err(Refusal::Usage(format!(
                            "--map-length: `{other}` is not one of them"
                        )));
                    }
                });
            }
            "--collection-delimiters" => {
                let name = take()?;
                collection_delimiters =
                    Some(CollectionDelimiterPolicy::parse(&name).ok_or_else(|| {
                        Refusal::Usage(format!(
                            "--collection-delimiters: `{name}` is not one of them"
                        ))
                    })?);
            }
            "--wstring-length" => {
                let name = take()?;
                wstring_length = Some(WstringLengthPolicy::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--wstring-length: `{name}` is not one of them"))
                })?);
            }
            "--wchar-framing" => {
                let name = take()?;
                wchar_framing = Some(WcharFraming::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--wchar-framing: `{name}` is not one of them"))
                })?);
            }
            "--wchar-width" => {
                let name = take()?;
                wchar_width = Some(WcharWidth::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--wchar-width: `{name}` is not one of them"))
                })?);
            }
            "--map-delimiter" => {
                let name = take()?;
                map_delimiter = Some(MapDelimiter::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--map-delimiter: `{name}` is not one of them"))
                })?);
            }
            "--wstring-units" => {
                let name = take()?;
                wstring_unit_order = Some(WstringUnitOrder::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--wstring-units: `{name}` is not one of them"))
                })?);
            }
            "--pid-flags" => {
                let name = take()?;
                pid_flags = Some(PidFlagPolicy::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--pid-flags: `{name}` is not one of them"))
                })?);
            }
            "--enum-holder" => {
                let name = take()?;
                enum_holder = Some(EnumHolderWidth::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--enum-holder: `{name}` is not one of them"))
                })?);
            }
            "--union-labels" => {
                let name = take()?;
                union_label = Some(UnionLabelMatch::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--union-labels: `{name}` is not one of them"))
                })?);
            }
            "--map-entries" => {
                let name = take()?;
                map_entries = Some(MapEntryOrder::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--map-entries: `{name}` is not one of them"))
                })?);
            }
            "--enumerator-value" => {
                let name = take()?;
                enumerator_value = Some(EnumeratorValue::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--enumerator-value: `{name}` is not one of them"))
                })?);
            }
            "--union-framing" => {
                let name = take()?;
                union_framing = Some(MutableUnionFraming::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--union-framing: `{name}` is not one of them"))
                })?);
            }
            "--short-pid-range" => {
                let name = take()?;
                short_pid_range = Some(ShortPidRange::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--short-pid-range: `{name}` is not one of them"))
                })?);
            }
            "--fixed-digits" => {
                let name = take()?;
                fixed_digits = Some(FixedDigitCount::parse(&name).ok_or_else(|| {
                    Refusal::Usage(format!("--fixed-digits: `{name}` is not one of them"))
                })?);
            }
            other => return Err(Refusal::Usage(format!("`{other}` is not an option"))),
        }
    }

    // One direction per run. `--in` and `--values` together is a caller
    // that has not decided which way the octets go, and picking one for
    // it would produce a probe that quietly ignores half the command
    // line.
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
    // Named and not standard output: the caller canonicalises the
    // document in Rust and needs a path to hand the canonicaliser, and a
    // probe that printed it would mix the values with whatever the
    // interpreter writes to the same stream.
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
        length_codes,
        collection_delimiters,
        wstring_length,
        wchar_framing,
        wchar_width,
        map_delimiter,
        wstring_unit_order,
        pid_flags,
        enum_holder,
        union_label,
        map_entries,
        enumerator_value,
        union_framing,
        short_pid_range,
        fixed_digits,
        map_length_with_delimiter,
        out_dir: out_dir.ok_or_else(|| Refusal::Usage("--out-dir is required".to_string()))?,
    })
}

/// Generate `probe.py`.
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
    let applied = apply(&graph, &rule::python::catalogue())
        .map_err(|error| Refusal::Gap(format!("the Python rules do not apply: {error:?}")))?;
    // The materialiser is the authority on what Python has a form for,
    // so its refusal is exit 3 and every other stop is exit 6.
    let mut module = zerodds_lang_rules::adapter::python::python_module(&ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    let key = layout_key(args);
    // What the chosen direction calls, and nothing else. A codec the
    // probe never reaches is source the interpreter still compiles, and
    // generating both halves would double every case in the corpus
    // sweep.
    let codec = if args.input.is_some() {
        let decoders = decoder_from::<Python>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        // Table 9's defaulters first, because a decoder opens with
        // `v = zd_def_T()`. Python resolves a global at the call and not
        // at the `def`, so the order is not what makes the name resolve
        // — the *presence* is, and a file without them is a `NameError`
        // at the first sample.
        let mut all =
            python_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else {
        codec_from::<Python>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?
    };

    module.imports.extend(runtime_import());
    module
        .imports
        .push(zerodds_lang_cst::python::Import::Module {
            path: zerodds_lang_cst::common::Path::single("sys"),
            alias: None,
        });
    if args.input.is_some() {
        module
            .imports
            .push(zerodds_lang_cst::python::Import::Module {
                path: zerodds_lang_cst::common::Path::single("json"),
                alias: None,
            });
    }
    module.body.extend(codec);

    let mut out = zerodds_lang_render::python::module(&module);
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
    let path = args.out_dir.join("probe.py");
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
        eprintln!("wireprobe-zerodds-python-gen: {}", refusal.message());
        std::process::exit(refusal.code());
    }
}

/// Everything the generated module names from the runtime.
///
/// Imported wholesale rather than per case, for the reason the Java
/// probe imports wholesale: which policy a configuration reaches is the
/// layout key's answer, and a file that imported only what one key
/// needed would raise a `NameError` under another.
///
/// **Three lines and not one.**
///
/// `utf8_octets` and `utf16_units` are module-level functions of
/// `zerodds_wire.typesupport` that the package's `__init__` does not
/// re-export — measured against its `__all__` — and `emit/decode.rs`
/// reaches both by bare name when a bounded `string` or `wstring` is
/// trimmed under §7.3.1.2.1.12.3. A single import of the package would
/// be an `ImportError` naming them at the top of every read-direction
/// file that has one.
///
/// The third line is `import zerodds_wire as zd_wire`, and it is what
/// the **harness** names the runtime through. A generated declaration
/// binds its name at the module root, after the imports, so a corpus
/// case declaring `enum Status` rebinds `Status` and
/// `Status.DISCARDED` becomes an `AttributeError` — measured on
/// `enum-sparse-values` and its unannotated twin, which is the same
/// collision `wireprobe-zerodds-csharp-gen` records against CS0117.
/// The bare names stay for the codec, which
/// [`zerodds_lang_rules::emit::python::Python`] writes and this crate
/// does not choose the spelling of.
fn runtime_import() -> Vec<zerodds_lang_cst::python::Import> {
    use zerodds_lang_cst::common::{Ident, Path};
    use zerodds_lang_cst::python::{Import, ImportName};
    let name = |text: &str| ImportName {
        name: Ident::new(text),
        alias: None,
    };
    let exported = [
        // The two the harness constructs and the codec is annotated
        // with.
        "Endianness",
        "PlCdr1Framing",
        "Reader",
        "Writer",
        // The handles a generated codec declares locals of.
        "DHeader",
        "EmHeader",
        "EmHeaderRead",
        "Pid",
        "PidRead",
        "ReaderMark",
        // The policy values `emit/python.rs`'s `constant` writes.
        "ParameterRole",
        "Status",
        "TryConstruct",
        // A `long double`, which is sixteen octets and never a Python
        // `float`.
        "Float128",
        // The delimiter a `@final` aggregate does not write.
        "no_dheader",
        // The union-overlay patterns `emit/encode.rs` reads a
        // suppressed arm's bits through.
        "bit_pattern_bool",
        "bit_pattern_f32",
        "bit_pattern_f64",
        "bit_pattern_int",
        // What a refused write raises. The harness catches it by name.
        "WireError",
    ];
    vec![
        Import::From {
            path: Path::single("zerodds_wire"),
            names: exported.into_iter().map(name).collect(),
        },
        Import::From {
            path: Path::new(&["zerodds_wire", "typesupport"]),
            names: vec![name("utf16_units"), name("utf8_octets")],
        },
        Import::Module {
            path: Path::single("zerodds_wire"),
            alias: Some(Ident::new(RUNTIME)),
        },
    ]
}

/// What the harness names the runtime through.
///
/// `zd_` because that is the prefix every generated local and every
/// generated function in this tree already carries, and no IDL
/// identifier the corpus declares reaches into it.
const RUNTIME: &str = "zd_wire";

/// The tail of `probe.py`: the sample and the two streams, or the bytes
/// and the values.
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    sample: Option<&serde_json::Value>,
) -> Result<String, Refusal> {
    // Every runtime name the harness writes goes through `zd_wire`, and
    // none of them is written bare. See [`runtime_import`]: a generated
    // declaration binds its name at the module root and would take a
    // bare one over.
    let order = match args.endianness {
        Endianness::Little => format!("{RUNTIME}.Endianness.LITTLE"),
        Endianness::Big => format!("{RUNTIME}.Endianness.BIG"),
    };
    let max_align = args.encoding.max_alignment();

    let mut out = String::from(
        "\n\n# -- the probe ------------------------------------------------------\n\
         #\n\
         # Generated by wireprobe-zerodds-python-gen. Do not edit.\n\
         #\n\
         # The octets come out of runtime/python/zerodds_wire when this runs.\n\
         # Nothing below decides a wire question.\n",
    );

    if let Some(input) = &args.input {
        let decoder = decoder_name::<Python>(ast, root);
        let strip = usize::from(args.encapsulation) * 4;
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        let _ = write!(
            out,
            "\nwith open({}, \"rb\") as zd_handle:\n\
             \x20   zd_input = zd_handle.read()\n",
            zerodds_lang_render::python::string_literal(&input.to_string_lossy())
        );
        if strip > 0 {
            // The encapsulation identifier is not re-read for the
            // encoding: the caller named one, and a probe that took the
            // stream's word for it could not be pointed at a stream to
            // test what it does with a header it disagrees with.
            //
            // The **options** word is read, because §7.6.3.1.2 says a
            // receiver shall use its low two bits to find where the
            // serialized data ended. Without them the trailing padding
            // is handed to the decoder as data.
            let _ = write!(
                out,
                "if len(zd_input) < {strip}:\n\
                 \x20   sys.stderr.write(\"wireprobe-zerodds-python: fewer than the four-byte \
                 encapsulation header\\n\")\n\
                 \x20   raise SystemExit(1)\n\
                 zd_declared = zd_input[3] & 3\n\
                 zd_body = zd_input[{strip}:]\n\
                 if len(zd_body) < zd_declared:\n\
                 \x20   sys.stderr.write(\n\
                 \x20       f\"wireprobe-zerodds-python: the options field declares \
                 {{zd_declared}} padding \"\n\
                 \x20       f\"byte(s) and only {{len(zd_body)}} follow the header\\n\"\n\
                 \x20   )\n\
                 \x20   raise SystemExit(1)\n\
                 zd_body = zd_body[: len(zd_body) - zd_declared]\n"
            );
        } else {
            out.push_str("zd_body = zd_input\n");
        }
        let _ = writeln!(
            out,
            "zd_reader = {RUNTIME}.Reader(zd_body, {order}, {max_align}, {}, {})",
            zerodds_lang_render::python::string_literal(key.pid_flags.as_str()),
            key.short_pid_range.highest_short_id()
        );
        out.push_str(&dump::dump(ast, root, key.union_label).map_err(Refusal::Gap)?);
        let _ = write!(
            out,
            "\n# The top-level type is not a member of anything, so no\n\
             # `@try_construct` governs it and §7.2.2.7's default stands.\n\
             try:\n\
             \x20   zd_value = {decoder}(zd_reader, {RUNTIME}.TryConstruct.DISCARD)\n\
             except {RUNTIME}.WireError as zd_error:\n\
             \x20   sys.stderr.write(f\"wireprobe-zerodds-python: {{zd_error}}\\n\")\n\
             \x20   raise SystemExit(1) from zd_error\n\
             # A refusal about the *type*, which the reader records rather than\n\
             # raising: the generated decoder walks to the end of the sample so\n\
             # that one test here decides what the whole read was.\n\
             if zd_reader.reader_status() is {RUNTIME}.Status.DISCARDED:\n\
             \x20   # Not a failed decode. DDS-XTypes 1.3 §7.2.2.7: the receiver\n\
             \x20   # drops the object and carries on, and a caller that cannot tell\n\
             \x20   # that from a broken peer cannot tell a narrowed type from a\n\
             \x20   # corrupt stream. Exit code 3, the Rust probe's.\n\
             \x20   sys.stderr.write(\n\
             \x20       f\"wireprobe-zerodds-python: at {{zd_reader.reader_position()}}: \
             discarded\\n\"\n\
             \x20   )\n\
             \x20   raise SystemExit(3)\n\
             if zd_reader.reader_status() is not {RUNTIME}.Status.OK:\n\
             \x20   sys.stderr.write(\n\
             \x20       f\"wireprobe-zerodds-python: at {{zd_reader.reader_position()}}: \"\n\
             \x20       f\"{{zd_reader.reader_status().value}}\\n\"\n\
             \x20   )\n\
             \x20   raise SystemExit(1)\n\
             # Nothing may be left over that is not padding. A decoder that\n\
             # stopped short and said nothing is the silent-drop shape: the\n\
             # sample comes back looking plausible with its last members\n\
             # missing. Padding is the one thing a writer legitimately leaves\n\
             # behind, and it is what alignment asks for from this offset and\n\
             # never more.\n\
             zd_left = zd_reader.reader_remaining()\n\
             zd_pad = ({max_align} - zd_reader.reader_position() % {max_align}) % {max_align}\n\
             if zd_left > zd_pad:\n\
             \x20   sys.stderr.write(\n\
             \x20       f\"wireprobe-zerodds-python: {{zd_left}} byte(s) left at offset \"\n\
             \x20       f\"{{zd_reader.reader_position()}}, of which at most {{zd_pad}} could be \"\n\
             \x20       \"padding\\n\"\n\
             \x20   )\n\
             \x20   raise SystemExit(1)\n\
             # Plain JSON in the dialect crates/wire-probe/src/lib.rs documents\n\
             # and not canonical form: the caller canonicalises in Rust, which is\n\
             # what the C and C# columns do.\n\
             with open({}, \"w\", encoding=\"utf-8\") as zd_out:\n\
             \x20   zd_out.write(json.dumps({}(zd_value, \"sample\")))\n\
             \x20   zd_out.write(\"\\n\")\n",
            zerodds_lang_render::python::string_literal(&values_out.to_string_lossy()),
            dump::dumper_name(ast, root)
        );
        return Ok(out);
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let sample_type = zerodds_lang_render::python::type_(
        &<Python as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let filled = fill::fill(ast, root, sample).map_err(Refusal::Gap)?;
    let encoder = codec_name::<Python>(ast, root, Purpose::Data);
    let framing = framing_expression(key);
    let short_codes = if key.length_codes.short_code_where_it_fits() {
        "True"
    } else {
        "False"
    };

    let _ = write!(out, "\nzd_sample: {sample_type} = {filled}\n\n");
    let _ = write!(out, "zd_framing = {framing}\n\n");
    let _ = write!(
        out,
        "zd_writer = {RUNTIME}.Writer({order}, {max_align}, zd_framing, {short_codes})\n\
         # A value the type cannot hold is the sample's refusal and not the\n\
         # harness's: `char above U+00FF` is §7.2.2.2.1.2.2, and the Rust probe\n\
         # answers 1 for the same sample. Exit 6 here would file it as\n\
         # `harness-gap` — a tool of ours that has not been written — which is a\n\
         # different thing from a runtime that read the clause and said no.\n\
         # `docs/specs/zerodds-verdicts-1.0.md`.\n\
         try:\n\
         \x20   {encoder}(zd_writer, zd_sample)\n\
         except {RUNTIME}.WireError as zd_error:\n\
         \x20   sys.stderr.write(f\"wireprobe-zerodds-python: {{zd_error}}\\n\")\n\
         \x20   raise SystemExit(1) from zd_error\n\
         zd_body = zd_writer.finish()\n"
    );
    if args.encapsulation {
        let header = encapsulation_header(args.encoding, args.endianness);
        // §7.4.1.1 pads the serialized data to the next four-byte
        // boundary with zeros, and §7.6.3.1.2 puts the count of those
        // bytes in the options field's low two bits — the same
        // convention `crates/wire-codec/src/encode.rs`'s own
        // `Emit::PayloadWithEncapsulation` arm and
        // `wireprobe-zerodds-c-gen`'s `zd_pad` use. Both halves or
        // neither: padding without the count hands a conforming receiver
        // up to three bytes of zero as data, and the count without the
        // padding tells it to cut bytes that are data. Computed here and
        // not at generation time, because the body's length is not known
        // until `zd_body` exists.
        let _ = write!(
            out,
            "# The encapsulation identifier is this encoding and byte order;\n\
             # its options field's low two bits carry the count of the zero\n\
             # bytes that pad the body to the next four-byte boundary.\n\
             zd_pad = (4 - len(zd_body) % 4) % 4\n\
             sys.stdout.buffer.write(bytes(({}, {}, {}, {} | zd_pad)))\n",
            header[0], header[1], header[2], header[3]
        );
    }
    out.push_str(
        "# `.buffer`, because `sys.stdout` is a text stream and the payload is\n\
         # not text: writing it through the encoder would re-encode every octet\n\
         # above 0x7F and, on Windows, turn 0x0A into two bytes.\n\
         sys.stdout.buffer.write(zd_body)\n",
    );
    if args.encapsulation {
        // The padding itself, after the body and still inside the
        // encapsulated form: `Emit::Payload` stays unpadded — that is
        // the serialized data itself — and only this form carries it.
        out.push_str("if zd_pad:\n    sys.stdout.buffer.write(bytes(zd_pad))\n");
    }
    out.push_str("sys.stdout.buffer.flush()\n");
    Ok(out)
}

/// The PL_CDR1 framing one layout key adds up to, as the Python
/// expression that builds it.
///
/// The arguments are named and not positional: `PlCdr1Framing` is a
/// frozen dataclass of five fields, three of them `bool`, and a
/// positional call that drifted against the runtime's field order would
/// put one measured divergence where another belongs and still run.
///
/// The flag policy is a *string* and not a computed word: which bits
/// ride on a parameter id at which position is a measured divergence
/// between implementations, and `zerodds_wire`'s `pid_flags_at` is where
/// this column keeps its answer. A generator that wrote `0x4000` would
/// have moved a wire decision out of the runtime and into the Rust tree,
/// which is the one thing a further column may not do.
fn framing_expression(key: LayoutKey) -> String {
    let headers = key.pl_cdr1_headers;
    let flag = |value: bool| if value { "True" } else { "False" };
    format!(
        "{RUNTIME}.PlCdr1Framing(\n\
         \x20   pid_flags={},\n\
         \x20   highest_short_id={},\n\
         \x20   pad_declared_length={},\n\
         \x20   pad_declared_optional_length={},\n\
         \x20   member_must_understand={},\n\
         \x20   pad_after_value={},\n\
         )",
        zerodds_lang_render::python::string_literal(key.pid_flags.as_str()),
        key.short_pid_range.highest_short_id(),
        flag(headers.pads_declared_length()),
        flag(headers.pads_declared_optional_length()),
        flag(key.member_pid_carries_must_understand()),
        flag(headers.pads_after_value())
    )
}

/// The layout key the flags add up to, read off the same place the C
/// and TypeScript columns read it.
///
/// The dialect first, then each override that was given — the order
/// `wireprobe-zerodds` and `zerodds-wireprobe-gen::Args::layout_key`
/// apply them in, and the only order that leaves a flag left off free to
/// leave the dialect's own answer alone.
fn layout_key(args: &Args) -> LayoutKey {
    let mut key =
        LayoutKey::in_dialect(args.dialect, args.encoding, args.endianness, Purpose::Data);
    if let Some(policy) = args.length_codes {
        key = key.with_length_codes(policy);
    }
    if let Some(policy) = args.collection_delimiters {
        key = key.with_collection_delimiters(policy);
    }
    if let Some(policy) = args.wstring_length {
        key = key.with_wstring_length(policy);
    }
    if let Some(width) = args.wchar_width {
        key = key.with_wchar_width(width);
    }
    if let Some(policy) = args.map_delimiter {
        key = key.with_map_delimiter(policy);
    }
    if let Some(framing) = args.wchar_framing {
        key = key.with_wchar_framing(framing);
    }
    if let Some(order) = args.wstring_unit_order {
        key = key.with_wstring_unit_order(order);
    }
    if let Some(policy) = args.enum_holder {
        key = key.with_enum_holder(policy);
    }
    if let Some(policy) = args.union_label {
        key = key.with_union_label(policy);
    }
    if let Some(policy) = args.map_entries {
        key = key.with_map_entries(policy);
    }
    if let Some(policy) = args.enumerator_value {
        key = key.with_enumerator_value(policy);
    }
    if let Some(policy) = args.union_framing {
        key = key.with_union_framing(policy);
    }
    if let Some(policy) = args.short_pid_range {
        key = key.with_short_pid_range(policy);
    }
    if let Some(policy) = args.fixed_digits {
        key = key.with_fixed_digits(policy);
    }
    if let Some(policy) = args.pid_flags {
        key = key.with_pid_flags(policy);
    }
    key
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
/// ids under every dialect — measured as four cells per BE option set
/// against `matrix/zerodds-rust`.
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
        map_length_with_delimiter: args
            .map_length_with_delimiter
            .unwrap_or(!args.dialect.map_count_omitted_when_delimited()),
    }
}
