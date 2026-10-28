// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-ada-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-ada-gen` — the Ada program the Ada probe builds
//! and runs.
//!
//! One IDL file, one type, one JSON sample and one layout key in; one
//! `zd_probe.adb` holding the types, the codec, the sample and the
//! harness that writes the octets to standard output — or, with
//! `--in`, reads them back and writes the values as JSON where
//! `--values-out` says. `tools/wireprobe/wireprobe-zerodds-ada` is the
//! shell around it, and compiles `zd_probe.adb` together with
//! `runtime/ada/src/zd_wire.adb`.
//!
//! Nothing here writes a byte. The declarations come from the Ada
//! materialiser, the codec from the shared walk through
//! [`zerodds_lang_rules::emit::ada::Ada`], and the bytes come out of
//! `runtime/ada` when the program runs — a further reading of
//! DDS-XTypes 1.3 §7.4, calling nothing in Rust.
//!
//! The whole program is one parameterless procedure: Ada declares
//! types, functions and objects in a declarative part, so the probe
//! needs no package pair and GNAT builds the one file beside the
//! runtime.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

mod dump;
mod fill;

use std::fmt::Write as _;
use std::path::PathBuf;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::default_ada::ada_defaulters_called_by;
use zerodds_lang_rules::emit::ada::Ada;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

const USAGE: &str = "\
wireprobe-zerodds-ada-gen — the Ada the ZeroDDS Ada probe builds and runs

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
    --out-dir <dir>         where zd_probe.adb goes (required)

The defaults are `wireprobe-zerodds`'s, because an Ada column that
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
    /// Ada has no form for the construct. Exit 3. The materialiser's
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

