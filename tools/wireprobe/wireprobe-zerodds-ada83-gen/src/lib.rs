// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-ada83-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-ada83-gen` — the Ada 83 program the Ada 83 probe
//! builds and runs. `wireprobe-zerodds-ada-gen` is the template this
//! crate follows for the type walk and the codec; what is different
//! here is entirely the harness, because RM83 admits none of what the
//! Ada 2012 harness is built from:
//!
//! - **No `Ada.Command_Line`.** There is no `Set_Exit_Status` and no
//!   reading `argv` at all. Every path this harness needs — the IDL
//!   source, the sample, the bytes read or written, the JSON written
//!   back — is a literal baked into the generated source at generation
//!   time, exactly as `--in` and `--values-out` already are in the Ada
//!   2012 column; the one addition here is `--bytes-out` for the write
//!   direction, because there is nothing this harness can stream to
//!   standard output. Setting a real process exit status without
//!   `Ada.Command_Line` is `pragma Import (C, …, "exit")` — a GNAT
//!   extension available regardless of `-gnat83`, and the only route
//!   this harness found; unverified on a host with `gnatmake`, since
//!   none is on the machine that built this column.
//! - **No `Ada.Streams.Stream_IO`, no `Ada.Text_IO.Text_Streams`.**
//!   Reading and writing the raw octets is RM83 14.2.2's
//!   `Sequential_IO`, generic over `Zd_Wire83.Byte`; the JSON the read
//!   direction writes is plain `Text_IO`, which is not a child unit at
//!   all in Ada 83.
//! - **No `Ada.Exceptions`.** There is no `Exception_Message` to read
//!   back, and RM83 11.3 gives `raise` no string to carry one in the
//!   first place. The handler below therefore names
//!   `Zd_Wire83`'s exceptions one by one rather than formatting a
//!   caught one's text.
//!
//! Nothing here writes a byte or decides a wire question, exactly as in
//! the Ada 2012 generator: the declarations come from
//! [`zerodds_lang_rules::adapter::ada83`], the codec from the shared
//! walk through [`zerodds_lang_rules::emit::ada83::Ada83`], and the
//! octets come out of `runtime/ada-83` when the generated program runs.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

mod dump;
mod fill;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::default_ada83::ada83_defaulters_called_by;
use zerodds_lang_rules::emit::ada83::Ada83;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

const USAGE: &str = "\
wireprobe-zerodds-ada83-gen — the Ada 83 the ZeroDDS Ada 83 probe builds and runs

    --idl <file>            the IDL source (required)
    --type <ScopedName>     the top-level type (required)
    --values <file>         the sample, as JSON (required, unless --in)
    --in <file>             bytes to read back instead of writing
    --values-out <file>     where the values read go (required with --in)
    --bytes-out <file>      where the written octets go (required, unless --in) —
                            this column has no Ada.Command_Line and nothing to
                            stream stdout through, so the path is baked in
    --encoding <name>       xcdr1 | xcdr2 | plain-cdr1 | pl-cdr1 |
                            plain-cdr2 | delimited-cdr | pl-cdr2
    --endian le|be
    --emit <form>           payload | payload+encapsulation
    --dialect <name>        specification (the default) | cyclone | rti |
                            fastdds | opendds | ros-fastrtps |
                            ros-cyclone | omniorb | tao | jacorb
    --out-dir <dir>         where zd83_probe.adb goes (required)

The defaults are `wireprobe-zerodds`'s, because an Ada 83 column that
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
    /// Where the written octets go. `None` in the read direction.
    pub bytes_out: Option<PathBuf>,
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
    /// The model is outside what the specification admits. Exit 1.
    Refused(String),
    /// The command line. Exit 2.
    Usage(String),
    /// Ada 83 has no form for the construct. Exit 3. The
    /// materialiser's answer and nobody else's.
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
    let mut bytes_out = None;
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
            "--bytes-out" => bytes_out = Some(PathBuf::from(take()?)),
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
    if input.is_none() && bytes_out.is_none() {
        return Err(Refusal::Usage(
            "--bytes-out is required to write: this column has nowhere else to put the \
             octets"
                .to_string(),
        ));
    }

    Ok(Args {
        idl: idl.ok_or_else(|| Refusal::Usage("--idl is required".to_string()))?,
        type_name: type_name.ok_or_else(|| Refusal::Usage("--type is required".to_string()))?,
        values,
        input,
        values_out,
        bytes_out,
        encoding,
        endianness,
        encapsulation,
        dialect,
        out_dir: out_dir.ok_or_else(|| Refusal::Usage("--out-dir is required".to_string()))?,
    })
}

