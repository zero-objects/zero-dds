// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-zerodds` — one IDL type, one JSON value, the bytes.
//!
//! The ZeroDDS member of the probe set defined in
//! `docs/specs/zerodds-interop-suite-1.0.md`. The Cyclone, RTI and Fast
//! DDS probes take the same options, so a case runs through all four by
//! substituting the program name.
//!
//! ```text
//! wireprobe-zerodds --idl robot.idl --type common::Robot \
//!                   --values robot.json --encoding xcdr2 --endian le \
//!                   --out robot.zerodds.xcdr2.le.bin
//! ```
//!
//! ## The return direction
//!
//! `--in` reads bytes instead of writing them, and `--values-out` says
//! where the values go. The point of the two together is the pairwise
//! run of `docs/specs/zerodds-interop-suite-1.0.md` §4, P2: A's bytes
//! into B, and B's values against the sample that went into A. A round
//! trip through one implementation passes a decoder that is wrong in the
//! same way its own encoder is wrong; a round trip through two does not.
//!
//! ```text
//! wireprobe-zerodds --idl robot.idl --type common::Robot \
//!                   --in robot.cyclone.xcdr2.le.bin --encoding xcdr2 \
//!                   --endian le --values-out robot.read.json
//! ```
//!
//! Arguments are parsed by hand. A probe with ten flags does not need a
//! dependency, and an unrecognised flag has to be an error rather than
//! something a parser generator decides to tolerate.
//!
//! ## The dialect and its flags
//!
//! `--dialect` is a preset — one peer's answers to every wire axis at
//! once — and every other axis flag (`--length-codes`, `--pid-flags`,
//! `--map-length`, ...) overrides one of that preset's answers. Both are
//! `zerodds_ast_layout::axes`'s: the flag names, their parsers, the help
//! text and the dialect-first-then-flags order are the same table
//! `zerodds-idlc generate --dialect` and `zerodds-build`'s
//! `Config::dialect` read, so a value this probe accepts is a value
//! those two accept, under the same name and the same spelling.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

use std::path::PathBuf;
use zerodds_ast_layout::{
    AXES, AxisFlag, Dialect, DialectSettings, Encoding, Endianness, Purpose, dialect_help,
};
use zerodds_wire_probe::ProbeError;
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::{Emit, MapLengthPolicy, ProbeRequest};

/// The usage text, built from the shared axis table.
const USAGE: &str = concat!(
    "wireprobe-zerodds — one IDL type and one JSON value, as wire bytes\n\
    \n\
    \x20\x20\x20\x20--idl <file>            the IDL source (required, repeatable)\n\
    \x20\x20\x20\x20--type <ScopedName>     the top-level type (required)\n\
    \x20\x20\x20\x20--values <file>         the sample, as JSON (required, unless --in)\n\
    \x20\x20\x20\x20--in <file>             bytes to read back instead of writing\n\
    \x20\x20\x20\x20--values-out <file>     where the values read go (default: stdout)\n\
    \x20\x20\x20\x20--encoding <name>       xcdr1 | xcdr2 | plain-cdr1 | pl-cdr1 |\n\
    \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20  plain-cdr2 | delimited-cdr | pl-cdr2\n\
    \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20  (default: xcdr2, which is plain-cdr2)\n\
    \x20\x20\x20\x20--endian le|be          (default: le)\n\
    \x20\x20\x20\x20--emit <form>           payload | payload+encapsulation | keyhash\n\
    \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20  (default: payload)\n\
    \x20\x20\x20\x20--dialect <name>        ",
    // dialect_help() is a runtime string, included below in the dynamic portion
);

/// The command line: every axis flag folded into `overrides`, in the
/// order the caller gave them, rather than one field per axis.
///
/// `zerodds_ast_layout::axes` is what turns `dialect` and `overrides`
/// into the settings [`run`] plans and lowers under; this struct only
/// keeps what the table cannot — the flags that are not axes at all
/// (`--idl`, `--type`, `--encoding`, ...) — and the raw `(axis, value)`
/// pairs the table has not seen yet.
#[derive(Debug)]
struct Args {
    idl: Vec<PathBuf>,
    type_name: String,
    values: Option<PathBuf>,
    input: Option<PathBuf>,
    encoding: Encoding,
    endianness: Endianness,
    emit: Emit,
    dialect: Dialect,
    /// `--<axis> <value>`, in the order given. Applied after the
    /// dialect: a dialect is a preset and these override one answer of
    /// it at a time.
    overrides: Vec<(AxisFlag, String)>,
    out: Option<PathBuf>,
    values_out: Option<PathBuf>,
}

