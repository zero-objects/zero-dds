// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-lua-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-lua-gen` — the Lua chunk the Lua probe runs.
//!
//! One IDL file, one type, one JSON sample and one layout key in; one
//! `probe.lua` holding the descriptor module, the codec, the sample and
//! the statements that write it to standard output.
//! `tools/wireprobe/wireprobe-zerodds-lua` is the shell around it.
//!
//! One file, because a Lua script is a file: the descriptor module the
//! materialiser produces is embedded whole as
//! `local M = (function() … end)()` — the same chunk `require` would
//! run, byte for byte — and the codec, the defaulters and the dumpers
//! sit beside it in one `ZD` table. Only the wire runtime is required
//! in (`local zdw = require("zerodds_wire")`), from `runtime/lua`,
//! which implements DDS-XTypes 1.3 §7.4 in plain Lua and calls nothing
//! in Rust: a probe that reached the Rust core through FFI would be
//! byte-identical by construction and would prove nothing about whether
//! the two read the specification the same way.
//!
//! ## Both directions
//!
//! `--values` generates the write direction: the sample is filled from
//! the JSON and the octets go to standard output. `--in` generates the
//! read direction: the octets are read from the named file and the
//! values go to the file `--values-out` names, as plain JSON in the
//! dialect `crates/wire-probe/src/lib.rs` documents. Not canonical
//! form: the caller canonicalises in Rust, as every sibling column
//! does — a second canonicaliser in a second language would be a
//! second chance for two columns to part over a spelling.

#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

mod dump;
mod fill;

use std::fmt::Write as _;
use std::path::PathBuf;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::default_lua::lua_defaulters_called_by;
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::emit::lua::{CODECS, Lua, RUNTIME};
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

const USAGE: &str = "\
wireprobe-zerodds-lua-gen — the Lua the ZeroDDS Lua probe runs

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
    --out-dir <dir>         where probe.lua goes (required)

The defaults are `wireprobe-zerodds`'s, because a Lua column that
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

/// Why the generator could not produce a file. Three kinds and not one
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
    /// Lua has no form for the construct — the materialiser's answer
    /// and nobody else's. Exit 3.
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

/// Generate `probe.lua`.
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
    let applied = apply(&graph, &rule::lua::catalogue())
        .map_err(|error| Refusal::Gap(format!("the Lua rules do not apply: {error:?}")))?;
    // The materialiser is the authority on what Lua has a form for, so
    // its refusal is exit 3 and every other stop is exit 6.
    let module = zerodds_lang_rules::adapter::lua::lua_file(&ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;
    let module_text = zerodds_lang_render::lua::file(&module);

    let key = layout_key(args);
    // What the chosen direction calls, and nothing else: a codec the
    // probe never reaches is source the interpreter still compiles, and
    // generating both halves would double every case in a sweep.
    let codec = if args.input.is_some() {
        let decoders = decoder_from::<Lua>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?;
        // Table 9's defaulters come with the decoders: a decoder opens
        // with `local v = ZD.zd_def_T()`. A `ZD` lookup resolves at the
        // call, so the order carries no requirement — the presence
        // does.
        let mut all =
            lua_defaulters_called_by(&ast, &decoders).map_err(|error| from_codec(&error))?;
        all.extend(decoders);
        all
    } else {
        codec_from::<Lua>(&ast, root, key, codec_options(args))
            .map_err(|error| from_codec(&error))?
    };
    let codec_text = codec
        .iter()
        .map(zerodds_lang_render::lua::stat)
        .collect::<Vec<_>>()
        .join("\n");

    let mut out = String::from(
        "-- Generated by wireprobe-zerodds-lua-gen. Do not edit.\n\
         --\n\
         -- The octets come out of runtime/lua/zerodds_wire when this runs.\n\
         -- Nothing below decides a wire question.\n\n",
    );
    let _ = writeln!(out, "local {RUNTIME} = require(\"zerodds_wire\")\n");
    // The descriptor module, embedded whole: the same chunk `require`
    // would run, wrapped so its `return M` lands in a local.
    out.push_str("local M = (function()\n");
    out.push_str(&module_text);
    out.push_str("end)()\n");
    // `M` is read by the codec's enumerator constants; a module with no
    // enumeration leaves it unread, and Lua says nothing about an
    // unread local — the reference keeps the reader honest either way.
    out.push_str("local _ = M\n\n");
    let _ = writeln!(out, "local {CODECS} = {{}}\n");
    out.push_str(&codec_text);
    out.push('\n');
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
    let path = args.out_dir.join("probe.lua");
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
        eprintln!("wireprobe-zerodds-lua-gen: {}", refusal.message());
        std::process::exit(refusal.code());
    }
}