/// Generate `zd83_probe.adb`.
///
/// # Errors
/// [`Refusal`], classified so the probe can answer in the verdict's own
/// exit code.
#[allow(clippy::too_many_lines)]
pub fn source(args: &Args) -> Result<String, Refusal> {
    let text = std::fs::read_to_string(&args.idl)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", args.idl.display())))?;
    let ast = model_from_idl(&text, &args.idl.to_string_lossy(), args.dialect)?;
    let root = find_type(&ast, &args.type_name)?;
    // A cycle an array closes has no bound in any language — IDL 4.2
    // §7.4.1.4.4.4.4, `zerodds_ast_layout::plan::LayoutError::RecursionOutsideCollection`.
    // Checked here, before the materialiser ever runs, so the verdict
    // is `refused` (a specification question, exit 1) and not this
    // column's own `no-form`: `adapter::ada83::declare_type`'s `Array`
    // arm answers the identical question and is the fallback for a
    // caller that reaches the materialiser directly — RM83 3.10.1
    // independently admits an incomplete type only as an access type's
    // designated subtype, never as an array element, so both answers
    // are true and this one is what gets measured.
    refuse_recursion_outside_collection(&ast, root, layout_key(args))?;

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
    let applied = apply(&graph, &rule::ada83::catalogue())
        .map_err(|error| Refusal::Gap(format!("the Ada 83 rules do not apply: {error:?}")))?;
    // The materialiser is the authority on what Ada 83 has a form for,
    // so its refusal is exit 3 and every other stop is exit 6.
    let items = zerodds_lang_rules::adapter::ada83::ada83_items(&ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    let key = layout_key(args);
    let codec = if args.input.is_some() {
        let decoders = decoder_from::<Ada83>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        let mut all =
            ada83_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else {
        codec_from::<Ada83>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?
    };

    let mut out = String::from(
        "--  Generated by wireprobe-zerodds-ada83-gen. Do not edit.\n\
         --\n\
         --  The octets come out of runtime/ada-83 (Zd_Wire83) when this runs.\n\
         --  Nothing below decides a wire question.\n\
         \n\
         with Text_IO;\n\
         with Sequential_IO;\n\
         with Zd_Wire83;\n\
         \n\
         procedure Zd83_Probe is\n\
         \n   \
         package Zd83_Byte_IO is new Sequential_IO (Zd_Wire83.Byte);\n\
         \n   \
         --  Ada 83 has no Ada.Command_Line and so no Set_Exit_Status; the C\n   \
         --  library's own exit is imported instead (a GNAT extension, not\n   \
         --  gated on the language version selected by -gnat83).\n   \
         procedure Zd83_Exit (Status : Integer);\n   \
         pragma Import (C, Zd83_Exit, \"exit\");\n",
    );

    for decl in &items {
        out.push('\n');
        out.push_str(&indent(&zerodds_lang_render::ada83::decl(decl)));
    }
    for decl in &codec {
        out.push('\n');
        out.push_str(&indent(&zerodds_lang_render::ada83::decl(decl)));
    }
    if args.input.is_some() {
        out.push_str(&indent(
            &dump::dump(&ast, root, key.union_label).map_err(Refusal::Gap)?,
        ));
    }
    out.push_str(&harness(args, &ast, root, key, sample.as_ref())?);
    Ok(out)
}

/// Write `zd83_probe.adb` where `--out-dir` says.
///
/// # Errors
/// As [`source`], plus any write failure.
pub fn run(args: &Args) -> Result<(), Refusal> {
    let source = source(args)?;
    std::fs::create_dir_all(&args.out_dir)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", args.out_dir.display())))?;
    let main = args.out_dir.join("zd83_probe.adb");
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
        eprintln!("wireprobe-zerodds-ada83-gen: {}", refusal.message());
        std::process::exit(refusal.code());
    }
}

/// Every line of `text` moved one level into the procedure.
fn indent(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            let _ = writeln!(out, "   {line}");
        }
    }
    out
}

