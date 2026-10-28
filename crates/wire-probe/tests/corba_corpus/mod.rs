// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The corpus pass of the CORBA column, shared by `corba_column.rs` and
//! `corba_column_headline.rs`.

use zerodds_wire_probe::corba_idl::corba_idl;
use zerodds_wire_probe::encode::model_from_idl;

/// Prints every case of `tests/interop-corpus` as CORBA IDL and returns
/// how many printed and how many were refused. A case whose printed IDL
/// keeps an annotation, or whose refusal gives no reason, fails here.
pub fn print_or_refuse_every_case() -> (usize, usize) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/interop-corpus")
        .canonicalize()
        .unwrap_or_else(|error| panic!("the corpus is not where it was: {error}"));
    let mut printed = 0usize;
    let mut refused = 0usize;
    let mut entries: Vec<_> = std::fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("{error}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for case in entries {
        let idl = case.join("type.idl");
        let Ok(source) = std::fs::read_to_string(&idl) else {
            continue;
        };
        let name = std::fs::read_to_string(case.join("type")).unwrap_or_else(|_| "Sample".into());
        let case_name = case
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        let ast = match model_from_idl(&source, &idl.display().to_string()) {
            Ok(ast) => ast,
            // The two cases the IDL front end refuses; see
            // `zerodds_wire_probe::REFUSED_BY_THE_FRONT_END`. They are
            // refusals of this column too — a case with no model has no
            // CORBA form — and the front end's sentence names the clause,
            // so it satisfies the "a refusal names its construct" rule
            // above without a second wording.
            Err(error)
                if zerodds_wire_probe::REFUSED_BY_THE_FRONT_END.contains(&case_name.as_str()) =>
            {
                let message = format!("{error}");
                assert!(
                    message.len() > 30,
                    "{}: a refusal with no reason: {message}",
                    case.display()
                );
                refused += 1;
                continue;
            }
            Err(error) => panic!("{}: {error}", case.display()),
        };
        let segments: Vec<String> = name
            .trim()
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(ToString::to_string)
            .collect();
        let Some(type_) = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
        else {
            panic!("{}: `{}` is not in the model", case.display(), name.trim());
        };
        match corba_idl(&ast, type_) {
            Ok(text) => {
                assert!(
                    !text.is_empty() && !text.contains('@'),
                    "{}: an annotation reached the printed IDL:\n{text}",
                    case.display()
                );
                printed += 1;
            }
            Err(error) => {
                let message = format!("{error}");
                // A refusal has to name the construct. "unsupported"
                // with no noun is what a filter would produce, and a
                // run cannot act on it.
                assert!(
                    message.len() > 30,
                    "{}: a refusal with no reason: {message}",
                    case.display()
                );
                refused += 1;
            }
        }
    }
    (printed, refused)
}
