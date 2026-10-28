// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! One DDS-XTypes 1.3 probe in, three measurements out.
//!
//! Reads `id<TAB>op<TAB>arg` on stdin and writes
//! `id<TAB>decode<TAB>encode<TAB>rule` on stdout, one line per probe.
//! Nothing here decides whether a result is right: the specification's
//! answer lives in `cases.tsv` and in the coverage page, and the
//! recorded state of the gap lives in `expected.tsv`. This program only
//! reports what the tree did.
//!
//! The three columns, per op:
//!
//! | op | `decode` | `encode` | `rule` |
//! |---|---|---|---|
//! | `ti` | the octets parse as a `TypeIdentifier` | re-encoding them gives the same octets back | §7.2.4.1: the identifier is assignable from itself |
//! | `to` | the octets parse as a `TypeObject` | re-encoding them gives the same octets back | §7.6.3: the object becomes a model (`ModelFromTypeObjects`) |
//! | `idl` | the IDL parses, lowers and resolves | the `MinimalTypeObject` survives an encode/decode/encode round trip | the `CompleteTypeObject` builds |
//! | `assign` | both types build from one IDL text | — | §7.2.4.4: `is_assignable(T2, T1)` said yes |
//! | `datarep` | — | — | §7.6.3.1 Table 59: the negotiation accepted |
//! | `encaps` | — | — | §7.6.3.1.2 Table 60: the pairing is in the matrix |
//! | `tl` | the octets parse as the named TypeLookup type | re-encoding gives the same octets back | — |
//! | `todh` | the type builds from IDL | the encoded `TypeObject` opens with a DHEADER covering the rest (§7.4.2 for `@extensibility(APPENDABLE)`) | — |

use std::io::{self, BufRead, Write};

use zerodds_ast::Ast;
use zerodds_ast_core::Id;
use zerodds_types::assignability::{AssignabilityConfig, is_assignable};
use zerodds_types::dynamic::to_ast::ModelFromTypeObjects;
use zerodds_types::from_ast::{
    Form, annotation_type_object, complete_type_object, minimal_type_object, register_all,
    type_identifier,
};
use zerodds_types::qos::{
    DataRepresentationId, ExtensibilityForRepr, RepresentationNegotiation,
    check_data_repr_extensibility, negotiate_representation,
};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::TypeIdentifier;
use zerodds_types::type_lookup::{
    GetTypeDependenciesReply, GetTypeDependenciesRequest, GetTypeDependenciesResult, GetTypesReply,
    GetTypesRequest, GetTypesResult, TypeLookupCall, TypeLookupReplyMessage, TypeLookupRequest,
    TypeLookupReturn,
};
use zerodds_types::type_object::TypeObject;

/// What one column of one probe came to.
///
/// `Skip` is not a pass and not a failure: the column does not apply to
/// this op, and the scan prints `-`. A column that *could* have been
/// measured and was not would be a lie in either direction.
enum Cell {
    Ok,
    Err,
    Skip,
}

impl Cell {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Err => "err",
            Self::Skip => "-",
        }
    }

    fn from(ok: bool) -> Self {
        if ok { Self::Ok } else { Self::Err }
    }
}

struct Measured {
    decode: Cell,
    encode: Cell,
    rule: Cell,
    /// The first diagnostic, for `--explain`. Never compared; a scan
    /// that compared message text would break on every reworded error.
    note: String,
}

impl Measured {
    fn refused(note: impl Into<String>) -> Self {
        Self {
            decode: Cell::Err,
            encode: Cell::Skip,
            rule: Cell::Skip,
            note: note.into(),
        }
    }
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let clean: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .collect();
    if clean.len() % 2 != 0 {
        return None;
    }
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).ok())
        .collect()
}

/// A model from one IDL text, or the first error that stopped it.
fn model(source: &str) -> Result<Ast, String> {
    let parsed = zerodds_idl_parse::parse(source);
    if let Some(first) = parsed.diagnostics().iter().find(|d| d.is_error()) {
        return Err(format!("{first:?}"));
    }
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "probe.idl").ast;
    let report = ast.resolve_all_by_name().map_err(|e| e.to_string())?;
    if !report.is_clean() {
        return Err(format!("unresolved: {:?}", report.broken));
    }
    Ok(ast)
}

fn named(ast: &Ast, name: &str) -> Result<Id<zerodds_ast::TypeNode>, String> {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .ok_or_else(|| format!("no type named `{name}`"))
}

