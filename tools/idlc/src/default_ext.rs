// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Default-Extensibility / Default-Nested als AST-Patch-Pass.
//!
//! OMG XTypes 1.3 §7.2.2.4 laesst die Extensibility eines aggregierten
//! Typs (`struct`/`union`) bzw. eines `enum` offen, wenn keine
//! Annotation (`@final` / `@appendable` / `@mutable` /
//! `@extensibility(...)`) gesetzt ist — der Default ist
//! implementierungsdefiniert. Fast DDS macht ihn ueber
//! `-default_extensibility` zum CLI-Schalter, Cyclone DDS ueber `-x`.
//!
//! Analog laesst XTypes die Nestedness offen; Cyclone steuert sie
//! ueber `-n`.
//!
//! Dieses Modul materialisiert die CLI-Wahl im AST: jeder
//! un-annotierte Typ bekommt vor dem Codegen eine synthetische
//! Alias-Annotation. Damit sehen alle sieben Backends denselben
//! Default — jeder Emitter hat sonst seinen eigenen Hardcoded-
//! Fallback (z.B. liest der Rust-Emitter un-annotierte Structs als
//! `Final`). Bereits annotierte Typen bleiben unangetastet.

use zerodds_idl::ast::{
    Annotation, AnnotationParams, ConstrTypeDecl, Definition, Identifier, ScopedName,
    Specification, StructDcl, TypeDecl, UnionDcl,
};
use zerodds_idl::errors::Span;

/// Synthetischer Null-Span fuer Compiler-generierte AST-Knoten.
const SYNTH: Span = Span { start: 0, end: 0 };

/// Extensibility-Default aus `--default-extensibility`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefaultExt {
    Final,
    Appendable,
    Mutable,
}

impl DefaultExt {
    /// Built-in default selected at compile time by the `ext-default-*` Cargo
    /// features (XTypes 1.3 §7.3.3.1). Used when no `--default-extensibility`
    /// CLI flag is given. The default feature is `ext-default-appendable`
    /// (FastDDS/spec-leaning); build idlc with
    /// `--no-default-features --features ext-default-final` for a Cyclone-DDS
    /// compatible default.
    #[must_use]
    pub const fn cfg_default() -> Self {
        #[cfg(feature = "ext-default-final")]
        {
            Self::Final
        }
        #[cfg(all(feature = "ext-default-mutable", not(feature = "ext-default-final")))]
        {
            Self::Mutable
        }
        #[cfg(all(
            not(feature = "ext-default-final"),
            not(feature = "ext-default-mutable")
        ))]
        {
            Self::Appendable
        }
    }

    /// CLI-Wert (`final` / `appendable` / `mutable`) parsen.
    pub fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "final" => Self::Final,
            "appendable" => Self::Appendable,
            "mutable" => Self::Mutable,
            _ => return None,
        })
    }

    /// Annotation-Alias-Name fuer den Patch (`@final` etc.).
    fn alias(self) -> &'static str {
        match self {
            Self::Final => "final",
            Self::Appendable => "appendable",
            Self::Mutable => "mutable",
        }
    }
}

/// Wendet die Default-Extensibility auf den AST an. Liefert die Anzahl
/// der Typen, die dadurch neu annotiert wurden.
pub fn apply_default_extensibility(spec: &mut Specification, ext: DefaultExt) -> usize {
    let mut patched = 0;
    for def in &mut spec.definitions {
        patched += patch_definition(def, &|annos| {
            if has_any(annos, &["final", "appendable", "mutable", "extensibility"]) {
                false
            } else {
                annos.push(make_annotation(ext.alias()));
                true
            }
        });
    }
    patched
}

/// Wendet Default-Nested auf den AST an: jeder Typ ohne `@nested` und
/// ohne `@topic` bekommt `@nested`. Liefert die Anzahl der Patches.
/// `nested == false` ist ein No-Op (der OMG-Default).
pub fn apply_default_nested(spec: &mut Specification, nested: bool) -> usize {
    if !nested {
        return 0;
    }
    let mut patched = 0;
    for def in &mut spec.definitions {
        patched += patch_definition(def, &|annos| {
            if has_any(annos, &["nested", "topic"]) {
                false
            } else {
                annos.push(make_annotation("nested"));
                true
            }
        });
    }
    patched
}

/// Rekursiver Walk durch Module. `patch` bekommt die Annotation-Liste
/// eines struct/union/enum und liefert `true`, wenn es gepatcht hat.
///
/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn patch_definition(def: &mut Definition, patch: &dyn Fn(&mut Vec<Annotation>) -> bool) -> usize {
    match def {
        Definition::Module(m) => {
            let mut n = 0;
            for d in &mut m.definitions {
                n += patch_definition(d, patch);
            }
            n
        }
        Definition::Type(TypeDecl::Constr(ctd)) => {
            let annos = match ctd {
                ConstrTypeDecl::Struct(StructDcl::Def(s)) => &mut s.annotations,
                ConstrTypeDecl::Union(UnionDcl::Def(u)) => &mut u.annotations,
                ConstrTypeDecl::Enum(e) => &mut e.annotations,
                // Forward-Decls + Bitset/Bitmask: keine Extensibility/
                // Nestedness im hier behandelten Sinn.
                _ => return 0,
            };
            usize::from(patch(annos))
        }
        _ => 0,
    }
}

