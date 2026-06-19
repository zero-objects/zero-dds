//! Apply vendor key pragmas to the AST.
//!
//! `#pragma keylist`, `#pragma DCPS_DATA_KEY`, `#pragma cats` (as well as the
//! RTI top-level `keylist` VendorExtension) mark key fields *outside*
//! the type definition. The preprocessor only captures them (see
//! [`crate::preprocessor`]); only here do they become semantically effective, by
//! generating a synthetic `@key` in the named struct members. This makes
//! a pragma key marking downstream identical to an inline `@key`
//! (same lowering, same `TypeObject` flags, same CDR key hash).
//!
//! The caller combines preprocessor and parser themselves (the parser does not see
//! the pragmas, they are removed before the tokenizer); afterwards it calls
//! [`apply_key_pragmas`] with the parsed [`Specification`] and the
//! [`ProcessedSource`].

use crate::ast::{
    Annotation, AnnotationParams, ConstrTypeDecl, Definition, Identifier, Member, ScopedName,
    Specification, StructDcl, StructDef, TypeDecl,
};
use crate::errors::Span;
use crate::preprocessor::{OpenSplicePragma, ProcessedSource};

/// Result of [`apply_key_pragmas`]: what was applied and what could not
/// be resolved (for diagnostics at the consumer).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyPragmaReport {
    /// Number of members into which a synthetic `@key` was injected.
    pub applied: usize,
    /// Pragma type names that were not found in the AST.
    pub unresolved_types: Vec<String>,
    /// `(type, field)` pairs whose field does not exist in the found struct.
    pub unresolved_fields: Vec<(String, String)>,
}

/// Applies all captured key pragmas from `processed` to `spec` by injecting
/// a synthetic `@key` into the named struct members.
///
/// Idempotent: a member that already carries `@key` (inline or from an
/// earlier pragma run) is not marked twice.
pub fn apply_key_pragmas(spec: &mut Specification, processed: &ProcessedSource) -> KeyPragmaReport {
    let mut report = KeyPragmaReport::default();

    for kl in &processed.pragma_keylists {
        apply_one(spec, &kl.type_name, &kl.keys, &mut report);
    }

    for osp in &processed.opensplice_pragmas {
        match osp {
            OpenSplicePragma::DataKey {
                type_name, fields, ..
            } => apply_one(spec, type_name, fields, &mut report),
            OpenSplicePragma::Cats {
                type_name, keys, ..
            } => apply_one(spec, type_name, keys, &mut report),
            _ => {}
        }
    }

    // RTI top-level `keylist Type(f, ...);` is present as a VendorExtension with
    // raw reconstructed token text. Since the generic delta fallback sets no
    // meaningful `production_name`, we discriminate via the content:
    // `parse_rti_keylist` returns `Some` only if `raw` begins with `keylist`.
    // First collect (immutable borrow), then apply (mutable).
    let rti: Vec<(String, Vec<String>)> = spec
        .definitions
        .iter()
        .filter_map(|d| match d {
            Definition::VendorExtension(v) => parse_rti_keylist(&v.raw),
            _ => None,
        })
        .collect();
    for (type_name, keys) in rti {
        apply_one(spec, &type_name, &keys, &mut report);
    }

    report
}

/// Marks the `keys` fields in struct `type_name` as `@key`.
///
/// Empty `keys` (e.g. `#pragma keylist Foo` without fields = keyless topic) are
/// a no-op at the member level; the type is only verified as known.
fn apply_one(
    spec: &mut Specification,
    type_name: &str,
    keys: &[String],
    report: &mut KeyPragmaReport,
) {
    let parts: Vec<&str> = type_name.split("::").filter(|s| !s.is_empty()).collect();
    let Some(st) = find_struct_mut(&mut spec.definitions, &parts) else {
        report.unresolved_types.push(type_name.to_string());
        return;
    };

    for key in keys {
        let Some(member) = st.members.iter_mut().find(|m| member_has_field(m, key)) else {
            report
                .unresolved_fields
                .push((type_name.to_string(), key.clone()));
            continue;
        };
        if !member_has_key(member) {
            let span = member.span;
            member.annotations.push(synthetic_key(span));
            report.applied += 1;
        }
    }
}

