// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds-idlc` — read OMG IDL 4.2 and say what is in it, or what is
//! wrong with it, or turn it into a language's source.
//!
//! ```text
//! zerodds-idlc check      FILE...   parse and lower; report every diagnostic
//! zerodds-idlc lint       FILE...   the rules a valid file can still break
//! zerodds-idlc format     FILE...   the canonical form, to stdout, --check or --write
//! zerodds-idlc dump-ast   FILE...   the model, as a tree
//! zerodds-idlc print-deps FILE...   the types each declaration names
//! zerodds-idlc generate   --lang L [--out DIR] [-I DIR] [--stamp F --depfile F --output-manifest F] FILE...
//! zerodds-idlc dlrl       --lang L [--out DIR] [-I DIR] [--stamp F --depfile F --output-manifest F] FILE...
//! ```
//!
//! The first five say what is in a file. `generate` and `dlrl` are the
//! two that write something back out.
//!
//! `generate` is the wire path: it calls `zerodds_lang_rules::generate::source`
//! for the model — the declarations and the codec, the same walk every
//! `tools/wireprobe/wireprobe-zerodds-<lang>-gen` runs to build a probe
//! program, minus the `--dialect`, `--values` and octet comparison a
//! measurement needs and an application never asks for.
//!
//! `dlrl` is not: OMG DDS 1.2 §8's Data Local Reconstruction Layer sits
//! above DCPS, not on the wire, and its three pragmas —
//! `DCPS_DATA_TYPE`, `DCPS_DATA_KEY`, `DCPS_DLRL_RELATION` — are read
//! independently of `generate`'s AST pipeline, by `zerodds_dlrl`'s own
//! text parser (`mod dlrl`). Two of those three spellings collide with
//! OpenDDS's key-marking pragmas of the same name, which `idl-lower`
//! reads for every other command; see that crate's `tests/dialects.rs`
//! for how the two readings are told apart. `dlrl` covers the four
//! languages OMG DLRL has no Rust PSM for and this tree does not
//! otherwise generate for DLRL: C++, C#, Java, TypeScript.
//!
//! ## Dialect
//!
//! Files are parsed under `Dialect::ALL_VENDORS` unless `--strict` says
//! otherwise. A tool pointed at a file somebody else wrote should read it
//! before it judges it — refusing a vendor extension at the lexer means
//! never reaching the rule that would have been worth reporting.
//! `--strict` is OMG IDL 4.2 and nothing else, which is the question
//! "would every vendor accept this". `generate` reads under the same
//! rule, but what it emits answers a different question: a dialect is
//! what a measurement reproduces when it stands in for another vendor,
//! and a generated application has no vendor to reproduce, so its
//! `--encoding`/`--endian` are always built against the specification's
//! own reading.
//!
//! The two do not reach the output the same way. `--encoding` changes
//! the emitted bodies: XCDR1 frames an `@appendable` struct without the
//! `DHEADER` XCDR2 gives it, so the same model is two different codecs
//! under the two flags. What it selects is the *version*, though, not
//! the per-type encoding identifier: XTypes 1.3 §7.4.2 makes that a
//! function of the type's own extensibility — `@final` is PLAIN_CDR2,
//! `@appendable` DELIMITED_CDR, `@mutable` PL_CDR2 — which is why
//! `zerodds_ast_layout` reads `Encoding` through `is_v1`/`is_v2` and
//! `max_alignment` and nothing else, and why `xcdr2`, `delimited-cdr`
//! and `pl-cdr2` emit one text. `xcdr1` and `pl-cdr1` are the same
//! pair on the other version. `--endian` changes no character of them, and
//! that is the design: every wire runtime holds the byte order in its
//! writer and its reader, so the same `w.WriteI32(v)` writes either
//! order and the choice is made where the writer is built. What
//! `--endian` does reach is the preamble, which names the order the file
//! was generated for as the constant a caller hands that writer.

#![allow(clippy::print_stdout, clippy::print_stderr)]

mod c_support;
mod csharp_support;
mod dlrl;
mod dump;
mod format;
mod generate;
mod java_support;
mod lint;
mod python_support;
mod report;
mod source;
mod type_metadata;
mod typescript_support;

use std::path::PathBuf;
use std::process::ExitCode;

use zerodds_ast_layout::{
    AXES, AxisFlag, Dialect, DialectSettings, Encoding, Endianness, dialect_names,
    dialect_settings_from_overrides,
};
use zerodds_lang_cst::Language;
use zerodds_lang_rules::adapter::interface::ServiceMapping;

use report::{Form, Tally};
use source::Unit;

/// What the tool was asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    /// Parse and lower, reporting both stages' diagnostics.
    Check,
    /// The rules a file can break while still being valid IDL.
    Lint,
    /// The canonical form — to stdout, or `--check`, or `--write`.
    Format,
    /// The model as a tree.
    DumpAst,
    /// The types each declaration names.
    PrintDeps,
    /// One language's source for the model.
    Generate,
    /// DDS 1.2 §8 DLRL object and home classes, from the three DLRL
    /// pragmas.
    Dlrl,
}

impl Command {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "check" => Some(Self::Check),
            "lint" => Some(Self::Lint),
            "format" => Some(Self::Format),
            "dump-ast" => Some(Self::DumpAst),
            "print-deps" => Some(Self::PrintDeps),
            "generate" => Some(Self::Generate),
            "dlrl" => Some(Self::Dlrl),
            _ => None,
        }
    }
}

/// Everything the command line said.
#[derive(Debug)]
struct Options {
    command: Command,
    files: Vec<String>,
    form: Form,
    /// `--strict`: OMG IDL 4.2 and nothing else.
    ///
    /// Read through [`Options::dialect`], which every subcommand goes
    /// through — a flag that only one of six honours makes a promise the
    /// other five break.
    strict: bool,
    /// `--ros2`: admit ROS 2 dialect (unqualified member types that differ
    /// only in case from the member name, e.g., `Pose pose;`).
    ros2: bool,
    /// Report warnings as errors, for a build that wants a clean file.
    deny_warnings: bool,
    /// `format --check`: say which files are not canonical, change none.
    format_check: bool,
    /// `format --write`: rewrite the files that are not canonical.
    format_write: bool,
    /// `--default-extensibility`: what an aggregate whose source said
    /// nothing is taken to be.
    ///
    /// `None` leaves the facet absent and XTypes 1.3 §7.3.1.2.1.8's own
    /// default applies. The same paragraph provides for the option — "IDL
    /// compilers may provide configuration options to allow users to
    /// specify whether types of unspecified extensibility are to be
    /// considered final, appendable, or mutable" — and it is what lets a
    /// caller generate for a peer whose compiler reads silence as `@final`.
    /// What the source *did* say still wins.
    default_extensibility: Option<zerodds_ast::Extensibility>,
    /// `-I <DIR>`: where a quoted `#include` is searched after the
    /// including file's own directory, in the order given.
    include_dirs: Vec<PathBuf>,
    /// `generate`'s own flags, filled in exactly when `command` is
    /// [`Command::Generate`]. `parse_args` refuses a `generate` line
    /// without `--lang` before this is ever read, so a `None` here for
    /// `Command::Generate` cannot happen — and where it matters, it is
    /// treated the way `Command::Format` is treated in `run_one` when
    /// matched outside its own early return: trusted, not asserted.
    generate: Option<GenerateRequest>,
    /// `dlrl`'s own flags, filled in exactly when `command` is
    /// [`Command::Dlrl`] — the same non-`None`-unless-matching contract
    /// [`Options::generate`] documents, and for the same reason:
    /// `parse_args` refuses a `dlrl` line without `--lang` before this is
    /// ever read.
    dlrl: Option<DlrlRequest>,
}

impl Options {
    /// The dialect this run reads under.
    ///
    /// `--strict` is OMG IDL 4.2 and nothing else: every vendor
    /// construct becomes an error instead of a warning. Without it every
    /// vendor extension is admitted, because a tool pointed at a file
    /// somebody else wrote has to read it before it judges it.
    fn dialect(&self) -> zerodds_idl_parse::Dialect {
        let mut dialect = if self.strict {
            zerodds_idl_parse::Dialect::STRICT
        } else {
            zerodds_idl_parse::Dialect::ALL_VENDORS
        };
        if self.ros2 {
            dialect.ros2 = true;
        }
        dialect
    }
}

/// What `generate` was asked for.
#[derive(Debug)]
struct GenerateRequest {
    /// The languages to emit: one, or every one for `--lang all`.
    languages: Vec<Language>,
    /// `--stamp FILE`: written once every output of the run is.
    stamp: Option<PathBuf>,
    /// `--depfile FILE`: a Make-format dependency file naming the
    /// target (the stamp, else the manifest) and every IDL file the run
    /// read, `#include`s included.
    depfile: Option<PathBuf>,
    /// `--output-manifest FILE`: JSON with the files read and written.
    manifest: Option<PathBuf>,
    /// Where the file goes. The current directory when `--out` is not
    /// given.
    out_dir: PathBuf,
    /// The encoding the codec is built for. Half of the `LayoutKey`
    /// `generate::source` plans the encoder and the decoder under, so it
    /// reaches the emitted bodies and not only the header: XCDR1 frames
    /// an `@appendable` struct without the `DHEADER` XCDR2 gives it.
    encoding: Encoding,
    /// The byte order the codec is built for — the other half of that
    /// `LayoutKey`. Unlike `encoding` it does not change the emitted
    /// text: every wire runtime holds the byte order in its `Writer` and
    /// `Reader`, so the same generated body writes either order.
    endianness: Endianness,
    /// `--cpp-mapping native|psm|classic`: how the C++ column spells topic types.
    /// `psm` (the default) is DDS-PSM-Cxx 1.0 §7.4 and §8.1 — classes with
    /// accessors, `dds::core::optional`, `std::map`, … — over the same
    /// codec. `native` is the mapping the codec and the endpoint gates are
    /// built on.
    cpp_mapping: zerodds_lang_rules::adapter::cpp::CppMapping,
    /// `--rust-map entries|btree-map`: how the Rust column holds an IDL `map`. `entries` (the
    /// default) is the sequence of `MapEntry` structures XTypes 1.3 §7.5.1.3.6 binds a map to;
    /// `btree-map` is the idiomatic container.
    rust_map: zerodds_lang_rules::adapter::cst::RustMapForm,
    /// `--dialect <name>`: the preset, one peer's answers to every wire
    /// question at once. `None` is the default, `specification`.
    dialect: Option<Dialect>,
    /// `--<axis> <value>` flags, each overriding one answer of the
    /// dialect. Applied after the dialect, in declaration order.
    axis_overrides: Vec<(AxisFlag, String)>,
    /// `--java-mapping <omg|psm>`: the shape Java types take. `psm`, Clause 9
    /// of DDS-Java-PSM 1.0, is the default; `omg` is the IDL-to-Java
    /// mapping. Only `--lang java` reads it.
    java_mapping: zerodds_lang_rules::adapter::java::JavaMapping,
    /// `--rpc-mapping <basic|enhanced|enhanced-empty>`: which of the two
    /// service mappings of DDS-RPC 1.0 §7.5.1 the topic types of an
    /// `interface` are written for. [`ServiceMapping::Basic`] when not
    /// given.
    rpc_mapping: ServiceMapping,
}