/// The Ada 83 subtype mark of the sample's own type.
fn sample_mark(ast: &Ast, root: Id<TypeNode>) -> Result<String, Refusal> {
    match zerodds_lang_rules::adapter::ada83::ada83_value_path(ast, root) {
        Ok(path) => Ok(path
            .segments()
            .iter()
            .map(zerodds_lang_cst::common::Ident::as_str)
            .collect::<Vec<&str>>()
            .join(".")),
        Err(error) => Err(Refusal::NoForm(format!(
            "the sample's type has no Ada 83 form: {error:?}"
        ))),
    }
}

/// The Ada 83 spelling of the framing aggregate the key adds up to.
fn framing_aggregate(key: LayoutKey) -> Result<String, Refusal> {
    let policy = match key.pid_flags.as_str() {
        "sentinel-must-understand" => "Flags_Sentinel_Must_Understand",
        "bare" => "Flags_Bare",
        "impl-extension-members" => "Flags_Impl_Extension_Members",
        other => {
            return Err(Refusal::Gap(format!(
                "`{other}` is a PID-flag policy runtime/ada-83 does not spell"
            )));
        }
    };
    let headers = key.pl_cdr1_headers;
    let flag = |value: bool| if value { "True" } else { "False" };
    Ok(format!(
        "(Pid_Flags => Zd_Wire83.{policy},\n      \
         Highest_Short_Id => {},\n      \
         Pad_Declared_Length => {},\n      \
         Pad_Declared_Optional_Length => {},\n      \
         Member_Must_Understand => {},\n      \
         Pad_After_Value => {})",
        key.short_pid_range.highest_short_id(),
        flag(headers.pads_declared_length()),
        flag(headers.pads_declared_optional_length()),
        flag(key.member_pid_carries_must_understand()),
        flag(headers.pads_after_value()),
    ))
}

/// Every `Zd_Wire83` exception a wire operation can raise, each with
/// the sentence it stands for.
///
/// RM83 11.3 gives `raise` no text to carry and there is no
/// `Ada.Exceptions.Exception_Message` to read one back with, so the
/// *handler* is what says which value the sample or the stream did not
/// fit. Every one of them is exit 1 —
/// `docs/specs/zerodds-verdicts-1.0.md`'s `refused` — and one arm each,
/// because one arm for all eleven answered every refusal in this column
/// with the same sentence, and a matrix cell whose only record is that
/// sentence then says nothing about which clause refused it.
const RUNTIME_EXCEPTIONS: &[(&str, &str)] = &[
    (
        "Zd_Wire83.Value_Out_Of_Range",
        "a value the wire width cannot hold",
    ),
    (
        "Zd_Wire83.Code_Point_Out_Of_Range",
        "a code point this char or wchar cannot carry",
    ),
    (
        "Zd_Wire83.Member_Id_Out_Of_Range",
        "a member id past what the header can name",
    ),
    (
        "Zd_Wire83.Length_Code_Mismatch",
        "a member body whose octet count is not the one its length code frames",
    ),
    (
        "Zd_Wire83.Anchor_Not_A_Holder_Width",
        "a bitset anchor or union overlay that is not one, two, four or eight octets",
    ),
    (
        "Zd_Wire83.Not_A_Fixed_Value",
        "a decimal this fixed<P,S> cannot hold",
    ),
    (
        "Zd_Wire83.Parameter_Too_Long",
        "a parameter value longer than a short header can describe",
    ),
    (
        "Zd_Wire83.Bit_Out_Of_Range",
        "a bit position or width outside its bitmask's or bitset's bound",
    ),
    (
        "Zd_Wire83.Stream_Truncated",
        "the stream ended where a value was expected",
    ),
    (
        "Zd_Wire83.Malformed_Stream",
        "the stream is not well-formed independent of its length",
    ),
    (
        "Zd_Wire83.Wire_Not_Implemented",
        "binary128 rests with the f128 arc in every column",
    ),
];

