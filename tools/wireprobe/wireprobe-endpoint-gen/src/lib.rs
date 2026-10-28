// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-endpoint-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-endpoint-gen` — the program each hand-written endpoint SDK
//! compiles to write one corpus case.
//!
//! `endpoints/` holds twenty-one language bindings. Six of them write
//! their own octets from a hand-written writer — `endpoints/zig`,
//! `endpoints/nim`, `endpoints/d`, `endpoints/ada`, `endpoints/ada-83`
//! and `endpoints/ada-native` — and each carries one test: a byte
//! identity against two Rust goldens, one struct in two byte orders.
//! The seven generated codec columns are measured against 376 corpus
//! cases in ten configurations. This closes the distance: one IDL type
//! and one JSON sample in, one program in the binding's own language
//! out, and the octets it writes compared against
//! `tests/interop-results/<case>/zerodds-rust/<encoding>.<endian>.bin`
//! by the same ledger machinery every other column uses.
//!
//! ## What it does not do
//!
//! Decide a wire question. The walk asks `zerodds-ast-layout` where a
//! header goes and what a member's id is — both properties of the type,
//! which a hand-written program reads from the same IDL — and asks the
//! sample for values. Alignment, padding, byte order, length words,
//! DHEADER bodies and EMHEADER bit fields all come out of the binding.
//!
//! ## The three numbers
//!
//! A case a binding's write surface cannot express is not a failure of
//! the binding; it is the extent of it. `crate::surface` decides reach
//! before a byte is written, and the generator exits 3 with the reason
//! on standard error, so a sweep counts reach, agreement and divergence
//! apart. `docs/specs/zerodds-verdicts-1.0.md` gives the codes.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::exit)]

pub mod ops;
pub mod render;
pub mod surface;
pub mod walk;

use std::path::PathBuf;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};

const USAGE: &str = "\
wireprobe-endpoint-gen — the program one hand-written endpoint SDK compiles

    --lang <name>           ada | ada-83 | ada-native | d | elixir | go |
                            java | julia | kotlin | lua | nim | node |
                            ocaml | fsharp | python | swift | zig
    --idl <file>            the IDL source (required)
    --type <ScopedName>     the top-level type (required)
    --values <file>         the sample, as JSON (required)
    --encoding <name>       xcdr1 | xcdr2 | plain-cdr1 | pl-cdr1 |
                            plain-cdr2 | delimited-cdr | pl-cdr2
    --endian le|be
    --numbers bits|natural  which spelling of an integer argument the
                            program hands the binding. `bits` (the
                            default) is the two's complement pattern of
                            the write's width, which is the argument every
                            renderer emitted before this option existed.
                            `natural` is the IDL value, which is what a
                            caller of that binding writes. The difference
                            between two sweeps is the whole of
                            `tools/wireprobe/endpoint-number-surface`.
    --out-dir <dir>         where the generated source goes (required)

Exits 0 when the source was written, 2 on a bad command line, 3 when the
binding's write surface has no form for this case (the reason goes to
standard error), and 7 when the model or the sample could not be read.
";

/// What the generator was asked for.
#[derive(Debug, Clone)]
pub struct Args {
    /// Which binding.
    pub lang: surface::Language,
    /// The IDL source.
    pub idl: PathBuf,
    /// The top-level type's scoped name.
    pub type_name: String,
    /// The sample.
    pub values: PathBuf,
    /// The encoding.
    pub encoding: Encoding,
    /// The byte order.
    pub endianness: Endianness,
    /// Which spelling of an integer argument.
    pub form: ops::Form,
    /// Where the source goes.
    pub out_dir: PathBuf,
}

/// Parse a command line.
///
/// # Errors
/// A message naming the option, or the usage text for `--help`.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. What the
/// body calls is `Encoding::parse`, `Dialect::parse` and their siblings,
/// inherent functions on other types; `dds_bounded_recursion` compares only
/// the last segment of a call path, so they read as self-calls.
pub fn parse(argv: Vec<String>) -> Result<Args, String> {
    let mut lang = None;
    let mut idl = None;
    let mut type_name = None;
    let mut values = None;
    let mut encoding = Encoding::PlainCdr2;
    let mut endianness = Endianness::Little;
    let mut form = ops::Form::Bits;
    let mut out_dir = None;

    let mut rest = argv.into_iter().skip(1);
    while let Some(flag) = rest.next() {
        let mut take = || {
            rest.next()
                .ok_or_else(|| format!("{flag}: the option takes a value"))
        };
        match flag.as_str() {
            "--help" | "-h" => return Err(USAGE.to_string()),
            "--lang" => {
                let name = take()?;
                lang = Some(
                    surface::Language::parse(&name)
                        .ok_or_else(|| format!("--lang: `{name}` is not one of them"))?,
                );
            }
            "--idl" => idl = Some(PathBuf::from(take()?)),
            "--type" => type_name = Some(take()?),
            "--values" => values = Some(PathBuf::from(take()?)),
            "--out-dir" => out_dir = Some(PathBuf::from(take()?)),
            "--numbers" => form = ops::Form::parse(&take()?)?,
            "--encoding" => {
                let name = take()?;
                encoding = match name.as_str() {
                    "xcdr1" | "plain-cdr1" => Encoding::PlainCdr1,
                    "pl-cdr1" => Encoding::PlCdr1,
                    "xcdr2" | "plain-cdr2" => Encoding::PlainCdr2,
                    "delimited-cdr" => Encoding::DelimitedCdr,
                    "pl-cdr2" => Encoding::PlCdr2,
                    other => return Err(format!("--encoding: `{other}` is not one of them")),
                };
            }
            "--endian" => {
                endianness = match take()?.as_str() {
                    "le" => Endianness::Little,
                    "be" => Endianness::Big,
                    other => return Err(format!("--endian: `{other}` is not le or be")),
                };
            }
            other => return Err(format!("`{other}` is not an option")),
        }
    }

    Ok(Args {
        lang: lang.ok_or("--lang is required")?,
        idl: idl.ok_or("--idl is required")?,
        type_name: type_name.ok_or("--type is required")?,
        values: values.ok_or("--values is required")?,
        encoding,
        endianness,
        form,
        out_dir: out_dir.ok_or("--out-dir is required")?,
    })
}