/// What `dlrl` was asked for.
///
/// The build-contract fields mirror [`GenerateRequest`]'s exactly, and
/// [`BuildRecord::write`] takes them the same way for both — DLRL is not
/// on the wire, so `dlrl` carries neither `encoding`/`endianness` nor a
/// `dialect`/`axis_overrides`.
#[derive(Debug)]
struct DlrlRequest {
    /// The languages to emit: one, or all four for `--lang all`.
    languages: Vec<Language>,
    /// `--stamp FILE`: written once every output of the run is.
    stamp: Option<PathBuf>,
    /// `--depfile FILE`: as [`GenerateRequest::depfile`].
    depfile: Option<PathBuf>,
    /// `--output-manifest FILE`: as [`GenerateRequest::manifest`].
    manifest: Option<PathBuf>,
    /// Where the file goes. The current directory when `--out` is not
    /// given.
    out_dir: PathBuf,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-idlc {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            print_help();
            return ExitCode::from(2);
        }
    };

    let mut tally = Tally::default();
    let mut read_failed = false;
    let mut record = BuildRecord::default();
    for path in &options.files {
        match Unit::read(path) {
            Ok(unit) => tally.merge(run_one(&options, &unit, &mut record)),
            Err(message) => {
                eprintln!("error: {message}");
                read_failed = true;
            }
        }
    }

    if read_failed {
        return ExitCode::from(3);
    }
    // The build contract: the manifest, the depfile and last the stamp,
    // only for a run that wrote everything it was asked to — a stamp for a
    // failed run would tell the build system the outputs are current.
    if !tally.failed() {
        let write = if let Some(request) = &options.generate {
            let languages: Vec<String> = request
                .languages
                .iter()
                .map(|l| l.as_str().to_string())
                .collect();
            Some(record.write(
                request.stamp.as_deref(),
                request.depfile.as_deref(),
                request.manifest.as_deref(),
                &languages,
            ))
        } else {
            options.dlrl.as_ref().map(|request| {
                let languages: Vec<String> = request
                    .languages
                    .iter()
                    .map(|l| l.as_str().to_string())
                    .collect();
                record.write(
                    request.stamp.as_deref(),
                    request.depfile.as_deref(),
                    request.manifest.as_deref(),
                    &languages,
                )
            })
        };
        if let Some(Err(message)) = write {
            eprintln!("error: {message}");
            tally.errors += 1;
        }
    }
    // A run that produced nothing says so, so an empty output is never
    // mistaken for a clean file that was never read. `format --check`
    // gets the same line: a file counted as an error there is one that
    // is not canonical, or one whose formatting was refused.
    let summarised = matches!(
        options.command,
        Command::Check | Command::Lint | Command::Generate | Command::Dlrl
    ) || (options.command == Command::Format && options.format_check);
    if options.form == Form::Text && summarised {
        println!(
            "{}: {} file(s), {} error(s), {} warning(s)",
            match options.command {
                Command::Lint => "lint",
                Command::Format => "format",
                Command::Generate => "generate",
                Command::Dlrl => "dlrl",
                _ => "check",
            },
            options.files.len(),
            tally.errors,
            tally.warnings
        );
    }
    if tally.failed() || (options.deny_warnings && tally.warnings > 0) {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// Find an axis by its CLI flag name.
fn find_axis_flag(flag: &str) -> Option<AxisFlag> {
    AXES.into_iter().find(|axis| axis.flag_name() == flag)
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut iter = args.iter();
    let first = iter.next().ok_or("a subcommand is required")?;
    let command = Command::parse(first)
        .ok_or_else(|| format!("unknown subcommand `{first}` — see --help"))?;

    let mut options = Options {
        command,
        files: Vec::new(),
        form: Form::Text,
        strict: false,
        ros2: false,
        deny_warnings: false,
        format_check: false,
        format_write: false,
        default_extensibility: None,
        include_dirs: Vec::new(),
        generate: None,
        dlrl: None,
    };
    // `generate`'s own flags accumulate here rather than straight into
    // `options.generate`, because a `GenerateRequest` needs a language
    // and nothing before the end of the line guarantees `--lang` was
    // seen: `--out foo --lang rust a.idl` and `--lang rust --out foo
    // a.idl` are both valid, in either order.
    let mut generate_lang: Option<Vec<Language>> = None;
    let mut generate_stamp: Option<PathBuf> = None;
    let mut generate_depfile: Option<PathBuf> = None;
    let mut generate_manifest: Option<PathBuf> = None;
    let mut generate_out = PathBuf::from(".");
    let mut generate_encoding = Encoding::PlainCdr2;
    let mut generate_endianness = Endianness::Little;
    let mut generate_dialect: Option<Dialect> = None;
    let mut generate_rpc_mapping = ServiceMapping::Basic;
    let mut generate_cpp_mapping = zerodds_lang_rules::adapter::cpp::CppMapping::default();
    let mut generate_rust_map = zerodds_lang_rules::adapter::cst::RustMapForm::Entries;
    let mut generate_axis_overrides: Vec<(AxisFlag, String)> = Vec::new();
    let mut generate_java_mapping: Option<zerodds_lang_rules::adapter::java::JavaMapping> = None;
    // `dlrl`'s own flags, the same reason `generate`'s accumulate outside
    // `options` first.
    let mut dlrl_lang: Option<Vec<Language>> = None;
    let mut dlrl_stamp: Option<PathBuf> = None;
    let mut dlrl_depfile: Option<PathBuf> = None;
    let mut dlrl_manifest: Option<PathBuf> = None;
    let mut dlrl_out = PathBuf::from(".");

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => options.form = Form::Json,
            "--strict" => options.strict = true,
            "--ros2" => options.ros2 = true,
            "--deny-warnings" => options.deny_warnings = true,
            "--default-extensibility" => {
                let value = iter
                    .next()
                    .ok_or("--default-extensibility: the option takes a value")?;
                options.default_extensibility = Some(match value.to_ascii_lowercase().as_str() {
                    "final" => zerodds_ast::Extensibility::Final,
                    "appendable" => zerodds_ast::Extensibility::Appendable,
                    "mutable" => zerodds_ast::Extensibility::Mutable,
                    _ => {
                        return Err(format!(
                            "--default-extensibility: `{value}` is none of final, appendable, mutable"
                        ));
                    }
                });
            }
            "--check" if command == Command::Format => options.format_check = true,
            "--write" if command == Command::Format => options.format_write = true,
            "-I" | "--include" => {
                let dir = iter.next().ok_or("-I: the option takes a directory")?;
                options.include_dirs.push(PathBuf::from(dir));
            }
            "--lang" if command == Command::Generate => {
                let name = iter.next().ok_or("--lang: the option takes a value")?;
                generate_lang = Some(if name == "all" {
                    Language::ALL.to_vec()
                } else {
                    vec![generate::parse_language(name).ok_or_else(|| {
                        format!(
                            "--lang: `{name}` is not a language `generate` knows — one of \
                             {}, or all",
                            generate::language_names()
                        )
                    })?]
                });
            }
            "--lang" if command == Command::Dlrl => {
                let name = iter.next().ok_or("--lang: the option takes a value")?;
                dlrl_lang = Some(if name == "all" {
                    dlrl::LANGUAGES.to_vec()
                } else {
                    vec![dlrl::parse_language(name).ok_or_else(|| {
                        format!(
                            "--lang: `{name}` is not a language `dlrl` knows — one of {}, or \
                             all",
                            dlrl::language_names()
                        )
                    })?]
                });
            }
            name @ ("--stamp" | "--depfile" | "--output-manifest")
                if command == Command::Generate =>
            {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{name}: the option takes a file"))?;
                let slot = match name {
                    "--stamp" => &mut generate_stamp,
                    "--depfile" => &mut generate_depfile,
                    _ => &mut generate_manifest,
                };
                *slot = Some(PathBuf::from(value));
            }
            name @ ("--stamp" | "--depfile" | "--output-manifest") if command == Command::Dlrl => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{name}: the option takes a file"))?;
                let slot = match name {
                    "--stamp" => &mut dlrl_stamp,
                    "--depfile" => &mut dlrl_depfile,
                    _ => &mut dlrl_manifest,
                };
                *slot = Some(PathBuf::from(value));
            }
            "--out" if command == Command::Generate => {
                let value = iter.next().ok_or("--out: the option takes a value")?;
                generate_out = PathBuf::from(value);
            }
            "--out" if command == Command::Dlrl => {
                let value = iter.next().ok_or("--out: the option takes a value")?;
                dlrl_out = PathBuf::from(value);
            }
            "--encoding" if command == Command::Generate => {
                let value = iter.next().ok_or("--encoding: the option takes a value")?;
                generate_encoding = match value.as_str() {
                    "xcdr1" | "plain-cdr1" => Encoding::PlainCdr1,
                    "xcdr2" | "plain-cdr2" => Encoding::PlainCdr2,
                    "pl-cdr1" => Encoding::PlCdr1,
                    "pl-cdr2" => Encoding::PlCdr2,
                    "delimited-cdr" => Encoding::DelimitedCdr,
                    other => {
                        return Err(format!(
                            "--encoding: `{other}` is not xcdr1, xcdr2, pl-cdr1, pl-cdr2 or \
                             delimited-cdr"
                        ));
                    }
                };
            }
            // `--xcdr-endianness` is the spelling DDS-TS 1.0 §C.3.1
            // names — "the selected encapsulation kind SHALL be
            // configured via the codegen flag `--xcdr-endianness=BE|LE`,
            // defaulting to LE" — and it takes the same two values in
            // either case. One arm and not two: a second parser for one
            // option is how two spellings come to mean different things.
            name @ ("--endian" | "--xcdr-endianness") if command == Command::Generate => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{name}: the option takes a value"))?;
                generate_endianness = match value.to_ascii_lowercase().as_str() {
                    "le" => Endianness::Little,
                    "be" => Endianness::Big,
                    _ => return Err(format!("{name}: `{value}` is not le or be")),
                };
            }
            "--java-mapping" if command == Command::Generate => {
                let value = iter
                    .next()
                    .ok_or("--java-mapping: the option takes a value")?;
                generate_java_mapping = Some(
                    zerodds_lang_rules::adapter::java::JavaMapping::from_name(value).ok_or_else(
                        || format!("--java-mapping: `{value}` is neither omg nor psm"),
                    )?,
                );
            }
            // DDS-RPC 1.0 §7.2.4 names two mappings and no default; Basic
            // needs no extension of DDS and is the one written when the flag
            // is absent. `enhanced-empty` is the Enhanced mapping with an
            // operation that has nothing to carry as an `@Empty` structure
            // (§7.5.1.2.1.3) in place of the `dummy` member.
            "--rpc-mapping" if command == Command::Generate => {
                let value = iter
                    .next()
                    .ok_or("--rpc-mapping: the option takes a value")?;
                generate_rpc_mapping = match value.as_str() {
                    "basic" => ServiceMapping::Basic,
                    "enhanced" => ServiceMapping::Enhanced,
                    "enhanced-empty" => ServiceMapping::EnhancedEmptyStructures,
                    other => {
                        return Err(format!(
                            "--rpc-mapping: `{other}` is not basic, enhanced or enhanced-empty"
                        ));
                    }
                };
            }
            "--cpp-mapping" if command == Command::Generate => {
                let value = iter
                    .next()
                    .ok_or("--cpp-mapping: the option takes a value")?;
                generate_cpp_mapping = match value.as_str() {
                    "native" => zerodds_lang_rules::adapter::cpp::CppMapping::Native,
                    "psm" => zerodds_lang_rules::adapter::cpp::CppMapping::Psm,
                    "classic" => zerodds_lang_rules::adapter::cpp::CppMapping::Classic,
                    other => {
                        return Err(format!(
                            "--cpp-mapping: `{other}` is not native, psm or classic"
                        ));
                    }
                };
            }
            "--rust-map" if command == Command::Generate => {
                let value = iter.next().ok_or("--rust-map: the option takes a value")?;
                generate_rust_map = match value.as_str() {
                    "entries" => zerodds_lang_rules::adapter::cst::RustMapForm::Entries,
                    "btree-map" => zerodds_lang_rules::adapter::cst::RustMapForm::BTreeMap,
                    other => {
                        return Err(format!("--rust-map: `{other}` is not entries or btree-map"));
                    }
                };
            }
            "--dialect" if command == Command::Generate => {
                let name = iter.next().ok_or("--dialect: the option takes a value")?;
                generate_dialect = Some(Dialect::parse(name).ok_or_else(|| {
                    format!(
                        "--dialect: `{name}` is not a dialect — one of {}",
                        dialect_names()
                    )
                })?);
            }
            // These two are unconditional and placed ahead of the axis-flag
            // arm below on purpose: under `generate`, `--check`/`--write`
            // are not axis flags either, and without this ordering the
            // broader `name if …` guard below would claim them first and
            // report "unknown option" instead of naming the subcommand
            // they belong to.
            other @ ("--check" | "--write") => {
                return Err(format!("`{other}` belongs to the `format` subcommand"));
            }
            // `--lang`, `--out`, `--stamp`, `--depfile` and
            // `--output-manifest` are the five `dlrl` shares with
            // `generate`; the other four are wire-only and stay
            // `generate`'s alone.
            other @ ("--lang" | "--out" | "--stamp" | "--depfile" | "--output-manifest") => {
                return Err(format!(
                    "`{other}` belongs to the `generate` or `dlrl` subcommand"
                ));
            }
            other @ ("--encoding" | "--endian" | "--xcdr-endianness" | "--dialect"
            | "--rpc-mapping" | "--cpp-mapping" | "--java-mapping" | "--rust-map") => {
                return Err(format!("`{other}` belongs to the `generate` subcommand"));
            }
            // Axis flags under `generate`.  Only `--`-prefixed names are
            // checked; a bare word that is not a known flag is a file path.
            // A name that is not an axis flag falls through to "unknown
            // option", the same answer the non-generate catch-all below
            // gives a `-`-prefixed name nothing else claimed.
            name if command == Command::Generate && name.starts_with('-') => {
                let Some(axis) = find_axis_flag(name) else {
                    return Err(format!("unknown option `{name}`"));
                };
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{name}: the option takes a value"))?;
                // Validated against a throwaway default-dialect settings
                // value: parsing a flag's own value never depends on which
                // dialect or which earlier flags came before it, so this
                // is purely "is `value` one this axis recognises at all" —
                // the same question `run_generate` asks again, for real,
                // once the dialect is known.
                let settings = DialectSettings::in_dialect(
                    Dialect::default(),
                    generate_encoding,
                    generate_endianness,
                    zerodds_ast_layout::Purpose::Data,
                );
                let _ = axis
                    .parse_and_apply(settings, value)
                    .map_err(|msg| format!("{name}: {msg}"))?;
                generate_axis_overrides.push((axis, value.clone()));
            }
            other if other.starts_with('-') => {
                // Axis flags in non-generate context: reject with a
                // useful message if the flag would be valid under generate.
                if find_axis_flag(other).is_some() {
                    return Err(format!("`{other}` belongs to the `generate` subcommand"));
                }
                return Err(format!("unknown option `{other}`"));
            }
            path => options.files.push(path.to_string()),
        }
    }
    if options.format_check && options.format_write {
        return Err(String::from(
            "`--check` and `--write` contradict each other: one promises not to touch the \
             files, the other exists to",
        ));
    }
    if command == Command::Generate {
        let languages = generate_lang.ok_or_else(|| {
            format!(
                "generate needs --lang: one of {}, or all",
                generate::language_names()
            )
        })?;
        if generate_depfile.is_some() && generate_stamp.is_none() && generate_manifest.is_none() {
            return Err(String::from(
                "--depfile names the target it describes: give --stamp or --output-manifest",
            ));
        }
        if generate_java_mapping.is_some() && !languages.contains(&Language::Java) {
            return Err(String::from(
                "--java-mapping applies to --lang java (or all); no Java output was asked for",
            ));
        }
        options.generate = Some(GenerateRequest {
            languages,
            stamp: generate_stamp,
            depfile: generate_depfile,
            manifest: generate_manifest,
            out_dir: generate_out,
            encoding: generate_encoding,
            endianness: generate_endianness,
            dialect: generate_dialect,
            cpp_mapping: generate_cpp_mapping,
            rust_map: generate_rust_map,
            axis_overrides: generate_axis_overrides,
            java_mapping: generate_java_mapping.unwrap_or_default(),
            rpc_mapping: generate_rpc_mapping,
        });
    }
    if command == Command::Dlrl {
        let languages = dlrl_lang.ok_or_else(|| {
            format!(
                "dlrl needs --lang: one of {}, or all",
                dlrl::language_names()
            )
        })?;
        if dlrl_depfile.is_some() && dlrl_stamp.is_none() && dlrl_manifest.is_none() {
            return Err(String::from(
                "--depfile names the target it describes: give --stamp or --output-manifest",
            ));
        }
        options.dlrl = Some(DlrlRequest {
            languages,
            stamp: dlrl_stamp,
            depfile: dlrl_depfile,
            manifest: dlrl_manifest,
            out_dir: dlrl_out,
        });
    }
    if options.files.is_empty() {
        return Err(String::from("at least one file is required"));
    }
    Ok(options)
}