/// Find an axis by its CLI flag name.
fn find_axis(flag: &str) -> Option<AxisFlag> {
    AXES.into_iter().find(|axis| axis.flag_name() == flag)
}

/// The exit code a discarded sample gets.
///
/// Apart from 1, which is a probe that could not do the job, and from 2,
/// which is a command line it did not understand. §7.2.2.7's DISCARD is
/// neither: the probe worked, the stream was well formed, and this type
/// cannot hold that sample. A harness that has to tell "the reader
/// dropped it" from "the reader broke" reads this.
const DISCARDED: i32 = 3;

fn main() {
    let args = match parse(std::env::args().skip(1).collect()) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            // The usage text is built at runtime because dialect_help()
            // and the axis table are not const.
            eprintln!("{USAGE}");
            eprintln!("                             {}", dialect_help());
            for axis in AXES {
                eprintln!(
                    "    {:<30} {}",
                    format!("{} <name>", axis.flag_name()),
                    axis.help_values()
                );
            }
            eprintln!("    --out <file>            where the bytes go (default: stdout, as hex)");
            eprintln!();
            eprintln!("`--encoding xcdr1` and `xcdr2` name the plain form. A `@mutable` type");
            eprintln!("needs `pl-cdr1` or `pl-cdr2`, and an `@appendable` one `delimited-cdr`,");
            eprintln!("because the encoding is what the extensibility means on the wire.");
            std::process::exit(2);
        }
    };
    if let Err(message) = run(&args) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), String> {
    let mut ast = None;
    for path in &args.idl {
        let source = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let model = model_from_idl_in_dialect(&source, &path.display().to_string(), args.dialect)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if ast.is_some() {
            return Err(
                "more than one --idl is not supported yet: the second file needs \
                        lowering into the first model, not a model of its own"
                    .to_string(),
            );
        }
        ast = Some(model);
    }
    let ast = ast.ok_or("no --idl")?;

    // The dialect first, then every axis flag overriding one of its
    // answers, in the order the caller gave them — the one place this
    // rule is written, shared with `zerodds-idlc generate --dialect`.
    let overrides: Vec<(AxisFlag, &str)> = args
        .overrides
        .iter()
        .map(|(axis, value)| (*axis, value.as_str()))
        .collect();
    let settings = zerodds_ast_layout::dialect_settings_from_overrides(
        args.dialect,
        args.encoding,
        args.endianness,
        Purpose::Data,
        &overrides,
    )?;
    let map_length = if settings.map_length_with_delimiter {
        MapLengthPolicy::SequenceEquivalent
    } else {
        MapLengthPolicy::Rule15Omits
    };
    let mut request = ProbeRequest::from_key(&args.type_name, settings.key, map_length);
    request.emit = args.emit;
    let request = request;

    if let Some(input) = &args.input {
        let bytes =
            std::fs::read(input).map_err(|error| format!("{}: {error}", input.display()))?;
        let values = match decode(&ast, &request, &bytes) {
            Ok(values) => values,
            Err(error @ ProbeError::Discarded { .. }) => {
                eprintln!("{error}");
                std::process::exit(DISCARDED);
            }
            Err(error) => return Err(format!("{error}")),
        };
        let rendered = text(&values);
        return match &args.values_out {
            Some(path) => std::fs::write(path, format!("{rendered}\n"))
                .map_err(|error| format!("{}: {error}", path.display())),
            None => {
                println!("{rendered}");
                Ok(())
            }
        };
    }

    let values = args.values.as_ref().ok_or("--values is required")?;
    let source = std::fs::read_to_string(values)
        .map_err(|error| format!("{}: {error}", values.display()))?;
    let sample: serde_json::Value =
        serde_json::from_str(&source).map_err(|error| format!("{}: {error}", values.display()))?;

    if let Some(path) = &args.values_out {
        let type_ = find_type(&ast, &args.type_name)?;
        let canonical = canonicalise(&ast, type_, &sample, &args.type_name)
            .map_err(|error| format!("{error}"))?;
        std::fs::write(path, format!("{}\n", text(&canonical)))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if args.out.is_none() {
            return Ok(());
        }
    }

    let bytes = encode(&ast, &request, &sample).map_err(|error| format!("{error}"))?;

    match &args.out {
        Some(path) => {
            std::fs::write(path, &bytes).map_err(|error| format!("{}: {error}", path.display()))
        }
        None => {
            println!("{}", hex(&bytes));
            Ok(())
        }
    }
}