/// Resolves a (possibly scoped) type name against the module tree.
///
/// `M::Foo` navigates the module path exactly; an unscoped name `Foo` is
/// searched recursively across all module levels (pragmas often note the type
/// without a full scope).
///
/// zerodds-lint: recursion-depth 64 (module-tree walk; bounded by IDL nesting)
fn find_struct_mut<'a>(defs: &'a mut [Definition], parts: &[&str]) -> Option<&'a mut StructDef> {
    match parts {
        [] => None,
        [last] => find_struct_recursive(defs, last),
        [head, rest @ ..] => {
            for d in defs.iter_mut() {
                if let Definition::Module(m) = d {
                    if m.name.text == *head {
                        return find_struct_mut(&mut m.definitions, rest);
                    }
                }
            }
            None
        }
    }
}

/// zerodds-lint: recursion-depth 64 (Modulbaum-Walk; bounded by IDL nesting)
fn find_struct_recursive<'a>(defs: &'a mut [Definition], name: &str) -> Option<&'a mut StructDef> {
    for d in defs.iter_mut() {
        match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))))
                if s.name.text == name =>
            {
                return Some(s);
            }
            Definition::Module(m) => {
                if let Some(s) = find_struct_recursive(&mut m.definitions, name) {
                    return Some(s);
                }
            }
            _ => {}
        }
    }
    None
}

fn member_has_field(m: &Member, field: &str) -> bool {
    m.declarators.iter().any(|d| d.name().text == field)
}

fn member_has_key(m: &Member) -> bool {
    m.annotations
        .iter()
        .any(|a| a.name.parts.last().is_some_and(|p| p.text == "key"))
}

fn synthetic_key(span: Span) -> Annotation {
    Annotation {
        name: ScopedName::single(Identifier::new("key", span)),
        params: AnnotationParams::None,
        span,
    }
}