/// Parse and lower one unit, and hand both stages' diagnostics back with
/// the model.
///
/// Lowering runs even when parsing reported errors: the tree is always
/// there, a bad range becomes an error node, and the rules that hold over
/// the good part of a file are worth reporting in the same run rather
/// than in the next one after a fix.
fn analyse(
    options: &Options,
    unit: &Unit,
) -> (
    zerodds_ast::Ast,
    Vec<zerodds_idl_parse::Diagnostic>,
    Vec<String>,
) {
    let loader = zerodds_idl_compose::PathLoader::new(options.include_dirs.clone());
    let lowering = if let Some(ref generate) = options.generate {
        let d = generate.dialect.unwrap_or_default();
        zerodds_ast_layout::DialectLowering::for_dialect(d)
    } else {
        zerodds_ast_layout::DialectLowering::for_dialect(zerodds_ast_layout::Dialect::default())
    };
    let composed = zerodds_idl_compose::compose_source_with(
        &unit.path,
        &unit.text,
        &loader,
        zerodds_idl_compose::ComposeOptions {
            dialect: options.dialect(),
            // `--default-extensibility` wins when given; absent that,
            // `--dialect`'s own answer — not `None`, which is where a
            // dialect whose default reads `@final` (Cyclone, the three
            // ORBs, both ROS columns) would otherwise leave every
            // unannotated aggregate composed as the specification's
            // `@appendable` while `zerodds_lang_rules::generate::source`
            // plans and frames it `@final` from the same `LayoutKey`
            // `run_generate` builds below. The two disagreeing is not
            // cosmetic: the codec's `DHEADER` follows the plan (correct,
            // `@final`), and `Ast::effective_extensibility` — what the
            // generated `DdsType::EXTENSIBILITY` constant and the
            // evolution-tolerant decode pattern both read — would still
            // answer `@appendable`, so a decoder built from this file
            // would default a truncated member instead of refusing a
            // stream `@final` gives no tolerance for. `Some(dialect)`
            // only when `--dialect` was actually given, so a run with
            // neither flag stays `None` — the composer's own default,
            // unchanged.
            default_extensibility: options.default_extensibility.or_else(|| {
                options
                    .generate
                    .as_ref()
                    .and_then(|generate| generate.dialect)
                    .map(zerodds_ast_layout::Dialect::default_extensibility)
            }),
            hashed_id_advances_the_counter: if lowering.hashed_id_advances_the_counter {
                zerodds_idl_lower::HashedIdCounter::Advances
            } else {
                zerodds_idl_lower::HashedIdCounter::Skipped
            },
            hashed_name_spelling: if lowering.hashid_reads_the_escaped_spelling {
                zerodds_idl_lower::HashedNameSpelling::AsWritten
            } else {
                zerodds_idl_lower::HashedNameSpelling::Stripped
            },
            name_of_the_enclosing_scope: if lowering.admits_name_of_enclosing_scope {
                zerodds_idl_lower::NameOfTheEnclosingScope::Admitted
            } else {
                zerodds_idl_lower::NameOfTheEnclosingScope::Refused
            },
            ..zerodds_idl_compose::ComposeOptions::default()
        },
    );
    (composed.ast, composed.diagnostics, loader.opened())
}

/// What a `generate` run read and wrote, for its build-contract files.
#[derive(Debug, Default)]
struct BuildRecord {
    /// Every IDL file read: each unit and each file it `#include`d.
    inputs: Vec<String>,
    /// Every file written.
    outputs: Vec<String>,
}

impl BuildRecord {
    /// Write the build-contract files `generate` and `dlrl` share:
    /// the manifest, the depfile and last the stamp.
    ///
    /// Generic over what asked for them — a [`GenerateRequest`] and a
    /// [`DlrlRequest`] have the same three optional paths and both name
    /// the languages they emitted, and neither carries anything else
    /// this needs.
    fn write(
        &self,
        stamp: Option<&std::path::Path>,
        depfile: Option<&std::path::Path>,
        manifest: Option<&std::path::Path>,
        languages: &[String],
    ) -> Result<(), String> {
        let write = |path: &std::path::Path, text: &str| -> Result<(), String> {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("{}: {error}", parent.display()))?;
            }
            std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
        };
        if let Some(path) = manifest {
            write(path, &self.manifest_json(languages))?;
        }
        if let Some(path) = depfile {
            // The target is what the build system asks about: the stamp, or
            // the manifest when there is no stamp (`parse_args` requires one).
            let target = stamp
                .or(manifest)
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let mut text = make_escape(&target);
            text.push(':');
            for input in &self.inputs {
                text.push_str(" \\\n  ");
                text.push_str(&make_escape(input));
            }
            text.push('\n');
            write(path, &text)?;
        }
        if let Some(path) = stamp {
            write(path, "")?;
        }
        Ok(())
    }

    fn manifest_json(&self, languages: &[String]) -> String {
        let list = |items: &[String]| {
            items
                .iter()
                .map(|i| json_string(i))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "{{\n  \"languages\": [{}],\n  \"inputs\": [{}],\n  \"outputs\": [{}]\n}}\n",
            list(languages),
            list(&self.inputs),
            list(&self.outputs)
        )
    }
}