/// What the generator produced, or why it did not.
pub enum Outcome {
    /// The source, and the file name it goes under.
    Source {
        /// The file name.
        name: &'static str,
        /// The text.
        text: String,
    },
    /// The binding's write surface has no form for this case.
    OutOfSurface(String),
    /// The model or the sample could not be read.
    Broken(String),
}

/// Generate the source for one case.
#[must_use]
pub fn generate(args: &Args) -> Outcome {
    let text = match std::fs::read_to_string(&args.idl) {
        Ok(text) => text,
        Err(error) => return Outcome::Broken(format!("{}: {error}", args.idl.display())),
    };
    let ast = match model_from_idl(&text, &args.idl.to_string_lossy()) {
        Ok(ast) => ast,
        Err(message) => return Outcome::Broken(message),
    };
    let root = match find_type(&ast, &args.type_name) {
        Ok(root) => root,
        Err(message) => return Outcome::Broken(message),
    };
    let sample = match std::fs::read_to_string(&args.values) {
        Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => value,
            Err(error) => return Outcome::Broken(format!("{}: {error}", args.values.display())),
        },
        Err(error) => return Outcome::Broken(format!("{}: {error}", args.values.display())),
    };

    let key = LayoutKey::data(args.encoding, args.endianness);
    let program = match walk::program(&ast, root, &sample, key, args.lang.surface()) {
        Ok(program) => program,
        Err(walk::Refusal::OutOfSurface(reason)) => return Outcome::OutOfSurface(reason.0),
        Err(walk::Refusal::Generator(message)) => return Outcome::Broken(message),
    };
    match render::render(
        args.lang,
        &program,
        args.endianness == Endianness::Big,
        args.form,
    ) {
        Ok(text) => Outcome::Source {
            name: render::file_name(args.lang),
            text,
        },
        Err(message) => Outcome::Broken(message),
    }
}

/// The command, for `main`.
pub fn cli_main() {
    let args = match parse(std::env::args().collect()) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    match generate(&args) {
        Outcome::Source { name, text } => {
            if let Err(error) = std::fs::create_dir_all(&args.out_dir) {
                eprintln!(
                    "wireprobe-endpoint-gen: {}: {error}",
                    args.out_dir.display()
                );
                std::process::exit(7);
            }
            let path = args.out_dir.join(name);
            if let Err(error) = std::fs::write(&path, text) {
                eprintln!("wireprobe-endpoint-gen: {}: {error}", path.display());
                std::process::exit(7);
            }
        }
        Outcome::OutOfSurface(reason) => {
            eprintln!(
                "wireprobe-endpoint-gen: {} has no form for this case: {reason}",
                args.lang.as_str()
            );
            std::process::exit(3);
        }
        Outcome::Broken(message) => {
            eprintln!("wireprobe-endpoint-gen: {message}");
            std::process::exit(7);
        }
    }
}

fn model_from_idl(source: &str, path: &str) -> Result<Ast, String> {
    let parsed = zerodds_idl_parse::parse(source);
    if !parsed.diagnostics().is_empty() {
        return Err(format!("{path}: {:?}", parsed.diagnostics()));
    }
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), path);
    if !lowered.diagnostics.is_empty() {
        return Err(format!("{path}: {:?}", lowered.diagnostics));
    }
    let mut ast = lowered.ast;
    ast.resolve_all_by_name()
        .map_err(|error| format!("{path}: {error}"))?;
    Ok(ast)
}

/// The type the corpus's `type` file names.
///
/// The scoped lookup first, and the simple name only when that finds
/// nothing. The corpus writes `Sample` for a type in the root module and
/// a scoped name where it has one, and a lookup that only did the second
/// takes the *first* declaration whose simple name matches — which is
/// the wrong one wherever a model declares the same simple name twice.
/// `tools/wireprobe/wireprobe-java-corpus` carries the same note.
///
/// The two cases that motivated it, `recursive-mutual` (`m::A`) and
/// `resolve-forward-declared` (`m::Holder`), resolve to the same type
/// either way in this corpus, and every one of their twenty cells is
/// unchanged. It is the models this corpus does not yet hold that the
/// scoped lookup is for.
fn find_type(ast: &Ast, name: &str) -> Result<Id<TypeNode>, String> {
    let segments: Vec<String> = name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    if let Some(id) = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
    {
        return Ok(id);
    }
    let simple = name.rsplit("::").next().unwrap_or(name);
    ast.types()
        .find(|(_, node)| node.name.as_str() == simple)
        .map(|(id, _)| id)
        .ok_or_else(|| format!("`{name}` is not a type in this model"))
}