fn find_type(
    ast: &zerodds_ast::Ast,
    name: &str,
) -> Result<zerodds_ast_core::Id<zerodds_ast::TypeNode>, String> {
    let segments: Vec<String> = name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .ok_or_else(|| format!("`{name}` is not in the model"))
}

fn parse(argv: Vec<String>) -> Result<Args, String> {
    let mut idl = Vec::new();
    let mut type_name = None;
    let mut values = None;
    let mut input = None;
    let mut values_out = None;
    let mut encoding = Encoding::PlainCdr2;
    let mut endianness = Endianness::Little;
    let mut emit = Emit::Payload;
    let mut dialect = Dialect::default();
    let mut overrides: Vec<(AxisFlag, String)> = Vec::new();
    let mut out = None;

    let mut rest = argv.into_iter();
    while let Some(flag) = rest.next() {
        let mut next = || rest.next().ok_or_else(|| format!("`{flag}` takes a value"));
        match flag.as_str() {
            "--idl" => idl.push(PathBuf::from(next()?)),
            "--type" => type_name = Some(next()?),
            "--values" => values = Some(PathBuf::from(next()?)),
            "--in" => input = Some(PathBuf::from(next()?)),
            "--values-out" => values_out = Some(PathBuf::from(next()?)),
            "--out" => out = Some(PathBuf::from(next()?)),
            "--encoding" => {
                let name = next()?;
                encoding =
                    parse_encoding(&name).ok_or_else(|| format!("`{name}` is not an encoding"))?;
            }
            "--endian" => {
                let name = next()?;
                endianness = match name.as_str() {
                    "le" | "little" => Endianness::Little,
                    "be" | "big" => Endianness::Big,
                    _ => return Err(format!("`{name}` is not a byte order")),
                };
            }
            "--emit" => {
                let name = next()?;
                emit = Emit::parse(&name).ok_or_else(|| format!("`{name}` is not an emit form"))?;
            }
            "--dialect" => {
                let name = next()?;
                dialect =
                    Dialect::parse(&name).ok_or_else(|| format!("`{name}` is not a dialect"))?;
            }
            "--help" | "-h" => {
                eprintln!("{USAGE}");
                eprintln!("                             {}", dialect_help());
                for axis in AXES {
                    eprintln!(
                        "    {:<30} {}",
                        format!("{} <name>", axis.flag_name()),
                        axis.help_values()
                    );
                }
                eprintln!(
                    "    --out <file>            where the bytes go (default: stdout, as hex)"
                );
                eprintln!();
                eprintln!("`--encoding xcdr1` and `xcdr2` name the plain form. A `@mutable` type");
                eprintln!(
                    "needs `pl-cdr1` or `pl-cdr2`, and an `@appendable` one `delimited-cdr`,"
                );
                eprintln!("because the encoding is what the extensibility means on the wire.");
                std::process::exit(0);
            }
            name => {
                let Some(axis) = find_axis(name) else {
                    return Err(format!("`{name}` is not an option"));
                };
                let value = next()?;
                // Checked here, against a throwaway default-dialect
                // settings value, so a typo in the axis's own value is
                // reported at the point it was given rather than after
                // every other flag on the line has been read. Applied
                // for real — against the dialect this run actually
                // asked for — in `run`.
                let probe_settings = DialectSettings::in_dialect(
                    Dialect::default(),
                    encoding,
                    endianness,
                    Purpose::Data,
                );
                let _ = axis.parse_and_apply(probe_settings, &value)?;
                overrides.push((axis, value));
            }
        }
    }

    if idl.is_empty() {
        return Err("--idl is required".to_string());
    }
    if input.is_some() && values.is_some() {
        return Err("--in reads bytes and --values writes them; pick one".to_string());
    }
    if input.is_none() && values.is_none() {
        return Err("--values is required, or --in to read bytes back".to_string());
    }
    Ok(Args {
        idl,
        type_name: type_name.ok_or("--type is required")?,
        values,
        input,
        encoding,
        endianness,
        emit,
        dialect,
        overrides,
        out,
        values_out,
    })
}