/// The exception handler every direction ends its statement part with.
///
/// One `when` per name in `RUNTIME_EXCEPTIONS`, each with its own
/// sentence. RM83 11.2 gives every handler a sequence of statements, so
/// a `when` with nothing under it — which is what a bare list of names
/// looks like — is what GNAT reports as "statement expected"; each arm
/// below carries its two statements. The Ada 2012 generator reaches one
/// `when Zd_Failure : others` and `Exception_Message` instead; Ada 83
/// has neither.
fn exception_handler() -> String {
    let mut out = String::from("exception\n");
    for (name, message) in RUNTIME_EXCEPTIONS {
        out.push_str(&format!(
            "   when {name} =>\n      \
             Text_IO.Put_Line\n        \
             (Text_IO.Standard_Error,\n         \
             \"wireprobe-zerodds-ada83: the sample is outside what this runtime \
             holds: {message}\");\n      \
             Zd83_Exit (1);\n"
        ));
    }
    out.push_str(
        "   \
         --  The four predefined ones by name. Ada 83 has no\n   \
         --  `Ada.Exceptions` and so no way to ask an unnamed exception\n   \
         --  what it was; a bare `when others` answers \"refused\" for a\n   \
         --  Constraint_Error out of a range check and for a\n   \
         --  Storage_Error out of an allocation alike, and the two want\n   \
         --  different work. Naming them costs four lines and says which\n   \
         --  happened.\n   \
         when Constraint_Error =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: constraint_error\");\n      \
         Zd83_Exit (1);\n   \
         when Numeric_Error =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: numeric_error\");\n      \
         Zd83_Exit (1);\n   \
         when Program_Error =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: program_error\");\n      \
         Zd83_Exit (1);\n   \
         when Storage_Error =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: storage_error\");\n      \
         Zd83_Exit (1);\n   \
         --  The file ones. RM83 14.4 puts them in `Io_Exceptions`, and\n   \
         --  both `Text_IO` and the `Sequential_IO` instance rename the\n   \
         --  same exceptions from there, so naming them once through\n   \
         --  `Text_IO` catches either. Without this a missing input file\n   \
         --  and a decoder fault answer the same word.\n   \
         when Text_IO.Name_Error =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: name_error\");\n      \
         Zd83_Exit (1);\n   \
         when Text_IO.Use_Error =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: use_error\");\n      \
         Zd83_Exit (1);\n   \
         when Text_IO.End_Error =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: end_error\");\n      \
         Zd83_Exit (1);\n   \
         when Text_IO.Data_Error =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: data_error\");\n      \
         Zd83_Exit (1);\n   \
         when Text_IO.Status_Error =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: status_error\");\n      \
         Zd83_Exit (1);\n   \
         when Text_IO.Mode_Error =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: mode_error\");\n      \
         Zd83_Exit (1);\n   \
         when others =>\n      \
         Text_IO.Put_Line\n        \
         (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: refused\");\n      \
         Zd83_Exit (1);\n\
         end Zd83_Probe;\n",
    );
    out
}

/// The Ada string literal naming an external file, resolved against the
/// generator's own working directory when the caller wrote it relative.
///
/// RM83 14.1 leaves the interpretation of an external file's name to the
/// implementation, and every implementation reads a relative name
/// against the directory the program runs in. This column has no
/// `Ada.Command_Line`, so the name is frozen into the source here — but
/// the program that opens it is built and run in a directory of the
/// driver's making, not the one the caller stood in. Freezing the name
/// without freezing what it is relative to is what raised
/// `Text_IO.Name_Error` (RM83 14.4) out of the read direction while the
/// write direction, whose path the driver already spelled absolutely,
/// carried its bytes.
fn baked_path_literal(path: &Path) -> String {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    };
    fill::ada_string_literal(&resolved.to_string_lossy())
}