/// A path as a Make rule spells it: spaces, `#` and `$` escaped.
fn make_escape(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            ' ' | '#' => {
                out.push('\\');
                out.push(c);
            }
            '$' => out.push_str("$$"),
            _ => out.push(c),
        }
    }
    out
}

/// A JSON string literal.
fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn run_one(options: &Options, unit: &Unit, record: &mut BuildRecord) -> Tally {
    // `format` reads the token stream, not the model: the model has no
    // comments and no directives, and the formatter refuses to print a
    // form without them.
    if options.command == Command::Format {
        return run_format(options, unit);
    }
    let (mut ast, diagnostics, opened) = analyse(options, unit);
    if matches!(options.command, Command::Generate | Command::Dlrl) {
        for path in core::iter::once(unit.path.clone()).chain(opened.clone()) {
            if !record.inputs.contains(&path) {
                record.inputs.push(path);
            }
        }
    }
    match options.command {
        Command::Check => report::emit(unit, &diagnostics, options.form),
        Command::Lint => {
            // The lint rules are about a file that already parses, so a
            // unit with parse errors reports those and stops: linting a
            // tree with error nodes in it produces findings about the
            // recovery and not about the source.
            let mut tally = report::emit(unit, &diagnostics, options.form);
            if !tally.failed() {
                // The rules read node spans, which the composer has put in
                // the files they were written in.
                let findings = lint::run(&ast);
                tally.merge(report::emit(unit, &findings, options.form));
            }
            tally
        }
        Command::DumpAst => {
            let tally = report::emit(unit, &diagnostics, options.form);
            print!("{}", dump::tree(&ast));
            tally
        }
        Command::PrintDeps => {
            let tally = report::emit(unit, &diagnostics, options.form);
            print!("{}", dump::dependencies(&ast));
            tally
        }
        Command::Generate => run_generate(options, unit, &mut ast, &diagnostics, record),
        Command::Dlrl => run_dlrl(options, unit, &ast, &diagnostics, &opened, record),
        // Handled before `analyse`; `run_one` never reaches here with it.
        Command::Format => Tally::default(),
    }
}

/// Write `ast` out as `options.generate`'s language, beside its
/// diagnostics.
///
/// A unit with parse or lowering errors reports those and stops, the
/// same way `Command::Lint` does: a model with error nodes in it has
/// nothing a materialiser should be asked to render.
fn run_generate(
    options: &Options,
    unit: &Unit,
    ast: &mut zerodds_ast::Ast,
    diagnostics: &[zerodds_idl_parse::Diagnostic],
    record: &mut BuildRecord,
) -> Tally {
    let mut tally = report::emit(unit, diagnostics, options.form);
    if tally.failed() {
        return tally;
    }
    // See the doc on `Options::generate`: `parse_args` never builds a
    // `Command::Generate` without this, so a run that reaches here
    // always has it. A `None` is left alone rather than asserted away.
    let Some(request) = &options.generate else {
        return tally;
    };

    // DDS-XTypes 1.3 §7.3.1.1.1: a construct outside the building blocks
    // is ignored or warned about, "rather than halting with an error". A
    // declaration that holds `any`, `ValueBase`, `native` or an object
    // reference has no type identifier and no wire form, so it is left
    // out of the generated code and the file's other declarations are
    // generated.
    let left_out = match ast.drop_outside_profile(zerodds_ast::Profile::XTypes13) {
        Ok(left_out) => left_out,
        Err(error) => {
            eprintln!("error: {}: {error}", unit.path);
            tally.errors += 1;
            return tally;
        }
    };
    let ast = &*ast;
    if !left_out.is_empty() {
        let warnings: Vec<zerodds_idl_parse::Diagnostic> = left_out
            .iter()
            .map(|dropped| {
                zerodds_idl_parse::Diagnostic::warning(
                    zerodds_idl_parse::DiagnosticCode::Unsupported,
                    zerodds_idl_cst::Span::new(0, 0),
                    format!(
                        "{} reaches `{}`, for which DDS-XTypes 1.3 has no type ({}); no code is \
                         generated for it",
                        dropped.name, dropped.construct, dropped.detail
                    ),
                    dropped.spec_reference,
                )
            })
            .collect();
        tally.merge(report::emit(unit, &warnings, options.form));
    }

    let dialect = request.dialect.unwrap_or_default();
    let overrides: Vec<(AxisFlag, &str)> = request
        .axis_overrides
        .iter()
        .map(|(a, v)| (*a, v.as_str()))
        .collect();
    let settings = match dialect_settings_from_overrides(
        dialect,
        request.encoding,
        request.endianness,
        zerodds_ast_layout::Purpose::Data,
        &overrides,
    ) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("error: {}: {error}", unit.path);
            tally.errors += 1;
            return tally;
        }
    };
    let key = settings.key;
    let codec_options = zerodds_lang_rules::emit::encode::CodecOptions {
        map_length_with_delimiter: settings.map_length_with_delimiter,
    };

    let stem = std::path::Path::new(&unit.path)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("generated");
    // Build a preamble string naming the dialect and each axis flag.
    let dialect_info = if dialect.is_default() && request.axis_overrides.is_empty() {
        None
    } else {
        let mut info = if dialect.is_default() {
            String::new()
        } else {
            format!("dialect {}", dialect.as_str())
        };
        for (axis, value) in &request.axis_overrides {
            if info.is_empty() {
                info = format!("{} {}", axis.flag_name(), value);
            } else {
                info.push_str(&format!(", {} {}", axis.flag_name(), value));
            }
        }
        Some(info)
    };
    let mut generated = Vec::new();
    // The mapping is a scope on this thread: it decides names in every function that spells a
    // C++ type path, and `generate::files` is one of them.
    let _mapping = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(request.cpp_mapping);
    let _map_form = zerodds_lang_rules::adapter::cst::RustMapFormScope::enter(request.rust_map);
    for language in &request.languages {
        // The Java mapping is in force for this one call: the
        // materialiser, the codec and the typed companion all read it.
        let produced =
            zerodds_lang_rules::adapter::java::with_mapping(request.java_mapping, || {
                generate::files_with_mapping(
                    ast,
                    *language,
                    &unit.path,
                    stem,
                    key,
                    codec_options,
                    dialect_info.as_deref(),
                    request.rpc_mapping,
                )
            });
        match produced {
            Ok(files) => generated.extend(files),
            Err(message) => {
                eprintln!("error: {}: {} — {message}", unit.path, language.as_str());
                tally.errors += 1;
                return tally;
            }
        }
    }

    write_generated_files(
        options,
        unit,
        &generated,
        &request.out_dir,
        record,
        &mut tally,
    );
    tally
}

/// Write every one of `generated` under `out_dir`, the way `run_generate`
/// and `run_dlrl` both do: refuse a name a prior input in this run already
/// wrote, make the parent directory, write the file, and say so in text
/// form. Shared rather than duplicated, because the refusal is the one
/// piece of this either subcommand cannot afford to get out of step with
/// the other on — a language that writes a fixed file name (Java's
/// `ZdTypes.java`, or `dlrl`'s own `<Type>.java`) silently overwriting a
/// prior input's output is the same defect under either subcommand.
fn write_generated_files(
    options: &Options,
    unit: &Unit,
    generated: &[generate::Generated],
    out_dir: &std::path::Path,
    record: &mut BuildRecord,
    tally: &mut Tally,
) {
    for file in generated {
        let out = out_dir.join(&file.name);
        // A language that writes fixed file names (Java's ZdTypes.java,
        // Ada's zd_types.ad[sb]) gives two inputs the same path; writing the
        // second over the first dropped the first file's types without a
        // word. Refuse instead.
        let shown = out.display().to_string();
        if record.outputs.contains(&shown) {
            eprintln!(
                "error: {}: {shown} was already written for an earlier input in this run; \
                 give each IDL file its own --out, or #include them into one file",
                unit.path
            );
            tally.errors += 1;
            return;
        }
        if let Some(parent) = out.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                eprintln!("error: {}: {error}", parent.display());
                tally.errors += 1;
                return;
            }
        }
        if let Err(error) = std::fs::write(&out, &file.text) {
            eprintln!("error: {}: {error}", out.display());
            tally.errors += 1;
            return;
        }
        if options.form == Form::Text {
            println!("{}: wrote {}", unit.path, out.display());
        }
        record.outputs.push(out.display().to_string());
    }
}

/// Write DDS 1.2 §8 DLRL object and home classes for `unit`'s pragmas.
///
/// `diagnostics` is `analyse`'s — parse and lowering — reported first and
/// combined with `unit`'s own [`dlrl::scan`], the same "report, then stop
/// if it failed" shape `run_generate` follows: a file that does not parse
/// is not one `zerodds_dlrl::pragma::parse_pragma` should be asked to
/// scan either, since a byte offset from a broken file is not one a
/// person can trust to point where it claims to. `opened` — the files
/// `unit` `#include`d — are scanned too, each against its own text: a
/// DLRL pragma from an `#include`d file reported against `unit.text`
/// would point at the wrong line in the wrong file entirely.
fn run_dlrl(
    options: &Options,
    unit: &Unit,
    ast: &zerodds_ast::Ast,
    diagnostics: &[zerodds_idl_parse::Diagnostic],
    opened: &[String],
    record: &mut BuildRecord,
) -> Tally {
    let (mut pragmas, scan_diagnostics) = dlrl::scan(&unit.text);
    let mut combined = diagnostics.to_vec();
    combined.extend(scan_diagnostics);
    let mut tally = report::emit(unit, &combined, options.form);

    for path in opened {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let (found, file_diagnostics) = dlrl::scan(&text);
                pragmas.extend(found);
                if !file_diagnostics.is_empty() {
                    let included = Unit {
                        path: path.clone(),
                        text,
                    };
                    tally.merge(report::emit(&included, &file_diagnostics, options.form));
                }
            }
            Err(error) => {
                eprintln!("error: {path}: {error}");
                tally.errors += 1;
            }
        }
    }
    if tally.failed() {
        return tally;
    }
    // See the doc on `Options::dlrl`: `parse_args` never builds a
    // `Command::Dlrl` without this, so a run that reaches here always has
    // it. A `None` is left alone rather than asserted away, the same
    // standing `run_generate` gives `options.generate`.
    let Some(request) = &options.dlrl else {
        return tally;
    };

    let mut infos = zerodds_dlrl_codegen::collect_type_infos(&pragmas);
    let key_errors: Vec<zerodds_idl_parse::Diagnostic> =
        dlrl::resolve_member_types(ast, &mut infos)
            .into_iter()
            .map(|message| {
                zerodds_idl_parse::Diagnostic::error(
                    zerodds_idl_parse::DiagnosticCode::Unsupported,
                    zerodds_idl_cst::Span::new(0, 0),
                    message,
                    "OMG DDS 1.2 §8.2.2",
                )
            })
            .collect();
    if !key_errors.is_empty() {
        tally.merge(report::emit(unit, &key_errors, options.form));
        return tally;
    }
    if infos.is_empty() {
        // A run that writes nothing says so, the same reason the summary
        // line at the very end of `main` counts files rather than
        // trusting silence: an author who meant to mark a type and
        // mistyped the pragma tag would otherwise see "0 errors" and a
        // clean exit for a run that generated nothing at all.
        let warning = zerodds_idl_parse::Diagnostic::warning(
            zerodds_idl_parse::DiagnosticCode::Unsupported,
            zerodds_idl_cst::Span::new(0, 0),
            "no `#pragma DCPS_DATA_TYPE`, `DCPS_DATA_KEY` or `DCPS_DLRL_RELATION` found; \
             `dlrl` writes no file for it"
                .to_string(),
            "OMG DDS 1.2 §8.2.2",
        );
        tally.merge(report::emit(unit, &[warning], options.form));
        return tally;
    }

    let stem = std::path::Path::new(&unit.path)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("generated");
    let mut generated = Vec::new();
    for language in &request.languages {
        generated.extend(dlrl::files(&infos, *language, &unit.path, stem));
    }
    write_generated_files(
        options,
        unit,
        &generated,
        &request.out_dir,
        record,
        &mut tally,
    );
    tally
}