fn probe_ti(arg: &str) -> Measured {
    let Some(bytes) = unhex(arg) else {
        return Measured::refused("the probe's argument is not hex");
    };
    match TypeIdentifier::from_bytes_le(&bytes) {
        Err(e) => Measured::refused(format!("{e:?}")),
        Ok(id) => {
            let round = id.to_bytes_le();
            let encode = Cell::from(round.as_ref().is_ok_and(|b| *b == bytes));
            let registry = TypeRegistry::new();
            let cfg = AssignabilityConfig::default();
            let rule = Cell::from(is_assignable(&id, &id, &registry, &cfg).is_yes());
            let note = match &round {
                Ok(b) if *b != bytes => format!("re-encoded {} octet(s)", b.len()),
                Err(e) => format!("{e:?}"),
                _ => String::new(),
            };
            Measured {
                decode: Cell::Ok,
                encode,
                rule,
                note,
            }
        }
    }
}

fn probe_to(arg: &str) -> Measured {
    let Some(bytes) = unhex(arg) else {
        return Measured::refused("the probe's argument is not hex");
    };
    match TypeObject::from_bytes_le(&bytes) {
        Err(e) => Measured::refused(format!("{e:?}")),
        Ok(object) => {
            let round = object.to_bytes_le();
            let encode = Cell::from(round.as_ref().is_ok_and(|b| *b == bytes));
            let registry = TypeRegistry::new();
            let mut builder = ModelFromTypeObjects::new(&registry);
            let built = builder.add_type_object(&object);
            let note = match (&round, &built) {
                (Err(e), _) => format!("{e:?}"),
                (_, Err(e)) => format!("{e:?}"),
                (Ok(b), _) if *b != bytes => format!("re-encoded {} octet(s)", b.len()),
                _ => String::new(),
            };
            Measured {
                decode: Cell::Ok,
                encode,
                rule: Cell::from(built.is_ok()),
                note,
            }
        }
    }
}

fn probe_idl(arg: &str) -> Measured {
    let Some((name, source)) = arg.split_once(";;") else {
        return Measured::refused("an `idl` probe reads `TypeName;;<idl>`");
    };
    let ast = match model(source) {
        Err(e) => return Measured::refused(e),
        Ok(ast) => ast,
    };
    // An `@annotation` declaration is not a `TypeNode` — a use of
    // `@Name` resolves to the declaration — so it is looked up in the
    // other arena. Annex B gives it a TypeObject all the same:
    // `case TK_ANNOTATION: MinimalAnnotationType annotation_type;`.
    if let Some((annotation, _)) = ast.annotation_defs().find(|(_, node)| node.name == name) {
        let minimal = annotation_type_object(&ast, annotation, Form::Minimal);
        let complete = annotation_type_object(&ast, annotation, Form::Complete);
        let encode = match &minimal {
            Err(_) => Cell::Err,
            Ok(object) => match object.to_bytes_le() {
                Err(_) => Cell::Err,
                Ok(first) => match TypeObject::from_bytes_le(&first) {
                    Err(_) => Cell::Err,
                    Ok(back) => Cell::from(back.to_bytes_le().is_ok_and(|s| s == first)),
                },
            },
        };
        let note = match (&minimal, &complete) {
            (Err(e), _) | (_, Err(e)) => format!("{e:?}"),
            _ => String::new(),
        };
        return Measured {
            decode: Cell::Ok,
            encode,
            rule: Cell::from(complete.is_ok()),
            note,
        };
    }
    let id = match named(&ast, name) {
        Err(e) => return Measured::refused(e),
        Ok(id) => id,
    };
    // The identifier first: a fully descriptive type has no TypeObject
    // behind it (§7.3.4.6), and asking for one would be the wrong
    // question rather than a gap.
    let ident = type_identifier(&ast, id, Form::Minimal);
    if let Err(e) = &ident {
        return Measured {
            decode: Cell::Ok,
            encode: Cell::Err,
            rule: Cell::Skip,
            note: format!("{e:?}"),
        };
    }
    let minimal = minimal_type_object(&ast, id);
    let encode = match &minimal {
        Err(_) => Cell::Err,
        Ok(m) => {
            let object = TypeObject::Minimal(m.clone());
            match object.to_bytes_le() {
                Err(_) => Cell::Err,
                Ok(first) => match TypeObject::from_bytes_le(&first) {
                    Err(_) => Cell::Err,
                    Ok(back) => Cell::from(back.to_bytes_le().is_ok_and(|second| second == first)),
                },
            }
        }
    };
    let complete = complete_type_object(&ast, id);
    let note = match (&minimal, &complete) {
        (Err(e), _) => format!("{e:?}"),
        (_, Err(e)) => format!("{e:?}"),
        _ => String::new(),
    };
    Measured {
        decode: Cell::Ok,
        encode,
        rule: Cell::from(complete.is_ok()),
        note,
    }
}