fn parse_encoding(name: &str) -> Option<Encoding> {
    match name {
        "xcdr1" | "plain-cdr1" => Some(Encoding::PlainCdr1),
        "xcdr2" | "plain-cdr2" => Some(Encoding::PlainCdr2),
        "pl-cdr1" => Some(Encoding::PlCdr1),
        "delimited-cdr" => Some(Encoding::DelimitedCdr),
        "pl-cdr2" => Some(Encoding::PlCdr2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn the_required_options_are_required() {
        assert!(parse(argv(&["--type", "Point"])).is_err());
        assert!(parse(argv(&["--idl", "a.idl", "--type", "Point"])).is_err());
    }

    #[test]
    fn an_unknown_option_is_refused_rather_than_ignored() {
        let error = parse(argv(&[
            "--idl",
            "a.idl",
            "--type",
            "Point",
            "--values",
            "v.json",
            "--endianness",
            "le",
        ]))
        .expect_err("a typo in a wire-affecting flag");
        assert!(error.contains("--endianness"), "{error}");
    }

    #[test]
    fn the_defaults_are_xcdr2_little_endian_payload() {
        let args = parse(argv(&[
            "--idl", "a.idl", "--type", "Point", "--values", "v.json",
        ]))
        .expect("the minimum");
        assert_eq!(args.encoding, Encoding::PlainCdr2);
        assert_eq!(args.endianness, Endianness::Little);
        assert_eq!(args.emit, Emit::Payload);
        assert!(args.overrides.is_empty());
        assert!(args.dialect.is_default());
    }

    #[test]
    fn every_encoding_has_a_name_and_a_typo_is_refused() {
        for encoding in Encoding::ALL {
            let name = match encoding {
                Encoding::PlainCdr1 => "plain-cdr1",
                Encoding::PlCdr1 => "pl-cdr1",
                Encoding::PlainCdr2 => "plain-cdr2",
                Encoding::DelimitedCdr => "delimited-cdr",
                Encoding::PlCdr2 => "pl-cdr2",
            };
            assert_eq!(parse_encoding(name), Some(encoding));
        }
        assert_eq!(parse_encoding("xcdr2"), Some(Encoding::PlainCdr2));
        assert_eq!(parse_encoding("XCDR2"), None);
    }

    #[test]
    fn every_axis_flag_is_parseable_by_the_table() {
        // Each axis flag name in AXES must be matched by find_axis.
        for axis in AXES {
            assert!(
                find_axis(axis.flag_name()).is_some(),
                "axis {axis:?} not found"
            );
        }
    }

    #[test]
    fn a_map_length_flag_is_parsed_and_overrides_the_dialect() {
        // `--map-length` used to be a flag `find_axis` never saw: it was
        // parsed by a dedicated match arm in `parse` and applied by hand
        // in `run`, outside the shared table. Now it is one more axis in
        // `AXES`, so this exercises the same path every other flag does.
        let args = parse(argv(&[
            "--idl",
            "a.idl",
            "--type",
            "Point",
            "--values",
            "v.json",
            "--map-length",
            "rule-15-omits",
        ]))
        .expect("a known map-length value");
        assert_eq!(
            args.overrides,
            vec![(AxisFlag::MapLength, "rule-15-omits".to_string())]
        );
    }

    #[test]
    fn an_unknown_map_length_value_is_refused_at_parse_time() {
        let error = parse(argv(&[
            "--idl",
            "a.idl",
            "--type",
            "Point",
            "--values",
            "v.json",
            "--map-length",
            "nonsense",
        ]))
        .expect_err("an unknown map-length value");
        assert!(error.contains("nonsense"), "{error}");
        assert!(error.contains("map-length"), "{error}");
    }

    #[test]
    fn two_axis_flags_are_both_kept_in_the_order_given() {
        let args = parse(argv(&[
            "--idl",
            "a.idl",
            "--type",
            "Point",
            "--values",
            "v.json",
            "--pid-flags",
            "bare",
            "--length-codes",
            "compact",
        ]))
        .expect("two known axis flags");
        assert_eq!(
            args.overrides,
            vec![
                (AxisFlag::PidFlags, "bare".to_string()),
                (AxisFlag::LengthCodes, "compact".to_string()),
            ]
        );
    }
}