/// A Lua string literal for the harness.
fn quoted(text: &str) -> String {
    format!("\"{}\"", zerodds_lang_render::lua::escape_string(text))
}

/// The tail of `probe.lua`: the sample and the stream out, or the bytes
/// and the values back.
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    sample: Option<&serde_json::Value>,
) -> Result<String, Refusal> {
    let order = match args.endianness {
        Endianness::Little => "\"le\"",
        Endianness::Big => "\"be\"",
    };
    let max_align = args.encoding.max_alignment();
    let mut out =
        String::from("-- -- the probe ------------------------------------------------------\n");

    if let Some(input) = &args.input {
        let decoder = decoder_name::<Lua>(ast, root);
        let values_out = args
            .values_out
            .as_ref()
            .ok_or_else(|| Refusal::Usage("--values-out is required with --in".to_string()))?;
        out.push_str(&dump::dump(ast, root, key.union_label).map_err(Refusal::Gap)?);
        let _ = write!(
            out,
            "\nlocal zd_handle = assert(io.open({}, \"rb\"))\n\
             local zd_input = zd_handle:read(\"a\")\n\
             zd_handle:close()\n",
            quoted(&input.to_string_lossy())
        );
        if args.encapsulation {
            // The encapsulation identifier is not re-read for the
            // encoding — the caller named one. The **options** word is
            // read, because §7.6.3.1.2 says the receiver shall use its
            // low two bits to find where the serialized data ended.
            out.push_str(
                "if #zd_input < 4 then\n\
                 \x20 io.stderr:write(\"wireprobe-zerodds-lua: fewer than the four-byte \
                 encapsulation header\\n\")\n\
                 \x20 os.exit(1)\n\
                 end\n\
                 local zd_declared = string.byte(zd_input, 4) & 3\n\
                 local zd_body = string.sub(zd_input, 5)\n\
                 if #zd_body < zd_declared then\n\
                 \x20 io.stderr:write(\"wireprobe-zerodds-lua: the options field declares \"\n\
                 \x20   .. zd_declared .. \" padding byte(s) and only \" .. #zd_body\n\
                 \x20   .. \" follow the header\\n\")\n\
                 \x20 os.exit(1)\n\
                 end\n\
                 zd_body = string.sub(zd_body, 1, #zd_body - zd_declared)\n",
            );
        } else {
            out.push_str("local zd_body = zd_input\n");
        }
        let _ = writeln!(
            out,
            "local zd_reader = {RUNTIME}.reader_new(zd_body, {order}, {max_align}, {}, {})",
            quoted(key.pid_flags.as_str()),
            key.short_pid_range.highest_short_id()
        );
        let _ = write!(
            out,
            "-- The top-level type is not a member of anything, so no\n\
             -- `@try_construct` governs it and §7.2.2.7's default stands.\n\
             local zd_ok, zd_value = pcall({CODECS}.{decoder}, zd_reader, \"discard\")\n\
             if not zd_ok then\n\
             \x20 io.stderr:write(\"wireprobe-zerodds-lua: \" .. tostring(zd_value) .. \"\\n\")\n\
             \x20 os.exit(1)\n\
             end\n\
             -- A refusal about the *type*, which the reader records rather than\n\
             -- raising: the generated decoder walks to the end of the sample so\n\
             -- that one test here decides what the whole read was.\n\
             if {RUNTIME}.reader_status(zd_reader) == \"discarded\" then\n\
             \x20 -- Not a failed decode: §7.2.2.7's receiver drops the object and\n\
             \x20 -- carries on. Exit code 3, the Rust probe's.\n\
             \x20 io.stderr:write(\"wireprobe-zerodds-lua: at \"\n\
             \x20   .. {RUNTIME}.reader_position(zd_reader) .. \": discarded\\n\")\n\
             \x20 os.exit(3)\n\
             end\n\
             if {RUNTIME}.reader_status(zd_reader) ~= \"ok\" then\n\
             \x20 io.stderr:write(\"wireprobe-zerodds-lua: at \"\n\
             \x20   .. {RUNTIME}.reader_position(zd_reader) .. \": \"\n\
             \x20   .. {RUNTIME}.reader_status(zd_reader) .. \"\\n\")\n\
             \x20 os.exit(1)\n\
             end\n\
             -- Nothing may be left over that is not padding: a decoder that\n\
             -- stopped short and said nothing is the silent-drop shape.\n\
             local zd_left = {RUNTIME}.reader_remaining(zd_reader)\n\
             local zd_pad = ({max_align} - {RUNTIME}.reader_position(zd_reader) % {max_align}) \
             % {max_align}\n\
             if zd_left > zd_pad then\n\
             \x20 io.stderr:write(\"wireprobe-zerodds-lua: \" .. zd_left\n\
             \x20   .. \" byte(s) left at offset \" .. {RUNTIME}.reader_position(zd_reader)\n\
             \x20   .. \", of which at most \" .. zd_pad .. \" could be padding\\n\")\n\
             \x20 os.exit(1)\n\
             end\n\
             -- Plain JSON in the dialect crates/wire-probe/src/lib.rs documents\n\
             -- and not canonical form: the caller canonicalises in Rust.\n\
             local zd_out = assert(io.open({}, \"w\"))\n\
             zd_out:write({CODECS}.{}(zd_value, \"sample\"))\n\
             zd_out:write(\"\\n\")\n\
             zd_out:close()\n",
            quoted(&values_out.to_string_lossy()),
            dump::dumper_name(ast, root)
        );
        return Ok(out);
    }

    let sample = sample.ok_or_else(|| Refusal::Gap("--values is required to write".to_string()))?;
    let filled = fill::fill(ast, root, sample).map_err(Refusal::Gap)?;
    let encoder = codec_name::<Lua>(ast, root, Purpose::Data);
    let headers = key.pl_cdr1_headers;
    let flag = |value: bool| if value { "true" } else { "false" };

    let _ = write!(out, "\nlocal zd_sample = {filled}\n\n");
    // The framing arguments are named table keys, not positions: which
    // bits ride on a parameter id is a measured divergence and the
    // runtime's `pid_flags_at` is where this column keeps its answer.
    let _ = write!(
        out,
        "local zd_writer = {RUNTIME}.writer_new({{\n\
         \x20 endianness = {order},\n\
         \x20 max_align = {max_align},\n\
         \x20 pid_flags = {},\n\
         \x20 highest_short_id = {},\n\
         \x20 pad_declared_length = {},\n\
         \x20 pad_declared_optional_length = {},\n\
         \x20 member_must_understand = {},\n\
         \x20 pad_after_value = {},\n\
         \x20 short_length_codes = {},\n\
         }})\n",
        quoted(key.pid_flags.as_str()),
        key.short_pid_range.highest_short_id(),
        flag(headers.pads_declared_length()),
        flag(headers.pads_declared_optional_length()),
        flag(key.member_pid_carries_must_understand()),
        flag(headers.pads_after_value()),
        flag(key.length_codes.short_code_where_it_fits())
    );
    let _ = write!(
        out,
        "-- A value the type cannot hold is the sample's refusal and not the\n\
         -- harness's: exit 1, the Rust probe's answer for the same sample.\n\
         local zd_ok, zd_error = pcall({CODECS}.{encoder}, zd_writer, zd_sample)\n\
         if not zd_ok then\n\
         \x20 io.stderr:write(\"wireprobe-zerodds-lua: \" .. tostring(zd_error) .. \"\\n\")\n\
         \x20 os.exit(1)\n\
         end\n\
         local zd_body = {RUNTIME}.finish(zd_writer)\n"
    );
    if args.encapsulation {
        let header = encapsulation_header(args.encoding, args.endianness);
        let _ = writeln!(
            out,
            "-- The encapsulation identifier §7.6.3.1.2 gives this encoding and\n\
             -- byte order; the options field is zero here.\n\
             io.stdout:write(string.char({}, {}, {}, {}))",
            header[0], header[1], header[2], header[3]
        );
    }
    out.push_str(
        "-- A Lua string is a run of octets and `write` passes them through;\n\
         -- on a platform that translates line endings the probe would need a\n\
         -- binary stream, which POSIX stdout already is.\n\
         io.stdout:write(zd_body)\n",
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
/// [`layout_key`] carries. Two answers do not: whether a `@hashid`
/// member advances the sequential counter the next unannotated member is
/// numbered from, and which spelling of an escaped name the hash reads.
/// Both are settled in `zerodds_idl_lower::complete`, before any plan
/// exists, so a `LayoutKey` cannot carry them and a generator that
/// lowers with `Options::default()` writes the specification's member
/// ids under every dialect. `zerodds_wire_probe::model_from_idl_in_dialect`
/// is the same reading on the Rust side; `Dialect` holds the vendor
/// facts.
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