/// Extracts `(type, fields)` from a raw RTI `keylist` slice.
///
/// Accepts both spellings: `keylist Foo(a, b)` (RTI parenthesized form) and
/// `keylist Foo a b` (whitespace-separated). Trailing `;`/`)` are stripped.
fn parse_rti_keylist(raw: &str) -> Option<(String, Vec<String>)> {
    let rest = raw.trim().strip_prefix("keylist")?.trim_start();
    let (type_part, field_part) = if let Some(idx) = rest.find('(') {
        (&rest[..idx], &rest[idx + 1..])
    } else if let Some(idx) = rest.find(char::is_whitespace) {
        (&rest[..idx], &rest[idx..])
    } else {
        (rest, "")
    };

    let type_name = type_part.trim().trim_end_matches(';').trim().to_string();
    if type_name.is_empty() {
        return None;
    }

    let keys = field_part
        .split(|c: char| c == ',' || c == ')' || c.is_whitespace())
        .map(|s| s.trim().trim_end_matches(';'))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    Some((type_name, keys))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::preprocessor::{MemoryResolver, Preprocessor};
    use crate::semantics::lower_annotations;

    /// Full path: preprocess -> parse -> apply_key_pragmas. Returns the
    /// lowered key info of the first member of the named struct.
    fn keyed_members(src: &str, struct_name: &str) -> Vec<(String, bool)> {
        let processed = Preprocessor::new(MemoryResolver::new())
            .process("main.idl", src)
            .expect("preprocess");
        let mut spec = crate::parse(&processed.expanded, &ParserConfig::default()).expect("parse");
        apply_key_pragmas(&mut spec, &processed);
        let st = find_struct_recursive(&mut spec.definitions, struct_name).expect("struct");
        st.members
            .iter()
            .map(|m| {
                let name = m.declarators[0].name().text.clone();
                let is_key = lower_annotations(&m.annotations)
                    .map(|l| l.has_key())
                    .unwrap_or(false);
                (name, is_key)
            })
            .collect()
    }

    #[test]
    fn pragma_keylist_marks_same_keys_as_at_key() {
        // `#pragma keylist` and inline `@key` must mark exactly the same members
        // as key.
        let via_pragma = keyed_members(
            "struct Sensor { long id; double value; };\n#pragma keylist Sensor id\n",
            "Sensor",
        );
        let via_at_key =
            keyed_members("struct Sensor { @key long id; double value; };\n", "Sensor");
        assert_eq!(via_pragma, via_at_key);
        assert_eq!(
            via_pragma,
            vec![("id".into(), true), ("value".into(), false)]
        );
    }

    #[test]
    fn pragma_keylist_multiple_keys() {
        let m = keyed_members(
            "struct K { long a; long b; long c; };\n#pragma keylist K a c\n",
            "K",
        );
        assert_eq!(
            m,
            vec![("a".into(), true), ("b".into(), false), ("c".into(), true)]
        );
    }

    #[test]
    fn dcps_data_key_marks_key() {
        let m = keyed_members(
            "struct D { long k; long v; };\n#pragma DCPS_DATA_KEY D k\n",
            "D",
        );
        assert_eq!(m, vec![("k".into(), true), ("v".into(), false)]);
    }

    #[test]
    fn pragma_does_not_duplicate_existing_at_key() {
        // Inline `@key` + redundant pragma on the same field: idempotent,
        // exactly one `@key` annotation.
        let processed = Preprocessor::new(MemoryResolver::new())
            .process(
                "main.idl",
                "struct S { @key long id; };\n#pragma keylist S id\n",
            )
            .expect("preprocess");
        let mut spec = crate::parse(&processed.expanded, &ParserConfig::default()).expect("parse");
        let report = apply_key_pragmas(&mut spec, &processed);
        assert_eq!(report.applied, 0, "already keyed -> no re-injection");
        let st = find_struct_recursive(&mut spec.definitions, "S").expect("struct");
        let key_anns = st.members[0]
            .annotations
            .iter()
            .filter(|a| a.name.parts.last().unwrap().text == "key")
            .count();
        assert_eq!(key_anns, 1);
    }

    #[test]
    fn unresolved_type_is_reported_not_panicked() {
        let processed = Preprocessor::new(MemoryResolver::new())
            .process(
                "main.idl",
                "struct A { long x; };\n#pragma keylist Nonexistent x\n",
            )
            .expect("preprocess");
        let mut spec = crate::parse(&processed.expanded, &ParserConfig::default()).expect("parse");
        let report = apply_key_pragmas(&mut spec, &processed);
        assert_eq!(report.applied, 0);
        assert_eq!(report.unresolved_types, vec!["Nonexistent".to_string()]);
    }

    #[test]
    fn parse_source_applies_keylist_in_one_call() {
        // The bundled `parse_source` convenience must apply the keylist without
        // a separate apply_key_pragmas call.
        let spec = crate::parse_source(
            "main.idl",
            "struct Sensor { long id; double value; };\n#pragma keylist Sensor id\n",
            MemoryResolver::new(),
            &ParserConfig::default(),
        )
        .expect("parse_source");
        let mut spec = spec;
        let st = find_struct_recursive(&mut spec.definitions, "Sensor").expect("struct");
        let id_keyed = lower_annotations(&st.members[0].annotations)
            .map(|l| l.has_key())
            .unwrap_or(false);
        assert!(id_keyed, "parse_source must apply #pragma keylist");
    }

    #[test]
    fn rti_top_level_keylist_marks_key_e2e() {
        // RTI Connext: top-level `keylist Type (field);` (grammar delta, not a
        // preprocessor pragma) must also act as @key. The delta
        // node lands as a VendorExtension with raw token text; apply reads
        // it directly from the Specification (an empty ProcessedSource suffices).
        use crate::grammar::deltas::RTI_CONNEXT;
        use crate::parser::parse_with_deltas;
        let src = "struct Sensor { long id; double value; };\nkeylist Sensor (id);\n";
        let mut spec = parse_with_deltas(src, &ParserConfig::default(), &[&RTI_CONNEXT])
            .expect("RTI delta parse");
        let report = apply_key_pragmas(&mut spec, &ProcessedSource::default());
        assert_eq!(report.applied, 1, "RTI keylist must mark exactly id");
        let st = find_struct_recursive(&mut spec.definitions, "Sensor").expect("struct");
        let keyed: Vec<(String, bool)> = st
            .members
            .iter()
            .map(|m| {
                (
                    m.declarators[0].name().text.clone(),
                    lower_annotations(&m.annotations)
                        .map(|l| l.has_key())
                        .unwrap_or(false),
                )
            })
            .collect();
        assert_eq!(keyed, vec![("id".into(), true), ("value".into(), false)]);
    }

    #[test]
    fn rti_keylist_raw_parse_both_forms() {
        assert_eq!(
            parse_rti_keylist("keylist Foo(a, b)"),
            Some(("Foo".into(), vec!["a".into(), "b".into()]))
        );
        assert_eq!(
            parse_rti_keylist("keylist Foo a b"),
            Some(("Foo".into(), vec!["a".into(), "b".into()]))
        );
        assert_eq!(
            parse_rti_keylist("keylist Foo;"),
            Some(("Foo".into(), vec![]))
        );
    }
}