/// Format one unit, in whichever of the three modes was asked for.
///
/// The default prints the canonical form to stdout and changes nothing.
/// `--check` names each file that is not canonical and counts it as an
/// error, which is what makes the exit code say so. `--write` rewrites
/// exactly the files that are not canonical. A refusal — a file that
/// does not parse, or one whose comments the canonical form could not
/// carry — is reported like every other diagnostic and no output is
/// produced for that file in any mode.
/// Write `unit` out in canonical form, or say why not.
///
/// The refusal runs the whole pipeline, not just the parser. `format`
/// used to refuse only a file that did not *parse*, so a file with a
/// duplicate case label, a zero array bound, an unresolvable type or —
/// under `--strict` — a vendor pragma was rewritten without a word,
/// while the same run's `check` rejected it. Handing the user a
/// formatted file their tool discards on the next step is worse than
/// refusing both: 21 of the 356 probes of `tools/idl-4.2-gap-scan` were
/// in exactly that state, and 12 of the tree's 748 `.idl` files.
///
/// Nothing is printed on the success path, because the formatted text
/// goes to stdout and a warning printed ahead of it would end up in the
/// file a caller redirected it into.
fn run_format(options: &Options, unit: &Unit) -> Tally {
    let (_, diagnostics, _) = analyse(options, unit);
    let failed = diagnostics
        .iter()
        .any(|d| d.severity == zerodds_idl_parse::Severity::Error);
    if failed {
        return report::emit(unit, &diagnostics, options.form);
    }
    // `analyse` read the file after the preprocessor, under the same
    // dialect, and found no error: that is the validity the layout needs.
    match format::canonical_of_checked(&unit.text) {
        Err(diagnostics) => report::emit(unit, &diagnostics, options.form),
        Ok(formatted) => {
            let mut tally = Tally::default();
            let changed = formatted != unit.text;
            if options.format_check {
                if changed {
                    println!("{}: not in canonical form", unit.path);
                    tally.errors += 1;
                }
            } else if options.format_write {
                if changed {
                    if let Err(error) = std::fs::write(&unit.path, &formatted) {
                        eprintln!("error: {}: {error}", unit.path);
                        tally.errors += 1;
                    }
                }
            } else {
                print!("{formatted}");
            }
            tally
        }
    }
}