/// Annex B declares `union TypeObject` `@extensibility(APPENDABLE)` and
/// says the file is serialized with XCDR encoding version 2. §7.4.2
/// gives an appendable object under version 2 a DHEADER: a `UInt32`
/// holding the length of everything after it. This op builds a
/// `MinimalTypeObject` from IDL, encodes it, and reports whether that
/// header is there.
fn probe_todh(arg: &str) -> Measured {
    let Some((name, source)) = arg.split_once(";;") else {
        return Measured::refused("a `todh` probe reads `TypeName;;<idl>`");
    };
    let ast = match model(source) {
        Err(e) => return Measured::refused(e),
        Ok(ast) => ast,
    };
    let id = match named(&ast, name) {
        Err(e) => return Measured::refused(e),
        Ok(id) => id,
    };
    let object = match minimal_type_object(&ast, id) {
        Err(e) => return Measured::refused(format!("{e:?}")),
        Ok(m) => TypeObject::Minimal(m),
    };
    let bytes = match object.to_bytes_le() {
        Err(e) => return Measured::refused(format!("{e:?}")),
        Ok(b) => b,
    };
    let (header, rest) = bytes.split_at(4.min(bytes.len()));
    let announced = if header.len() == 4 {
        u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize
    } else {
        usize::MAX
    };
    let has_dheader = announced == rest.len();
    Measured {
        decode: Cell::Ok,
        encode: Cell::from(has_dheader),
        rule: Cell::Skip,
        note: format!(
            "{} octet(s), first four = {announced}, rest = {}",
            bytes.len(),
            rest.len()
        ),
    }
}

fn probe_assign(arg: &str) -> Measured {
    let parts: Vec<&str> = arg.splitn(3, ";;").collect();
    if parts.len() != 3 {
        return Measured::refused("an `assign` probe reads `T1;;T2;;<idl>`");
    }
    let (t1, t2, source) = (parts[0], parts[1], parts[2]);
    let ast = match model(source) {
        Err(e) => return Measured::refused(e),
        Ok(ast) => ast,
    };
    let mut registry = TypeRegistry::new();
    if let Err(e) = register_all(&ast, &mut registry) {
        return Measured::refused(format!("{e:?}"));
    }
    let reader = match named(&ast, t1)
        .and_then(|id| type_identifier(&ast, id, Form::Minimal).map_err(|e| format!("{e:?}")))
    {
        Err(e) => return Measured::refused(e),
        Ok(id) => id,
    };
    let writer = match named(&ast, t2)
        .and_then(|id| type_identifier(&ast, id, Form::Minimal).map_err(|e| format!("{e:?}")))
    {
        Err(e) => return Measured::refused(e),
        Ok(id) => id,
    };
    // XTypes 1.3 says "T1 is-assignable-from T2" with T1 the reader's
    // type; `is_assignable` takes the writer's first.
    let verdict = is_assignable(&writer, &reader, &registry, &AssignabilityConfig::default());
    let note = format!("{verdict:?}");
    Measured {
        decode: Cell::Ok,
        encode: Cell::Skip,
        rule: Cell::from(verdict.is_yes()),
        note,
    }
}

fn probe_datarep(arg: &str) -> Measured {
    let Some((offered, requested)) = arg.split_once(";;") else {
        return Measured::refused("a `datarep` probe reads `<offered>;;<requested>`");
    };
    let parse = |s: &str| -> Option<Vec<i16>> {
        if s.is_empty() {
            return Some(Vec::new());
        }
        s.split(',').map(|p| p.trim().parse::<i16>().ok()).collect()
    };
    let (Some(o), Some(r)) = (parse(offered), parse(requested)) else {
        return Measured::refused("a representation list is a comma-separated list of i16");
    };
    let verdict = negotiate_representation(&o, &r);
    let rule = Cell::from(matches!(verdict, RepresentationNegotiation::Accepted(_)));
    Measured {
        decode: Cell::Skip,
        encode: Cell::Skip,
        rule,
        note: format!("{verdict:?}"),
    }
}