/// Generate `zd_probe.adb`.
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
    let applied = apply(&graph, &rule::ada::catalogue())
        .map_err(|error| Refusal::Gap(format!("the Ada rules do not apply: {error:?}")))?;
    // The materialiser is the authority on what Ada has a form for, so
    // its refusal is exit 3 and every other stop is exit 6.
    let items = zerodds_lang_rules::adapter::ada::ada_items(&ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;

    let key = layout_key(args);
    let codec = if args.input.is_some() {
        let decoders = decoder_from::<Ada>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        let mut all =
            ada_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else {
        codec_from::<Ada>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?
    };

    let mut out = String::from(
        "--  Generated by wireprobe-zerodds-ada-gen. Do not edit.\n\
         --\n\
         --  The octets come out of runtime/ada (Zd_Wire) when this runs.\n\
         --  Nothing below decides a wire question.\n\
         \n\
         with Ada.Command_Line;\n\
         with Ada.Exceptions;\n\
         with Ada.Streams.Stream_IO;\n\
         with Ada.Strings.Unbounded;\n\
         with Ada.Text_IO.Text_Streams;\n\
         with Interfaces;\n\
         with Zd_Wire;\n\
         use type Interfaces.Unsigned_8;\n\
         use type Interfaces.Unsigned_16;\n\
         use type Interfaces.Unsigned_32;\n\
         use type Interfaces.Unsigned_64;\n\
         use type Interfaces.Integer_8;\n\
         use type Interfaces.Integer_16;\n\
         use type Interfaces.Integer_32;\n\
         use type Interfaces.Integer_64;\n\
         use type Interfaces.IEEE_Float_32;\n\
         use type Interfaces.IEEE_Float_64;\n\
         use type Zd_Wire.Status;\n\
         use type Zd_Wire.Try_Construct;\n\
         \n\
         procedure Zd_Probe is\n\
         \n   \
         package ASU renames Ada.Strings.Unbounded;\n",
    );

    for decl in &items {
        out.push('\n');
        out.push_str(&indent(&zerodds_lang_render::ada::decl(decl)));
    }
    for decl in &codec {
        out.push('\n');
        out.push_str(&indent(&zerodds_lang_render::ada::decl(decl)));
    }
    if args.input.is_some() {
        out.push_str(&indent(
            &dump::dump(&ast, root, key.union_label).map_err(Refusal::Gap)?,
        ));
    }
    out.push_str(&harness(args, &ast, root, key, sample.as_ref())?);
    Ok(out)
}

/// Write `zd_probe.adb` where `--out-dir` says.
///
/// # Errors
/// As [`source`], plus any write failure.
pub fn run(args: &Args) -> Result<(), Refusal> {
    let source = source(args)?;
    std::fs::create_dir_all(&args.out_dir)
        .map_err(|error| Refusal::Gap(format!("{}: {error}", args.out_dir.display())))?;
    let main = args.out_dir.join("zd_probe.adb");
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
        eprintln!("wireprobe-zerodds-ada-gen: {}", refusal.message());
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

/// The Ada subtype mark of the sample's own type.
fn sample_mark(ast: &Ast, root: Id<TypeNode>) -> Result<String, Refusal> {
    match zerodds_lang_rules::adapter::ada::ada_value_path(ast, root) {
        Ok(path) => Ok(path
            .segments()
            .iter()
            .map(zerodds_lang_cst::common::Ident::as_str)
            .collect::<Vec<&str>>()
            .join(".")),
        Err(error) => Err(Refusal::NoForm(format!(
            "the sample's type has no Ada form: {error:?}"
        ))),
    }
}

/// The Ada spelling of the framing aggregate the key adds up to.
fn framing_aggregate(key: LayoutKey) -> Result<String, Refusal> {
    let policy = match key.pid_flags.as_str() {
        "sentinel-must-understand" => "Flags_Sentinel_Must_Understand",
        "bare" => "Flags_Bare",
        "impl-extension-members" => "Flags_Impl_Extension_Members",
        other => {
            return Err(Refusal::Gap(format!(
                "`{other}` is a PID-flag policy runtime/ada does not spell"
            )));
        }
    };
    let headers = key.pl_cdr1_headers;
    let flag = |value: bool| if value { "True" } else { "False" };
    Ok(format!(
        "(Pid_Flags => Zd_Wire.{policy},\n      \
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

/// The tail of `zd_probe.adb`: the declarations the harness needs, the
/// statements, and the exception handler that turns any wire failure
/// into exit 1.
#[allow(clippy::too_many_lines)]
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    sample: Option<&serde_json::Value>,
) -> Result<String, Refusal> {
    let order = match args.endianness {
        Endianness::Little => "Zd_Wire.Little",
        Endianness::Big => "Zd_Wire.Big",
    };
    let max_align = args.encoding.max_alignment();
    let framing = framing_aggregate(key)?;
    let mark = sample_mark(ast, root)?;
    let stderr_line = |message: &str| {
        format!(
            "Ada.Text_IO.Put_Line (Ada.Text_IO.Standard_Error, \
             \"wireprobe-zerodds-ada: {message}\");"
        )
    };

    let mut out = String::new();

    if let Some(input) = &args.input {
        let decoder = decoder_name::<Ada>(ast, root);
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        let _ = write!(
            out,
            "\n   \
             zd_framing : constant Zd_Wire.Pl_Cdr1_Framing :=\n     {framing};\n   \
             zd_input : Zd_Wire.Byte_Array_Access := null;\n   \
             zd_data : Zd_Wire.Byte_Array_Access := null;\n\
             \nbegin\n"
        );
        let _ = write!(
            out,
            "   declare\n      \
             Zd_File : Ada.Streams.Stream_IO.File_Type;\n   \
             begin\n      \
             Ada.Streams.Stream_IO.Open\n        \
             (Zd_File, Ada.Streams.Stream_IO.In_File, {});\n      \
             declare\n         \
             Zd_Size : constant Natural :=\n           \
             Natural (Ada.Streams.Stream_IO.Size (Zd_File));\n      \
             begin\n         \
             zd_input := new Zd_Wire.Byte_Array (0 .. Zd_Size - 1);\n         \
             if Zd_Size > 0 then\n            \
             Zd_Wire.Byte_Array'Read\n              \
             (Ada.Streams.Stream_IO.Stream (Zd_File), zd_input.all);\n         \
             end if;\n      \
             end;\n      \
             Ada.Streams.Stream_IO.Close (Zd_File);\n   \
             end;\n",
            fill::ada_string_literal(&input.to_string_lossy())
        );
        if args.encapsulation {
            // The encapsulation identifier is not re-read for the
            // encoding: the caller named one. The options word is read
            // — §7.6.3.1.2's low two bits say where the data ended.
            let _ = write!(
                out,
                "   if zd_input'Length < 4 then\n      \
                 {}\n      \
                 Ada.Command_Line.Set_Exit_Status (1);\n      \
                 return;\n   \
                 end if;\n   \
                 declare\n      \
                 zd_declared : constant Natural :=\n        \
                 Natural (zd_input (zd_input'First + 3) and 3);\n   \
                 begin\n      \
                 if zd_input'Length - 4 < zd_declared then\n         \
                 {}\n         \
                 Ada.Command_Line.Set_Exit_Status (1);\n         \
                 return;\n      \
                 end if;\n      \
                 zd_data := new Zd_Wire.Byte_Array'\n        \
                 (zd_input (zd_input'First + 4 .. zd_input'Last - zd_declared));\n   \
                 end;\n",
                stderr_line("fewer than the four-byte encapsulation header"),
                stderr_line("the options field declares more padding than follows the header"),
            );
        } else {
            out.push_str("   zd_data := zd_input;\n");
        }
        let _ = write!(
            out,
            "   declare\n      \
             zd_reader : Zd_Wire.Reader :=\n        \
             Zd_Wire.New_Reader (zd_data, {order}, {max_align}, zd_framing);\n      \
             zd_value : {mark};\n   \
             begin\n      \
             --  The top-level type is not a member of anything, so no\n      \
             --  @try_construct governs it and 7.2.2.7's default stands.\n      \
             zd_value := {decoder} (zd_reader, Zd_Wire.Tc_Discard);\n      \
             if Zd_Wire.Reader_Status (zd_reader) = Zd_Wire.Status_Discarded then\n         \
             --  Not a failed decode: the receiver drops the object and\n         \
             --  carries on. Exit code 3, the Rust probe's.\n         \
             Ada.Text_IO.Put_Line\n           \
             (Ada.Text_IO.Standard_Error,\n            \
             \"wireprobe-zerodds-ada: at\"\n            \
             & Natural'Image (Zd_Wire.Reader_Position (zd_reader))\n            \
             & \": discarded\");\n         \
             Ada.Command_Line.Set_Exit_Status (3);\n         \
             return;\n      \
             end if;\n      \
             if Zd_Wire.Reader_Status (zd_reader) /= Zd_Wire.Status_Ok then\n         \
             Ada.Text_IO.Put_Line\n           \
             (Ada.Text_IO.Standard_Error,\n            \
             \"wireprobe-zerodds-ada: at\"\n            \
             & Natural'Image (Zd_Wire.Reader_Position (zd_reader))\n            \
             & \": \" & Zd_Wire.Status_Text (Zd_Wire.Reader_Status (zd_reader)));\n         \
             Ada.Command_Line.Set_Exit_Status (1);\n         \
             return;\n      \
             end if;\n      \
             --  Nothing may be left over that is not padding: a decoder\n      \
             --  that stopped short and said nothing is the silent-drop\n      \
             --  shape.\n      \
             declare\n         \
             zd_left : constant Natural := Zd_Wire.Reader_Remaining (zd_reader);\n         \
             zd_pad : constant Natural :=\n           \
             ({max_align} - Zd_Wire.Reader_Position (zd_reader) mod {max_align})\n           \
             mod {max_align};\n      \
             begin\n         \
             if zd_left > zd_pad then\n            \
             {}\n            \
             Ada.Command_Line.Set_Exit_Status (1);\n            \
             return;\n         \
             end if;\n      \
             end;\n      \
             declare\n         \
             zd_doc : constant ASU.Unbounded_String :=\n           \
             {} (zd_value, \"sample\");\n         \
             zd_sink : Ada.Text_IO.File_Type;\n      \
             begin\n         \
             Ada.Text_IO.Create (zd_sink, Ada.Text_IO.Out_File, {});\n         \
             Ada.Text_IO.Put_Line (zd_sink, ASU.To_String (zd_doc));\n         \
             Ada.Text_IO.Close (zd_sink);\n      \
             end;\n   \
             end;\n",
            stderr_line("bytes left past the padding at the sample's end"),
            dump::dumper_name(ast, root),
            fill::ada_string_literal(&values_out.to_string_lossy())
        );
        let _ = write!(
            out,
            "exception\n   \
             when Zd_Failure : others =>\n      \
             Ada.Text_IO.Put_Line\n        \
             (Ada.Text_IO.Standard_Error,\n         \
             \"wireprobe-zerodds-ada: \"\n         \
             & Ada.Exceptions.Exception_Message (Zd_Failure));\n      \
             Ada.Command_Line.Set_Exit_Status (1);\n\
             end Zd_Probe;\n"
        );
        return Ok(out);
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let filled = fill::fill(ast, root, sample, "zd_sample").map_err(Refusal::Gap)?;
    let encoder = codec_name::<Ada>(ast, root, Purpose::Data);
    let flag = |value: bool| if value { "True" } else { "False" };
    let short_codes = flag(key.length_codes.short_code_where_it_fits());

    let _ = write!(
        out,
        "\n   \
         zd_framing : constant Zd_Wire.Pl_Cdr1_Framing :=\n     {framing};\n   \
         zd_writer : Zd_Wire.Writer :=\n     \
         Zd_Wire.New_Writer ({order}, {max_align}, zd_framing, {short_codes});\n   \
         zd_sample : {mark};\n   \
         zd_stdout : constant Ada.Text_IO.Text_Streams.Stream_Access :=\n     \
         Ada.Text_IO.Text_Streams.Stream (Ada.Text_IO.Standard_Output);\n\
         \nbegin\n"
    );
    for line in filled {
        let _ = writeln!(out, "   {line}");
    }
    let _ = write!(
        out,
        "   {encoder} (zd_writer, zd_sample);\n   \
         declare\n      \
         zd_bytes : constant Zd_Wire.Byte_Array_Access := Zd_Wire.Finish (zd_writer);\n   \
         begin\n"
    );
    if args.encapsulation {
        let header = encapsulation_header(args.encoding, args.endianness);
        let _ = write!(
            out,
            "      --  The encapsulation identifier 7.6.3.1.2 gives this encoding\n      \
             --  and byte order; the options field is zero here.\n      \
             declare\n         \
             zd_encap : constant Zd_Wire.Byte_Array :=\n           \
             (0 => {}, 1 => {}, 2 => 0, 3 => 0);\n      \
             begin\n         \
             Zd_Wire.Byte_Array'Write (zd_stdout, zd_encap);\n      \
             end;\n",
            header[0], header[1]
        );
    }
    let _ = write!(
        out,
        "      Zd_Wire.Byte_Array'Write (zd_stdout, zd_bytes.all);\n   \
         end;\n\
         exception\n   \
         when Zd_Failure : others =>\n      \
         Ada.Text_IO.Put_Line\n        \
         (Ada.Text_IO.Standard_Error,\n         \
         \"wireprobe-zerodds-ada: \"\n         \
         & Ada.Exceptions.Exception_Message (Zd_Failure));\n      \
         Ada.Command_Line.Set_Exit_Status (1);\n\
         end Zd_Probe;\n"
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

/// The model, lowered **in the dialect**.
///
/// Two answers are settled before any layout plan and the `LayoutKey`
/// cannot carry them, because they decide what the member *is* and not
/// how it is written: whether an `@hashid` member advances the
/// sequential counter the members after it take their identifiers from,
/// and which spelling of an escaped name the hash is taken over.
/// `Options::default()` answers both for the specification and loses
/// the dialect's answer, so the two knobs come from `Dialect` here —
/// the same shape as `wireprobe-zerodds-d-gen::model_from_idl`.
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