fn print_help() {
    println!(
        "zerodds-idlc — read OMG IDL 4.2 and say what is in it\n\
         \n\
         USAGE:\n    \
             zerodds-idlc <SUBCOMMAND> [OPTIONS] FILE...\n\
         \n\
         SUBCOMMANDS:\n    \
             check         parse and lower; report every diagnostic\n    \
             lint          the rules a valid file can still break\n    \
             format        the canonical form, to stdout by default\n    \
             dump-ast      the model, as a tree\n    \
             print-deps    the types each declaration names\n    \
             generate      one language's source for the model\n    \
             dlrl          DDS 1.2 §8 DLRL object and home classes, from the three DLRL \
             pragmas\n\
         \n\
OPTIONS:\n    \
              --strict           OMG IDL 4.2 only; reject every vendor extension\n    \
              --ros2             admit ROS 2 dialect (unqualified member types like `Pose pose;`)\n    \
              --json             machine-readable diagnostics\n    \
             --deny-warnings    exit non-zero when anything was reported\n    \
             --default-extensibility <final|appendable|mutable>\n                       what an aggregate whose source said nothing is taken to be (default: the specification's appendable). A source that did say still wins\n    \
             --check            format only: name each non-canonical file, exit 1\n    \
             --write            format only: rewrite non-canonical files in place\n    \
             --lang <NAME>      generate/dlrl only, required: the language to emit, or `all` \
             — an unknown name lists the valid ones (generate: 21 wire languages; dlrl: cpp, \
             csharp, java, typescript)\n    \
             --out <DIR>        generate/dlrl only: where the file goes (default: the \
             current directory)\n    \
             -I, --include <DIR>  where a quoted #include is searched after the including \
             file's directory; repeatable\n    \
             --stamp <FILE>     generate/dlrl only: written last, and only when the run \
             succeeded\n    \
             --depfile <FILE>   generate/dlrl only: Make dependency file (target: the stamp, \
             else the manifest) listing every IDL file read\n    \
             --output-manifest <FILE>  generate/dlrl only: JSON of the files read and \
             written\n    \
             --encoding <ENC>   generate only: xcdr1, xcdr2 (default), pl-cdr1, pl-cdr2, \
             delimited-cdr\n    \
             --endian <le|be>   generate only: the byte order the codec is planned for. It \
             does not change the emitted text — every wire runtime holds the order in its \
             writer and reader — but the preamble names it (default: le)\n    \
             --xcdr-endianness <BE|LE>  generate only: the DDS-TS 1.0 spelling of --endian\n    \
             --java-mapping <omg|psm>  generate --lang java only: the shape of the Java types. \
             psm (default) is Clause 9 of DDS-Java-PSM 1.0, final classes with private state and bean \
             accessors, and java.util.Collection<E> for an unbounded sequence; \
             omg is the IDL-to-Java mapping, public fields and T[] for every sequence\n    \
             --rpc-mapping <M>  generate only: the service mapping of DDS-RPC 1.0 §7.5.1 an \
             `interface` is written for — basic (default), enhanced, or enhanced-empty (the \
             Enhanced mapping with `@Empty` structures for an operation with nothing to carry)\n    \
             --cpp-mapping <native|psm|classic>  generate only: how --lang c++ spells topic \
             types — psm (default; DDS-PSM-Cxx 1.0 §7.4 and §8.1: classes with accessors, \
             dds::core::optional, std::map, …, over the same codec), native (public \
             fields, the types the codec reads and writes; what the wire codec gates and the \
             endpoint SDKs ask for) or classic (the OMG IDL to C++ 1.3 mapping in C++03, \
             over the same codec)\n    \
             --rust-map <entries|btree-map>  generate --lang rust only: how an IDL map is \
             held — entries (default; the MapEntry sequence of XTypes 1.3 §7.5.1.3.6) or \
             btree-map (a BTreeMap, entries in ascending key order)\n    \
             --dialect <NAME>   generate only: reproduce one peer's answers to every wire \
             axis at once, before any axis flag overrides one of them (default: specification, \
             which is what generate always emitted) — an unknown name lists the valid ones\n    \
             --<axis> <VALUE>   generate only: override one answer of the dialect, applied \
             after --dialect in the order given — an unknown value lists the valid ones for \
             that axis; docs/wire-axis-flags.md has every axis's flag and values\n    \
             -h, --help         this text\n    \
             -V, --version      the version\n\
         \n\
         EXIT CODES:\n    \
             0  nothing to report\n    \
             1  a diagnostic was reported, or --check found a file to reformat\n    \
             2  the command line was wrong\n    \
             3  a file could not be read\n"
    );
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    fn options(args: &[&str]) -> Result<Options, String> {
        parse_args(&args.iter().map(|a| (*a).to_string()).collect::<Vec<_>>())
    }

    /// crates.io shows the package description, and it read "check, lint,
    /// format and inspect OMG IDL 4.2 without generating anything" while
    /// `generate` was a subcommand the tool parses and `--help` lists.
    /// Nothing failed from it: the sentence was simply wrong on the
    /// registry page, where no test looked.
    #[test]
    fn the_package_description_denies_no_subcommand_the_tool_parses() {
        let description = env!("CARGO_PKG_DESCRIPTION").to_ascii_lowercase();
        for name in [
            "check",
            "lint",
            "format",
            "dump-ast",
            "print-deps",
            "generate",
            "dlrl",
        ] {
            assert!(
                Command::parse(name).is_some(),
                "`{name}` is no longer a subcommand — this list has drifted \
                 away from `Command::parse`"
            );
            // The wrong description had the shape "without generating"; the
            // stem without a trailing `e` covers the -ing and -es forms.
            let stem = name.trim_end_matches('e');
            assert!(
                !description.contains(&format!("without {stem}")),
                "the description denies `{name}`, which the tool parses: \
                 {description}"
            );
        }
    }

    /// DDS-RPC 1.0 §7.2.4 names two service mappings and no default; Basic
    /// is the one written when `--rpc-mapping` is absent.
    #[test]
    fn the_rpc_mapping_is_basic_unless_named() {
        let got = options(&["generate", "--lang", "rust", "a.idl"]).expect("valid");
        assert_eq!(
            got.generate.as_ref().unwrap().rpc_mapping,
            ServiceMapping::Basic
        );
        for (name, want) in [
            ("basic", ServiceMapping::Basic),
            ("enhanced", ServiceMapping::Enhanced),
            ("enhanced-empty", ServiceMapping::EnhancedEmptyStructures),
        ] {
            let got = options(&["generate", "--lang", "rust", "--rpc-mapping", name, "a.idl"])
                .expect("valid");
            assert_eq!(got.generate.as_ref().unwrap().rpc_mapping, want, "{name}");
        }
    }

    #[test]
    fn an_unknown_rpc_mapping_and_a_misplaced_flag_are_refused() {
        let err = options(&[
            "generate",
            "--lang",
            "rust",
            "--rpc-mapping",
            "full",
            "a.idl",
        ])
        .expect_err("`full` is no mapping");
        assert!(err.contains("full"), "{err}");
        assert!(options(&["generate", "--rpc-mapping"]).is_err());
        assert!(options(&["check", "--rpc-mapping", "basic", "a.idl"]).is_err());
    }

    #[test]
    fn a_subcommand_and_a_file_are_the_minimum() {
        let got = options(&["check", "a.idl"]).expect("valid");
        assert_eq!(got.command, Command::Check);
        assert_eq!(got.files, vec![String::from("a.idl")]);
        assert_eq!(got.form, Form::Text);

        options(&["check"]).expect_err("a file is required");
        options(&[]).expect_err("a subcommand is required");
    }

    /// An unknown subcommand and an unknown option are both named back,
    /// because a tool that says "usage:" and nothing else makes the
    /// caller guess which of their words it did not know.
    #[test]
    fn what_was_not_understood_is_named() {
        let error = options(&["transpile", "a.idl"]).expect_err("not a subcommand");
        assert!(error.contains("transpile"), "{error}");
        let error = options(&["check", "--colour", "a.idl"]).expect_err("unknown option");
        assert!(error.contains("--colour"), "{error}");
    }

    #[test]
    fn flags_and_files_may_be_interleaved() {
        let got = options(&["lint", "a.idl", "--strict", "b.idl", "--json"]).expect("valid");
        assert_eq!(got.command, Command::Lint);
        assert_eq!(
            got.files,
            vec![String::from("a.idl"), String::from("b.idl")]
        );
        assert!(got.strict);
        assert_eq!(got.form, Form::Json);
    }

    #[test]
    fn every_subcommand_parses_by_its_written_name() {
        for (token, want) in [
            ("check", Command::Check),
            ("lint", Command::Lint),
            ("format", Command::Format),
            ("dump-ast", Command::DumpAst),
            ("print-deps", Command::PrintDeps),
            ("generate", Command::Generate),
            ("dlrl", Command::Dlrl),
        ] {
            assert_eq!(Command::parse(token), Some(want), "{token}");
        }
        assert_eq!(Command::parse("transpile"), None);
    }

    /// `--check` and `--write` belong to `format`: elsewhere they are
    /// named back, together they are refused, alone they are read.
    #[test]
    fn the_format_flags_are_format_only_and_exclusive() {
        let got = options(&["format", "--check", "a.idl"]).expect("valid");
        assert!(got.format_check && !got.format_write);
        let got = options(&["format", "--write", "a.idl"]).expect("valid");
        assert!(got.format_write && !got.format_check);

        let error = options(&["lint", "--check", "a.idl"]).expect_err("format only");
        assert!(
            error.contains("--check") && error.contains("format"),
            "{error}"
        );
        let error = options(&["format", "--check", "--write", "a.idl"]).expect_err("exclusive");
        assert!(
            error.contains("--check") && error.contains("--write"),
            "{error}"
        );
    }

    #[test]
    fn generate_needs_lang_and_names_the_valid_ones_when_missing() {
        let error = options(&["generate", "a.idl"]).expect_err("--lang is required");
        assert!(error.contains("--lang"), "{error}");
        assert!(
            error.contains("rust") && error.contains("kotlin"),
            "{error}"
        );
    }

    /// The same naming `what_was_not_understood_is_named` checks for a
    /// subcommand and an option: an unknown `--lang` value says which
    /// ones would have worked.
    #[test]
    fn an_unknown_language_names_the_valid_ones() {
        let error = options(&["generate", "--lang", "cobol", "a.idl"]).expect_err("unknown");
        assert!(error.contains("cobol"), "{error}");
        assert!(error.contains("rust") && error.contains("nim"), "{error}");
    }

    #[test]
    fn generate_defaults_to_xcdr2_little_endian_and_the_current_directory() {
        let got = options(&["generate", "--lang", "rust", "a.idl"]).expect("valid");
        let request = got.generate.expect("a generate request");
        assert_eq!(request.languages, vec![Language::Rust]);
        assert_eq!(request.out_dir, PathBuf::from("."));
        assert_eq!(request.encoding, Encoding::PlainCdr2);
        assert_eq!(request.endianness, Endianness::Little);
    }

    /// `--java-mapping` selects the shape of the Java types: Clause 9 of
    /// DDS-Java-PSM 1.0 by default, the IDL-to-Java mapping on request,
    /// and nothing else.
    #[test]
    fn the_java_mapping_defaults_to_psm_and_omg_is_selectable() {
        use zerodds_lang_rules::adapter::java::JavaMapping;
        let got = options(&["generate", "--lang", "java", "a.idl"]).expect("valid");
        assert_eq!(
            got.generate.expect("a generate request").java_mapping,
            JavaMapping::Psm
        );
        let got = options(&[
            "generate",
            "--lang",
            "java",
            "--java-mapping",
            "psm",
            "a.idl",
        ])
        .expect("valid");
        assert_eq!(
            got.generate.expect("a generate request").java_mapping,
            JavaMapping::Psm
        );
        let got = options(&[
            "generate",
            "--lang",
            "java",
            "--java-mapping",
            "omg",
            "a.idl",
        ])
        .expect("valid");
        assert_eq!(
            got.generate.expect("a generate request").java_mapping,
            JavaMapping::Omg
        );
        let got = options(&[
            "generate",
            "--lang",
            "all",
            "--java-mapping",
            "omg",
            "a.idl",
        ])
        .expect("valid");
        assert_eq!(
            got.generate.expect("a generate request").java_mapping,
            JavaMapping::Omg
        );
        let error = options(&[
            "generate",
            "--lang",
            "java",
            "--java-mapping",
            "jacorb",
            "a.idl",
        ])
        .expect_err("unknown mapping");
        assert!(error.contains("jacorb") && error.contains("psm"), "{error}");
        let error = options(&[
            "generate",
            "--lang",
            "rust",
            "--java-mapping",
            "psm",
            "a.idl",
        ])
        .expect_err("no Java output asked for");
        assert!(error.contains("--lang java"), "{error}");
        let error =
            options(&["check", "--java-mapping", "psm", "a.idl"]).expect_err("generate only");
        assert!(error.contains("generate"), "{error}");
    }

    #[test]
    fn generate_reads_out_encoding_and_endian() {
        let got = options(&[
            "generate",
            "--lang",
            "nim",
            "--out",
            "build",
            "--encoding",
            "xcdr1",
            "--endian",
            "be",
            "a.idl",
        ])
        .expect("valid");
        let request = got.generate.expect("a generate request");
        assert_eq!(request.languages, vec![Language::Nim]);
        assert_eq!(request.out_dir, PathBuf::from("build"));
        assert_eq!(request.encoding, Encoding::PlainCdr1);
        assert_eq!(request.endianness, Endianness::Big);
    }

    /// DDS-TS 1.0 §C.3.1 names the flag `--xcdr-endianness=BE|LE` and
    /// gives it the default LE. It is the same option under a second
    /// spelling, in either case, and the default is what a `generate`
    /// line that names neither already gets.
    #[test]
    fn generate_takes_the_dds_ts_spelling_of_the_endianness_flag() {
        let big = options(&[
            "generate",
            "--lang",
            "typescript",
            "--xcdr-endianness",
            "BE",
            "a.idl",
        ])
        .expect("valid")
        .generate
        .expect("a generate request");
        assert_eq!(big.endianness, Endianness::Big);

        let little = options(&[
            "generate",
            "--lang",
            "typescript",
            "--xcdr-endianness",
            "LE",
            "a.idl",
        ])
        .expect("valid")
        .generate
        .expect("a generate request");
        assert_eq!(little.endianness, Endianness::Little);

        let unnamed = options(&["generate", "--lang", "typescript", "a.idl"])
            .expect("valid")
            .generate
            .expect("a generate request");
        assert_eq!(unnamed.endianness, Endianness::Little);

        assert!(
            options(&[
                "generate",
                "--lang",
                "typescript",
                "--xcdr-endianness",
                "middle",
                "a.idl"
            ])
            .is_err(),
            "a third byte order was accepted"
        );
    }

    #[test]
    fn generate_rejects_a_bad_encoding_or_endian() {
        let error = options(&[
            "generate",
            "--lang",
            "rust",
            "--encoding",
            "ebcdic",
            "a.idl",
        ])
        .expect_err("not an encoding");
        assert!(
            error.contains("--encoding") && error.contains("ebcdic"),
            "{error}"
        );

        let error = options(&["generate", "--lang", "rust", "--endian", "middle", "a.idl"])
            .expect_err("not le or be");
        assert!(
            error.contains("--endian") && error.contains("middle"),
            "{error}"
        );
    }

    /// `--lang`, `--out`, `--encoding` and `--endian` belong to
    /// `generate` the way `--check`/`--write` belong to `format`.
    #[test]
    fn the_generate_flags_are_generate_only() {
        let error = options(&["check", "--lang", "rust", "a.idl"]).expect_err("generate only");
        assert!(
            error.contains("--lang") && error.contains("generate"),
            "{error}"
        );
        let error = options(&["lint", "--out", "build", "a.idl"]).expect_err("generate only");
        assert!(
            error.contains("--out") && error.contains("generate"),
            "{error}"
        );
    }

    /// `--encoding`, `--endian`, `--xcdr-endianness` and `--dialect` are
    /// wire questions `dlrl` has none of, so they stay `generate`'s
    /// alone even though `--lang`/`--out`/`--stamp`/`--depfile`/
    /// `--output-manifest` are now shared with `dlrl`.
    #[test]
    fn the_wire_only_generate_flags_do_not_mention_dlrl() {
        for (flag, value) in [
            ("--encoding", "xcdr1"),
            ("--endian", "le"),
            ("--xcdr-endianness", "LE"),
            ("--dialect", "specification"),
        ] {
            let error = options(&["dlrl", flag, value, "a.idl"]).expect_err("generate-only flag");
            assert!(
                error.contains(flag) && error.contains("generate") && !error.contains("dlrl"),
                "{error}"
            );
        }
    }

    #[test]
    fn dlrl_needs_lang_and_names_the_four_languages_when_missing() {
        let error = options(&["dlrl", "a.idl"]).expect_err("--lang is required");
        assert!(error.contains("--lang"), "{error}");
        assert!(
            error.contains("c++") && error.contains("typescript"),
            "{error}"
        );
        assert!(!error.contains("kotlin"), "{error}");
    }

    #[test]
    fn dlrl_lang_takes_the_same_aliases_generate_does() {
        for (alias, want) in [
            ("cpp", Language::Cpp),
            ("cxx", Language::Cpp),
            ("csharp", Language::CSharp),
            ("cs", Language::CSharp),
            ("java", Language::Java),
            ("typescript", Language::TypeScript),
            ("ts", Language::TypeScript),
        ] {
            let got = options(&["dlrl", "--lang", alias, "a.idl"]).expect("valid");
            let request = got.dlrl.expect("a dlrl request");
            assert_eq!(request.languages, vec![want], "{alias}");
        }
    }

    #[test]
    fn dlrl_lang_all_is_the_four_dlrl_languages_not_every_language() {
        let got = options(&["dlrl", "--lang", "all", "a.idl"]).expect("valid");
        let request = got.dlrl.expect("a dlrl request");
        assert_eq!(request.languages, dlrl::LANGUAGES.to_vec());
        assert!(!request.languages.contains(&Language::Rust));
    }

    /// `dlrl --lang rust` is a `generate` question — the CLI names the
    /// four valid languages, not the twenty-one `generate` takes.
    #[test]
    fn dlrl_rejects_a_wire_only_language() {
        let error = options(&["dlrl", "--lang", "rust", "a.idl"]).expect_err("not a dlrl target");
        assert!(error.contains("rust"), "{error}");
        assert!(error.contains("c++") && error.contains("java"), "{error}");
    }

    #[test]
    fn dlrl_defaults_to_the_current_directory() {
        let got = options(&["dlrl", "--lang", "cpp", "a.idl"]).expect("valid");
        let request = got.dlrl.expect("a dlrl request");
        assert_eq!(request.out_dir, PathBuf::from("."));
        assert!(request.stamp.is_none());
        assert!(request.depfile.is_none());
        assert!(request.manifest.is_none());
    }

    #[test]
    fn dlrl_reads_out_stamp_depfile_and_manifest() {
        let got = options(&[
            "dlrl",
            "--lang",
            "java",
            "--out",
            "build",
            "--stamp",
            "a.stamp",
            "--depfile",
            "a.d",
            "--output-manifest",
            "a.json",
            "a.idl",
        ])
        .expect("valid");
        let request = got.dlrl.expect("a dlrl request");
        assert_eq!(request.out_dir, PathBuf::from("build"));
        assert_eq!(request.stamp, Some(PathBuf::from("a.stamp")));
        assert_eq!(request.depfile, Some(PathBuf::from("a.d")));
        assert_eq!(request.manifest, Some(PathBuf::from("a.json")));
    }

    /// A real IDL file, generated end to end: composed, materialised and
    /// written, with the extension and the content its language
    /// promises.
    #[test]
    fn generate_writes_a_real_idl_file_as_lua() {
        let dir = std::env::temp_dir().join(format!(
            "zerodds-idlc-generate-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let idl_path = dir.join("robot.idl");
        std::fs::write(&idl_path, "struct Pose { double x; double y; };\n")
            .expect("writing the fixture IDL");

        let options = options(&[
            "generate",
            "--lang",
            "lua",
            "--out",
            dir.to_str().expect("a UTF-8 path"),
            idl_path.to_str().expect("a UTF-8 path"),
        ])
        .expect("a valid command line");
        let unit = Unit::read(idl_path.to_str().expect("a UTF-8 path")).expect("reading it back");
        let tally = run_one(&options, &unit, &mut BuildRecord::default());
        assert!(!tally.failed(), "{tally:?}");

        let generated = std::fs::read_to_string(dir.join("robot.lua"))
            .expect("`generate` writes `<stem>.<extension>` in `--out`");
        assert!(
            generated.contains("Generated by `idlc generate`"),
            "{generated}"
        );
        assert!(generated.contains("Pose"), "{generated}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `generate` writes one language's file for `idl` under `encoding`,
    /// and answers what it wrote.
    ///
    /// The whole command line, not `generate::source` directly: what the
    /// two tests below are about is that a flag a person types reaches
    /// the emitted body, and the flag is only a flag once `parse_args`
    /// has read it.
    fn generated_with(label: &str, language: &str, encoding: &str, idl: &str) -> String {
        let dir = std::env::temp_dir().join(format!(
            "zerodds-idlc-codec-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let idl_path = dir.join("sample.idl");
        std::fs::write(&idl_path, idl).expect("writing the fixture IDL");
        let path = idl_path.to_str().expect("a UTF-8 path");

        let options = options(&[
            "generate",
            "--lang",
            language,
            "--encoding",
            encoding,
            "--out",
            dir.to_str().expect("a UTF-8 path"),
            path,
        ])
        .expect("a valid command line");
        let unit = Unit::read(path).expect("reading it back");
        let tally = run_one(&options, &unit, &mut BuildRecord::default());
        assert!(!tally.failed(), "{language}/{encoding}: {tally:?}");

        let text = written(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        text
    }

    /// Everything `generate` wrote into `dir`, in name order, joined.
    ///
    /// Not "the one file": Java is two — `ZdTypes.java` and
    /// `ZdCodec.java` — because a Java file holds one `public` top-level
    /// type and the materialiser emits two. A helper that took the first
    /// entry the directory happened to yield would ask its question of
    /// whichever half the filesystem named first.
    fn written(dir: &std::path::Path) -> String {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .expect("the output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.file_name().is_some_and(|name| name != "sample.idl"))
            .collect();
        assert!(!paths.is_empty(), "`generate` writes a file");
        paths.sort();
        paths
            .iter()
            .map(|path| std::fs::read_to_string(path).expect("a generated file"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Java is written as three compilation units, and each holds one
    /// `public` top-level type.
    ///
    /// The materialiser emits `public final class ZdTypes`,
    /// `public class ZdCodec` and — since the standard TypeInformation
    /// companion — `public final class ZdTypeSupports`, and §7.6 of the
    /// language specification allows one `public` top-level type per
    /// compilation unit. As one file that is `class ZdCodec is public,
    /// should be declared in a file named ZdCodec.java` — `javac`
    /// refuses it whatever the file is called, so the Java column
    /// produced no compilable output at all. `wireprobe-zerodds-java-gen`
    /// and `crates/endpoint-e2e/tests/java.rs` both cut the same text at
    /// the same point by hand.
    #[test]
    fn java_is_written_as_three_compilation_units() {
        const IDL: &str = "struct Point { long x; long y; };\n";
        let dir = std::env::temp_dir().join(format!(
            "zerodds-idlc-java-split-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let idl_path = dir.join("sample.idl");
        std::fs::write(&idl_path, IDL).expect("writing the fixture IDL");
        let path = idl_path.to_str().expect("a UTF-8 path");
        let options = options(&[
            "generate",
            "--lang",
            "java",
            "--out",
            dir.to_str().expect("a UTF-8 path"),
            path,
        ])
        .expect("a valid command line");
        let unit = Unit::read(path).expect("reading it back");
        assert!(!run_one(&options, &unit, &mut BuildRecord::default()).failed());

        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .expect("the output directory")
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name != "sample.idl")
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec!["ZdCodec.java", "ZdTypeSupports.java", "ZdTypes.java"]
        );

        for name in &names {
            let text = std::fs::read_to_string(dir.join(name)).expect("a generated file");
            assert_eq!(
                text.matches("public class ").count() + text.matches("public final class ").count(),
                1,
                "{name} holds more than one public top-level type:\n{text}"
            );
        }
        // The runtime import goes with the half that names the runtime,
        // and the `ZdEndian` holder with it: a package declares a type
        // once, so it cannot stand in both.
        let types = std::fs::read_to_string(dir.join("ZdTypes.java")).expect("ZdTypes.java");
        let codec = std::fs::read_to_string(dir.join("ZdCodec.java")).expect("ZdCodec.java");
        assert!(!types.contains("import org.zerodds.wire"), "{types}");
        assert!(!types.contains("ZdEndian"), "{types}");
        assert!(codec.contains("import org.zerodds.wire.*;"), "{codec}");
        assert!(codec.contains("class ZdEndian"), "{codec}");
        // A codec nothing outside the class can call is not one: the
        // methods were `private static`, which a probe that inlines them
        // never noticed and a generated application cannot use.
        assert!(codec.contains("public static void zd_enc_"), "{codec}");
        assert!(!codec.contains("private static void zd_enc_"), "{codec}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `generate` writes `idl` in `language` into a scratch directory
    /// and answers the directory, for a test that has to look at the
    /// files themselves rather than at their text joined.
    fn generated_into(label: &str, language: &str, idl: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zerodds-idlc-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let idl_path = dir.join("sample.idl");
        std::fs::write(&idl_path, idl).expect("writing the fixture IDL");
        let path = idl_path.to_str().expect("a UTF-8 path");
        let options = options(&[
            "generate",
            "--lang",
            language,
            "--out",
            dir.to_str().expect("a UTF-8 path"),
            path,
        ])
        .expect("a valid command line");
        let unit = Unit::read(path).expect("reading it back");
        assert!(
            !run_one(&options, &unit, &mut BuildRecord::default()).failed(),
            "{language}"
        );
        dir
    }

    /// Every subprogram `body` holds a body for, by name.
    ///
    /// The Ada spelling is `procedure P (…) is` / `function F (…) return
    /// T is`, one per line at the package body's own indentation, so
    /// the name is the second word of a line that opens one.
    fn ada_body_names(body: &str) -> Vec<String> {
        body.lines()
            .filter_map(|line| {
                let text = line.trim_start();
                let rest = text
                    .strip_prefix("procedure ")
                    .or_else(|| text.strip_prefix("function "))?;
                let name = rest.split_whitespace().next()?;
                Some(name.split('(').next().unwrap_or(name).to_string())
            })
            .collect()
    }

    /// One Ada column is written as a specification and a body, and
    /// every subprogram the body defines is declared in the
    /// specification.
    ///
    /// Both halves of one defect. RM 7.1 admits only
    /// `basic_declarative_item`s in the visible part of a package, and
    /// a `subprogram_body` is not one — as one file the codec's bodies
    /// stood where the declarations are and `gnatmake` answered
    /// `max.ads:9:01: error: compilation unit expected`, measured, so
    /// the column produced no compilable output at all. And a body
    /// whose profile is in no specification is not declared by the
    /// package: it compiles, and no caller outside the unit can name
    /// it — the Ada form of the `private static` the Java column was
    /// written as.
    fn ada_column_is_a_specification_and_a_body(
        language: &str,
        label: &str,
        package: &str,
        spec_name: &str,
        body_name: &str,
    ) {
        // A map, a sequence, an array, an @optional member and a nested
        // aggregate: the codec emits an encoder, a decoder and a Table 9
        // defaulter for each, which is what makes the profile question
        // measurable at more than one subprogram.
        const IDL: &str = "@appendable struct Inner { long a; double b; };\n@mutable struct \
                           Point { @key long x; string name; sequence<long> ids; map<long, \
                           string> table; Inner nested; @optional octet extra; };\n";
        let dir = generated_into(label, language, IDL);

        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .expect("the output directory")
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name != "sample.idl")
            .collect();
        names.sort();
        assert_eq!(names, vec![body_name.to_string(), spec_name.to_string()]);

        let spec = std::fs::read_to_string(dir.join(spec_name)).expect("the specification");
        let body = std::fs::read_to_string(dir.join(body_name)).expect("the body");

        // The specification is a compilation unit: a context clause,
        // then one package, and nothing outside it.
        assert!(spec.contains(&format!("package {package} is")), "{spec}");
        assert!(spec.contains(&format!("end {package};")), "{spec}");
        assert!(spec.contains("with Interfaces;"), "{spec}");
        // `begin` opens the statement sequence of a body, and no body
        // may stand in the visible part.
        assert!(!spec.contains("begin"), "{spec}");

        assert!(
            body.contains(&format!("package body {package} is")),
            "{body}"
        );
        assert!(body.contains(&format!("end {package};")), "{body}");
        assert!(body.contains("begin"), "{body}");

        // Every subprogram the body defines is declared in the
        // specification, so a caller outside the unit can name it. The
        // decoders and the Table 9 defaulters are the half that went
        // missing: only the encoders carried a profile.
        let declared = ada_body_names(&spec);
        let defined = ada_body_names(&body);
        assert!(
            defined.iter().any(|name| name.starts_with("zd_dec_")),
            "the body defines no decoder:\n{body}"
        );
        let hidden: Vec<&String> = defined
            .iter()
            .filter(|name| !declared.contains(name))
            .collect();
        assert!(
            hidden.is_empty(),
            "{language}: {hidden:?} have a body and no declaration, so nothing outside the \
             package can call them"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ada_is_written_as_a_specification_and_a_body() {
        ada_column_is_a_specification_and_a_body(
            "ada",
            "ada-units",
            "Zd_Types",
            "zd_types.ads",
            "zd_types.adb",
        );
    }

    #[test]
    fn ada83_is_written_as_a_specification_and_a_body() {
        ada_column_is_a_specification_and_a_body(
            "ada83",
            "ada83-units",
            "Zd83_Types",
            "zd83_types.ads",
            "zd83_types.adb",
        );
    }

    /// The F# file opens on `module rec`, so a declaration may name one
    /// written below it.
    ///
    /// F# resolves names in file order and the materialiser writes the
    /// model's order, so a `map`'s entry class is declared after the
    /// aggregate whose member names it: `maponly.fs(7,20): error
    /// FS0039: the type ZdMapEntry_long_string is not defined`,
    /// measured with `dotnet fsi` (SDK 9.0.106). `module rec` is what
    /// `zerodds_lang_cst::fsharp` documents and
    /// `zerodds_lang_render::fsharp::file` writes for every other
    /// consumer; this tool assembled the clause itself and left the
    /// word out.
    #[test]
    fn fsharp_is_written_as_a_recursive_module() {
        const IDL: &str = "struct WithMap { map<long, string> t; };\n";
        let text = generated_with("fsharp-rec", "f#", "xcdr2", IDL);
        let clause = text
            .lines()
            .nth(1)
            .expect("a module clause under the header");
        assert_eq!(clause, "module rec ZdGen", "{text}");
        // The declaration the clause is there for: it is named above
        // and declared below, which is what a plain `module` refuses.
        assert!(text.contains("type ZdMapEntry_long_string()"), "{text}");
    }

    /// Every module the generated TypeScript imports from is a package
    /// this tree publishes, and the codec it declares is exported.
    ///
    /// Two halves of one file that no consumer could use. The
    /// descriptor import named `@zerodds/types`, a package that exists
    /// nowhere — `tsc` answered `error TS2307: Cannot find module
    /// '@zerodds/types' or its corresponding type declarations`, while
    /// the wire import in the same file already named the right one.
    /// And every codec function was module-level and unexported:
    /// `error TS2459: Module './max.js' declares 'zd_enc_Max' locally,
    /// but it is not exported`, which is the TypeScript form of the
    /// `private static` the Java column carried.
    ///
    /// The package name is **read out of `runtime/typescript`'s own
    /// `package.json`** and not spelled here: which package this tree
    /// publishes the runtime as is that file's answer, and a test
    /// holding a copy of it would pass while the two disagreed.
    #[test]
    fn typescript_imports_a_package_that_exists_and_exports_its_codec() {
        const IDL: &str = "struct Point { long x; long y; };\n";
        let text = generated_with("typescript-imports", "typescript", "xcdr2", IDL);

        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../runtime/typescript/package.json");
        let json = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|error| panic!("{}: {error}", manifest.display()));
        let published = json
            .split("\"name\"")
            .nth(1)
            .and_then(|rest| rest.split('"').nth(1))
            .expect("runtime/typescript/package.json names the package")
            .to_string();

        let mut named = 0;
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("import ") else {
                continue;
            };
            let module = rest
                .rsplit_once("from \"")
                .and_then(|(_, tail)| tail.split('"').next())
                .unwrap_or_else(|| panic!("an import with no module: {line}"));
            assert_eq!(
                module, published,
                "the generated file imports `{module}`, and the tree publishes `{published}`"
            );
            named += 1;
        }
        assert!(named >= 2, "no imports in the generated file:\n{text}");

        assert!(text.contains("export function zd_enc_Point"), "{text}");
        assert!(!text.contains("\nfunction zd_enc_Point"), "{text}");
        assert!(text.contains("export function zd_dec_Point"), "{text}");
    }

    /// The declarations were never the whole file: every language
    /// `--lang` takes emits a codec beside them.
    ///
    /// For a while none did. `generate` called
    /// `zerodds_lang_rules::generate::declarations`, which answers the
    /// type and nothing else, so `--lang go` wrote `type T_Point struct`
    /// over an application that then had no way to put it on a wire —
    /// and reported "0 errors" doing it. The encoder every backend but
    /// Rust's names through `emit::encode::codec_name` is `zd_enc_`;
    /// Rust's `encode.rs` names a method instead.
    ///
    /// Go's is `Zd_enc_`. Go has no visibility keyword — an identifier
    /// is exported if and only if its first character is upper case (Go
    /// specification, "Exported identifiers") — so a codec a Go program
    /// outside the generated package can call has to be spelled that
    /// way, and `emit::go::exported_name` raises the first letter and
    /// only that. Spelled out here rather than asked of that function,
    /// for the reason `runtime_mention` below gives: a test that asked
    /// the same function the code asks would pass whatever it answered.
    #[test]
    fn every_language_emits_a_codec_beside_its_declarations() {
        const IDL: &str = "module T { struct Point { long x; long y; }; };\n";
        for language in generate::language_names().split(", ") {
            let text = generated_with("all", language, "xcdr2", IDL);
            let has_codec = match language {
                "rust" => text.contains("fn encode") && text.contains("fn decode"),
                "go" => text.contains("Zd_enc_") && text.contains("Zd_dec_"),
                _ => text.contains("zd_enc_") && text.contains("zd_dec_"),
            };
            assert!(has_codec, "{language}: no codec in the output:\n{text}");
        }
    }

    /// The runtime name each language's preamble is written under, and
    /// which its codec always mentions.
    ///
    /// The mirror of `generate::runtime_symbol`, spelled out here so a
    /// change there has to be made twice on purpose. A test that asked
    /// the same function the code asks would pass whatever that
    /// function answered.
    fn runtime_mention(language: &str) -> &'static str {
        match language {
            "rust" | "java" | "kotlin" | "python" | "typescript" => "Writer",
            "julia" | "swift" => "ZdWriter",
            "c" => "zdw_writer",
            "c++" => "zerodds",
            "c#" => "Wire",
            "elixir" => "ZeroddsWire",
            "f#" => "ZdWire",
            "ocaml" => "Zdwire",
            "ada" => "Zd_Wire",
            "ada83" => "Zd_Wire83",
            "lua" => "zdw",
            _ => "zdwire",
        }
    }

    /// `text` without its first line.
    ///
    /// Line one is the header comment, and it carries the path the model
    /// was read from — which is a scratch directory whose name has
    /// `zerodds` in it. A test looking for `zerodds` in the output would
    /// find it there and pass over an empty file. The generator itself
    /// never reads the header: `generate::preamble` is handed the
    /// materialised body alone.
    fn below_the_header(text: &str) -> String {
        text.lines().skip(1).collect::<Vec<_>>().join("\n")
    }

    /// A model with a codec gets its language's runtime preamble.
    ///
    /// The generated file has to compile, and none of the twenty-one
    /// resolves the runtime on its own: Go's own output was eight
    /// `undefined: zdwire`, measured, before this. The import is written
    /// once per file, ahead of the declarations — for Python after them,
    /// because a `__future__` statement has to come first.
    #[test]
    fn a_model_with_a_codec_gets_its_runtime_preamble() {
        const IDL: &str = "struct Point { long x; long y; };\n";
        for language in generate::language_names().split(", ") {
            let text = below_the_header(&generated_with("with", language, "xcdr2", IDL));
            let mention = runtime_mention(language);
            assert!(
                text.contains(mention),
                "{language}: the codec does not mention {mention}:\n{text}"
            );
            // Ada and Ada 83 take a context clause and no constant: see
            // `generate::preamble`. Every other language names the byte
            // order it was generated for.
            let expected = match language {
                "ada" => "with Zd_Wire;",
                "ada83" => "with Zd_Wire83;",
                "rust" => "use zerodds_wire::",
                "c" => "#include \"zerodds_wire.h\"",
                "c++" => "#include \"zerodds/wire.hpp\"",
                "java" => "import org.zerodds.wire.*;",
                "kotlin" => "import zerodds.wire.*",
                "python" => "from zerodds_wire import ",
                "typescript" => "from \"@zerodds/wire\"",
                "go" => "import \"zdwire\"",
                "d" | "nim" => "import zdwire",
                "node" => "require(\"./zdwire.js\")",
                "zig" => "@import(\"zdwire.zig\")",
                "swift" => "import ZdWire",
                "julia" => "using .ZeroddsWire",
                "lua" => "require(\"zerodds_wire\")",
                "c#" => "ZeroDds.Wire.Endianness",
                "elixir" => "defmodule ZdEndian",
                "ocaml" => "let zd_endianness = Zdwire.",
                "f#" => "let zdEndianness = ZdWire.",
                other => panic!("{other} has no expected preamble in this test"),
            };
            assert!(
                text.contains(expected),
                "{language}: no `{expected}` in the output:\n{text}"
            );
        }
    }

    /// A model with no codec gets no runtime preamble.
    ///
    /// The other direction, and the one that costs something to get
    /// wrong: Go refuses to compile a file with an import nothing uses,
    /// and Rust's `unused_imports` is an error under the `-D warnings` a
    /// consumer's crate hands its dependencies. An empty module declares
    /// no type a codec is built for, so no backend names its runtime and
    /// no import is written — which is what makes the preamble follow
    /// the model rather than the flag.
    #[test]
    fn a_model_without_a_codec_gets_no_runtime_preamble() {
        const IDL: &str = "module Empty {\n};\n";
        for language in generate::language_names().split(", ") {
            let text = below_the_header(&generated_with("without", language, "xcdr2", IDL));
            assert!(
                !text.contains(runtime_mention(language)),
                "{language}: a model with no codec still names its runtime:\n{text}"
            );
        }
    }

    /// `--encoding` reaches the emitted body, and is not spent on the
    /// header comment.
    ///
    /// XCDR1 has no `DHEADER` and XCDR2's `@appendable` default does, so
    /// the same struct is two different encoders under the two flags.
    /// The flag reached only the header line for as long as `generate`
    /// called `declarations`: both files were byte-identical below it,
    /// and a person who passed `--encoding xcdr1` got XCDR2 code with an
    /// XCDR1 comment on top.
    #[test]
    fn the_encoding_flag_changes_the_emitted_codec() {
        const IDL: &str = "module T { struct Point { long x; long y; }; };\n";
        for language in generate::language_names().split(", ") {
            // Rust's column refuses nothing in this model, but it is the
            // one backend whose codec is not the shared walk, so it is
            // checked by the same difference rather than skipped.
            let one = generated_with("x1", language, "xcdr1", IDL);
            let two = generated_with("x2", language, "xcdr2", IDL);
            let body = |text: &str| text.lines().skip(1).collect::<Vec<_>>().join("\n");
            assert_ne!(
                body(&one),
                body(&two),
                "{language}: xcdr1 and xcdr2 emit the same body"
            );
        }
    }
}