fn probe_encaps(arg: &str) -> Measured {
    let Some((repr, ext)) = arg.split_once(";;") else {
        return Measured::refused("an `encaps` probe reads `<repr>;;<extensibility>`");
    };
    let repr = match repr {
        "XCDR" => DataRepresentationId::Xcdr1,
        "XCDR2" => DataRepresentationId::Xcdr2,
        "XML" => DataRepresentationId::Xml,
        other => return Measured::refused(format!("unknown representation `{other}`")),
    };
    let ext = match ext {
        "FINAL" => ExtensibilityForRepr::Final,
        "APPENDABLE" => ExtensibilityForRepr::Appendable,
        "MUTABLE" => ExtensibilityForRepr::Mutable,
        other => return Measured::refused(format!("unknown extensibility `{other}`")),
    };
    let verdict = check_data_repr_extensibility(repr, ext);
    let note = verdict.err().unwrap_or_default().to_string();
    Measured {
        decode: Cell::Skip,
        encode: Cell::Skip,
        rule: Cell::from(note.is_empty()),
        note,
    }
}

fn probe_tl(arg: &str) -> Measured {
    let Some((which, hex)) = arg.split_once(";;") else {
        return Measured::refused("a `tl` probe reads `<TypeLookup type>;;<hex>`");
    };
    let Some(bytes) = unhex(hex) else {
        return Measured::refused("the probe's argument is not hex");
    };
    macro_rules! trip {
        ($ty:ty) => {{
            let mut r = zerodds_types::framing::xcdr2_reader(&bytes);
            match <$ty>::decode_from(&mut r) {
                Err(e) => Measured::refused(format!("{e:?}")),
                Ok(value) => {
                    let mut w = zerodds_types::framing::xcdr2_writer();
                    let written = value.encode_into(&mut w);
                    let back = w.finish();
                    Measured {
                        decode: Cell::Ok,
                        encode: Cell::from(written.is_ok() && back == bytes),
                        rule: Cell::Skip,
                        note: if back == bytes {
                            String::new()
                        } else {
                            // The octets, not just their count. A `tl`
                            // case is written by hand out of §7.6.3.3.3,
                            // and the one thing that makes writing one
                            // cheap is seeing what this tree produced
                            // for the same value.
                            let hex: String =
                                back.iter().map(|b| format!("{b:02x}")).collect();
                            format!("re-encoded {} octet(s): {hex}", back.len())
                        },
                    }
                }
            }
        }};
    }
    match which {
        "getTypes_In" => trip!(GetTypesRequest),
        "getTypes_Out" => trip!(GetTypesReply),
        "getTypeDependencies_In" => trip!(GetTypeDependenciesRequest),
        "getTypeDependencies_Out" => trip!(GetTypeDependenciesReply),
        "getTypes_Result" => trip!(GetTypesResult),
        "getTypeDependencies_Result" => trip!(GetTypeDependenciesResult),
        "Call" => trip!(TypeLookupCall),
        "Return" => trip!(TypeLookupReturn),
        "Request" => trip!(TypeLookupRequest),
        "Reply" => trip!(TypeLookupReplyMessage),
        other => Measured::refused(format!("no TypeLookup type `{other}` in this tree")),
    }
}

fn main() {
    let explain = std::env::args().any(|a| a == "--explain");
    let stdin = io::BufReader::new(io::stdin());
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lines() {
        let Ok(line) = line else { break };
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut it = line.splitn(3, '\t');
        let (Some(id), Some(op), Some(arg)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let arg = arg.replace("<NL>", "\n");
        let m = match op {
            "ti" => probe_ti(&arg),
            "to" => probe_to(&arg),
            "idl" => probe_idl(&arg),
            "assign" => probe_assign(&arg),
            "datarep" => probe_datarep(&arg),
            "encaps" => probe_encaps(&arg),
            "tl" => probe_tl(&arg),
            "todh" => probe_todh(&arg),
            other => Measured::refused(format!("no op `{other}`")),
        };
        if explain {
            let _ = writeln!(
                out,
                "{id}\t{}\t{}\t{}\t{}",
                m.decode.as_str(),
                m.encode.as_str(),
                m.rule.as_str(),
                m.note.replace('\n', " ")
            );
        } else {
            let _ = writeln!(
                out,
                "{id}\t{}\t{}\t{}",
                m.decode.as_str(),
                m.encode.as_str(),
                m.rule.as_str()
            );
        }
    }
}