/// The tail of `zd83_probe.adb`: the declarations the harness needs,
/// the statements, and the exception handler.
#[allow(clippy::too_many_lines)]
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    sample: Option<&serde_json::Value>,
) -> Result<String, Refusal> {
    let order = match args.endianness {
        Endianness::Little => "Zd_Wire83.Little",
        Endianness::Big => "Zd_Wire83.Big",
    };
    let max_align = args.encoding.max_alignment();
    let framing = framing_aggregate(key)?;
    let mark = sample_mark(ast, root)?;
    let stderr_line = |message: &str| {
        format!(
            "Text_IO.Put_Line (Text_IO.Standard_Error, \
             \"wireprobe-zerodds-ada83: {message}\");"
        )
    };

    let mut out = String::new();

    if let Some(input) = &args.input {
        let decoder = decoder_name::<Ada83>(ast, root);
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        // Every object of this harness in one block, opened after
        // `begin`. Ada 83's declarative_part (RM 3.9) is basic
        // declarative items first and bodies after, so an object
        // declaration written below the codec bodies is rejected —
        // "decl cannot appear after body". Ada 95 dropped that rule,
        // which is why the Ada 2012 generator can put them there. The
        // block also keeps `zd_data` alive for the reader further down;
        // it used to be declared in a block that ended before its last
        // use.
        let _ = write!(
            out,
            "\nbegin\n   \
             declare\n      \
             zd_framing : constant Zd_Wire83.Pl_Cdr1_Framing :=\n        {framing};\n      \
             zd_file : Zd83_Byte_IO.File_Type;\n      \
             zd_octet : Zd_Wire83.Byte;\n      \
             zd_input : Zd_Wire83.Byte_Array_Access :=\n        \
             new Zd_Wire83.Byte_Array (0 .. 0);\n      \
             zd_length : Natural := 0;\n      \
             zd_data : Zd_Wire83.Byte_Array_Access;\n   \
             begin\n      \
             Zd83_Byte_IO.Open (zd_file, Zd83_Byte_IO.In_File, {});\n      \
             while not Zd83_Byte_IO.End_Of_File (zd_file) loop\n         \
             Zd83_Byte_IO.Read (zd_file, zd_octet);\n         \
             if zd_length > zd_input'Last then\n            \
             declare\n               \
             zd_grown : constant Zd_Wire83.Byte_Array_Access :=\n                 \
             new Zd_Wire83.Byte_Array (0 .. 2 * zd_input'Length);\n            \
             begin\n               \
             zd_grown (0 .. zd_length - 1) := zd_input (0 .. zd_length - 1);\n               \
             zd_input := zd_grown;\n            \
             end;\n         \
             end if;\n         \
             zd_input (zd_length) := zd_octet;\n         \
             zd_length := zd_length + 1;\n      \
             end loop;\n      \
             Zd83_Byte_IO.Close (zd_file);\n      \
             zd_data := new Zd_Wire83.Byte_Array'(zd_input (0 .. zd_length - 1));\n",
            baked_path_literal(input)
        );
        if args.encapsulation {
            let _ = write!(
                out,
                "   if zd_data'Length < 4 then\n      \
                 {}\n      \
                 Zd83_Exit (1);\n      \
                 return;\n   \
                 end if;\n   \
                 declare\n      \
                 zd_declared : constant Natural :=\n        \
                 Natural (zd_data (zd_data'First + 3)) mod 4;\n      \
                 zd_after : constant Zd_Wire83.Byte_Array_Access :=\n        \
                 new Zd_Wire83.Byte_Array'\n          \
                 (zd_data (zd_data'First + 4 .. zd_data'Last - zd_declared));\n   \
                 begin\n      \
                 if zd_data'Length - 4 < zd_declared then\n         \
                 {}\n         \
                 Zd83_Exit (1);\n         \
                 return;\n      \
                 end if;\n      \
                 zd_data := zd_after;\n   \
                 end;\n",
                stderr_line("fewer than the four-byte encapsulation header"),
                stderr_line("the options field declares more padding than follows the header"),
            );
        }
        let _ = write!(
            out,
            "   declare\n      \
             zd_reader : Zd_Wire83.Reader_Access :=\n        \
             Zd_Wire83.New_Reader (zd_data, {order}, {max_align}, zd_framing);\n      \
             zd_value : {mark};\n   \
             begin\n      \
             --  The top-level type is not a member of anything, so no\n      \
             --  @try_construct governs it and 7.2.2.7's default stands.\n      \
             zd_value := {decoder} (zd_reader, Zd_Wire83.Tc_Discard);\n      \
             --  The operator qualified. Ada 83 has no `use type`, so\n      \
             --  equality on `Status` is not directly visible here, and\n      \
             --  plain `use Zd_Wire83` would put every runtime name in\n      \
             --  scope — including `Text` and `Status`, which an IDL is\n      \
             --  free to use for its own types.\n      \
             if Zd_Wire83.\"=\" (Zd_Wire83.Reader_Status (zd_reader),\n                        \
             Zd_Wire83.Status_Discarded) then\n         \
             --  Not a failed decode: the receiver drops the object and\n         \
             --  carries on. Exit code 3, the Rust probe's.\n         \
             Text_IO.Put_Line\n           \
             (Text_IO.Standard_Error, \"wireprobe-zerodds-ada83: discarded\");\n         \
             Zd83_Exit (3);\n         \
             return;\n      \
             end if;\n      \
             if not Zd_Wire83.\"=\" (Zd_Wire83.Reader_Status (zd_reader),\n                            \
             Zd_Wire83.Status_Ok) then\n         \
             Text_IO.Put_Line\n           \
             (Text_IO.Standard_Error,\n            \
             \"wireprobe-zerodds-ada83: \" &\n            \
             Zd_Wire83.Status_Text (Zd_Wire83.Reader_Status (zd_reader)));\n         \
             Zd83_Exit (1);\n         \
             return;\n      \
             end if;\n      \
             --  Nothing may be left over that is not padding: a decoder\n      \
             --  that stopped short and said nothing is the silent-drop\n      \
             --  shape.\n      \
             declare\n         \
             zd_left : constant Natural := Zd_Wire83.Reader_Remaining (zd_reader);\n         \
             zd_pad : constant Natural :=\n           \
             ({max_align} - Zd_Wire83.Reader_Position (zd_reader) mod {max_align})\n           \
             mod {max_align};\n      \
             begin\n         \
             if zd_left > zd_pad then\n            \
             {}\n            \
             Zd83_Exit (1);\n            \
             return;\n         \
             end if;\n      \
             end;\n      \
             declare\n         \
             zd_doc : constant Zd_Wire83.Text :=\n           \
             {} (zd_value, \"sample\");\n         \
             zd_sink : Text_IO.File_Type;\n      \
             begin\n         \
             Text_IO.Create (zd_sink, Text_IO.Out_File, {});\n         \
             Text_IO.Put_Line (zd_sink, zd_doc.all);\n         \
             Text_IO.Close (zd_sink);\n      \
             end;\n   \
             end;\n\
             end;\n",
            stderr_line("bytes left past the padding at the sample's end"),
            dump::dumper_name(ast, root),
            baked_path_literal(values_out)
        );
        out.push_str(&exception_handler());
        return Ok(out);
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let bytes_out = args
        .bytes_out
        .as_ref()
        .ok_or_else(|| Refusal::Usage("--bytes-out is required to write".to_string()))?;
    let filled = fill::fill(ast, root, sample, "zd_sample").map_err(Refusal::Gap)?;
    let encoder = codec_name::<Ada83>(ast, root, Purpose::Data);
    let flag = |value: bool| if value { "True" } else { "False" };
    let short_codes = flag(key.length_codes.short_code_where_it_fits());

    // The same block as in the decode branch above, and for the same
    // reason: in Ada 83 an object declaration may not follow a body.
    let _ = write!(
        out,
        "\nbegin\n   \
         declare\n      \
         zd_framing : constant Zd_Wire83.Pl_Cdr1_Framing :=\n        {framing};\n      \
         zd_writer : Zd_Wire83.Writer_Access :=\n        \
         Zd_Wire83.New_Writer ({order}, {max_align}, zd_framing, {short_codes});\n      \
         zd_sample : {mark};\n   \
         begin\n"
    );
    for line in filled {
        let _ = writeln!(out, "      {line}");
    }
    let _ = write!(
        out,
        "      {encoder} (zd_writer, zd_sample);\n      \
         declare\n         \
         zd_bytes : constant Zd_Wire83.Byte_Array_Access := Zd_Wire83.Finish (zd_writer);\n         \
         zd_file : Zd83_Byte_IO.File_Type;\n      \
         begin\n         \
         Zd83_Byte_IO.Create (zd_file, Zd83_Byte_IO.Out_File, {});\n",
        baked_path_literal(bytes_out)
    );
    if args.encapsulation {
        let header = encapsulation_header(args.encoding, args.endianness);
        let _ = write!(
            out,
            "         --  The encapsulation identifier 7.6.3.1.2 gives this encoding\n         \
             --  and byte order; the options field is zero here.\n         \
             Zd83_Byte_IO.Write (zd_file, {});\n         \
             Zd83_Byte_IO.Write (zd_file, {});\n         \
             Zd83_Byte_IO.Write (zd_file, 0);\n         \
             Zd83_Byte_IO.Write (zd_file, 0);\n",
            header[0], header[1]
        );
    }
    let _ = write!(
        out,
        "         for zd_i in zd_bytes'Range loop\n            \
         Zd83_Byte_IO.Write (zd_file, zd_bytes (zd_i));\n         \
         end loop;\n         \
         Zd83_Byte_IO.Close (zd_file);\n      \
         end;\n   \
         end;\n"
    );
    out.push_str(&exception_handler());
    Ok(out)
}

/// The layout key the flags add up to.
fn layout_key(args: &Args) -> LayoutKey {
    LayoutKey::in_dialect(args.dialect, args.encoding, args.endianness, Purpose::Data)
}

/// Refuse a type whose cycle an array closes, in the specification's
/// own words — before the materialiser ever runs.
///
/// IDL 4.2 §7.4.1.4.4.4.4 admits a recursive structure or union only
/// through a member with a sequence, a map or `@optional`: each of the
/// three carries a way for a value to be finite. An array's element
/// count is fixed and non-zero, so nothing about a value can stop the
/// descent — `zerodds_ast_layout::xcdr::plan` refuses this the same
/// way for every column, and this is that same check, called early so
/// the answer is `refused` and not this column's own `no-form`.
///
/// # Errors
/// [`Refusal::Refused`] naming the clause, for exactly this shape.
/// Every other [`zerodds_ast_layout::plan::LayoutError`] is left for
/// `codec_from`/`decoder_from` to report later, in its own place: this
/// function answers one question and nothing else, so a layout
/// question with a different cause is not silently absorbed here.
fn refuse_recursion_outside_collection(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
) -> Result<(), Refusal> {
    let mut deps = zerodds_ast_layout::xcdr::DepCollector::default();
    if let Err(zerodds_ast_layout::plan::LayoutError::RecursionOutsideCollection { .. }) =
        zerodds_ast_layout::xcdr::plan(ast, root, key, &mut deps)
    {
        return Err(Refusal::Refused(
            "a member's type closes a cycle with nothing on it that can terminate a \
             value — IDL 4.2 §7.4.1.4.4.4.4 admits a recursive structure or union only \
             through a sequence, a map or an @optional member, each of which gives a \
             value a way to be finite; an array's element count is fixed and non-zero, \
             so nothing about a value can stop the descent"
                .to_string(),
        ));
    }
    Ok(())
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

/// The model, lowered **in the dialect**. Identical reasoning to
/// `wireprobe-zerodds-ada-gen::model_from_idl`.
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
        // The third: whether a declaration may carry the name of the scope
        // it sits in (IDL 4.2 §7.5.2). Fast DDS, RTI and both ROS
        // middlewares take `resolve-module-named-as-type` and write
        // octets. Without the dialect's answer a column refuses the case in
        // every dialect: measured by `reference_columns.rs` on the C++
        // column, whose shared copy in `wireprobe-zerodds-gen` lacked it,
        // as 140 slots where the Rust column wrote octets. Not measured on
        // this column.
        name_of_the_enclosing_scope: if dialect.admits_the_name_of_the_enclosing_scope() {
            zerodds_idl_lower::NameOfTheEnclosingScope::Admitted
        } else {
            zerodds_idl_lower::NameOfTheEnclosingScope::Refused
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
    // The report lists what the pass could not resolve; a reference that
    // resolves to nothing is a member that is not written, so it is
    // refused here as `wireprobe-zerodds-gen::model_from_idl` refuses it.
    let report = ast
        .resolve_all_by_name()
        .map_err(|error| Refusal::Gap(format!("{path}: {error}")))?;
    if !report.is_clean() {
        return Err(Refusal::Gap(format!(
            "{path}: unresolved after the resolve pass: {:?}",
            report.broken
        )));
    }
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