/// Hat die Annotation-Liste eine Annotation mit einem der Namen?
fn has_any(annos: &[Annotation], names: &[&str]) -> bool {
    annos.iter().any(|a| {
        a.name
            .parts
            .last()
            .is_some_and(|p| names.contains(&p.text.as_str()))
    })
}

/// Synthetische Annotation `@<name>` ohne Parameter.
fn make_annotation(name: &str) -> Annotation {
    Annotation {
        name: ScopedName::single(Identifier::new(name, SYNTH)),
        params: AnnotationParams::None,
        span: SYNTH,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use zerodds_idl::config::ParserConfig;
    use zerodds_idl::parser::parse;

    #[test]
    fn cfg_default_matches_active_feature() {
        // The compile-time `ext-default-*` feature selects the built-in default.
        #[cfg(all(
            not(feature = "ext-default-final"),
            not(feature = "ext-default-mutable")
        ))]
        assert_eq!(DefaultExt::cfg_default(), DefaultExt::Appendable);
        #[cfg(feature = "ext-default-final")]
        assert_eq!(DefaultExt::cfg_default(), DefaultExt::Final);
        #[cfg(all(feature = "ext-default-mutable", not(feature = "ext-default-final")))]
        assert_eq!(DefaultExt::cfg_default(), DefaultExt::Mutable);
    }

    fn parse_spec(src: &str) -> Specification {
        parse(src, &ParserConfig::default()).expect("parse")
    }

    /// Liefert die Annotation-Namen eines Top-Level-Structs `name`.
    fn struct_annos(spec: &Specification, name: &str) -> Vec<String> {
        for def in &spec.definitions {
            if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) =
                def
            {
                if s.name.text == name {
                    return s
                        .annotations
                        .iter()
                        .filter_map(|a| a.name.parts.last().map(|p| p.text.clone()))
                        .collect();
                }
            }
        }
        panic!("struct {name} not found");
    }

    #[test]
    fn unannotated_struct_gets_default() {
        let mut spec = parse_spec("struct Plain { long a; };");
        let n = apply_default_extensibility(&mut spec, DefaultExt::Appendable);
        assert_eq!(n, 1);
        assert_eq!(struct_annos(&spec, "Plain"), vec!["appendable"]);
    }

    #[test]
    fn already_annotated_struct_untouched() {
        // @final-Struct darf bei default appendable NICHT zusaetzlich
        // @appendable bekommen.
        let mut spec = parse_spec("@final struct Fixed { long a; };");
        let n = apply_default_extensibility(&mut spec, DefaultExt::Appendable);
        assert_eq!(n, 0);
        assert_eq!(struct_annos(&spec, "Fixed"), vec!["final"]);
    }

    #[test]
    fn extensibility_annotation_form_also_counts() {
        let mut spec = parse_spec("@extensibility(MUTABLE) struct M { long a; };");
        let n = apply_default_extensibility(&mut spec, DefaultExt::Final);
        assert_eq!(n, 0, "@extensibility(...) must be recognised as annotated");
    }

    #[test]
    fn default_applies_inside_modules() {
        let mut spec = parse_spec("module Outer { struct Inner { long a; }; };");
        let n = apply_default_extensibility(&mut spec, DefaultExt::Final);
        assert_eq!(n, 1, "module-nested struct must be patched");
    }

    #[test]
    fn unions_and_enums_are_patched() {
        let src = "enum Color { RED, GREEN }; \
                   union U switch (long) { case 1: long x; };";
        let mut spec = parse_spec(src);
        let n = apply_default_extensibility(&mut spec, DefaultExt::Appendable);
        assert_eq!(n, 2, "enum + union both get the default");
    }

    #[test]
    fn default_nested_false_is_noop() {
        let mut spec = parse_spec("struct S { long a; };");
        assert_eq!(apply_default_nested(&mut spec, false), 0);
    }

    #[test]
    fn default_nested_true_marks_unannotated() {
        let mut spec = parse_spec("struct S { long a; };");
        let n = apply_default_nested(&mut spec, true);
        assert_eq!(n, 1);
        assert!(struct_annos(&spec, "S").contains(&"nested".to_string()));
    }

    #[test]
    fn default_nested_skips_topic_annotated() {
        let mut spec = parse_spec("@topic struct T { long a; };");
        assert_eq!(apply_default_nested(&mut spec, true), 0);
    }

    #[test]
    fn parse_rejects_unknown_extensibility() {
        assert_eq!(
            DefaultExt::parse("appendable"),
            Some(DefaultExt::Appendable)
        );
        assert_eq!(DefaultExt::parse("garbage"), None);
    }
}
