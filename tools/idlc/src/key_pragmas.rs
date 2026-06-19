// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Vendor-Pragma → `@key`-Annotation.
//!
//! OpenDDS, Cyclone DDS und OpenSplice markieren Topic-Schluessel
//! historisch ueber Preprocessor-Pragmas statt der OMG-IDL-4.2-
//! Annotation `@key`:
//!
//! * `#pragma keylist <Type> <field>...`        (Cyclone / OpenSplice / RTI)
//! * `#pragma DCPS_DATA_KEY "<Type> <field>"`   (OpenDDS / OpenSplice)
//! * `#pragma cats <Type> <field>...`           (OpenSplice catenated keys)
//!
//! Der `zerodds-idl`-Preprocessor erfasst diese Pragmas strukturiert
//! (`ProcessedSource::pragma_keylists` / `opensplice_pragmas`). Dieses
//! Modul wendet sie nach dem Parsen auf den AST an: jeder genannte
//! Member bekommt eine synthetische `@key`-Annotation, sofern er noch
//! keine hat. Damit sehen alle sieben Backends den Schluessel
//! einheitlich — egal ob die IDL `@key` oder ein Vendor-Pragma nutzt.

use std::collections::{HashMap, HashSet};

use zerodds_idl::ast::{
    Annotation, AnnotationParams, ConstrTypeDecl, Definition, Identifier, Member, ScopedName,
    Specification, StructDcl, StructDef, TypeDecl,
};
use zerodds_idl::errors::Span;
use zerodds_idl::preprocessor::{OpenSplicePragma, PragmaKeylist};

/// Synthetischer Null-Span fuer Compiler-generierte AST-Knoten.
const SYNTH: Span = Span { start: 0, end: 0 };

/// Schluessel-Felder pro Typ-Name. Der Typ-Name kann scoped
/// (`Robot::Pose`) oder lokal (`Pose`) sein — beim Matching wird
/// beides geprueft.
type KeyMap = HashMap<String, HashSet<String>>;

/// Baut die `KeyMap` aus den vom Preprocessor erfassten Pragmas.
#[must_use]
pub fn collect_key_pragmas(keylists: &[PragmaKeylist], opensplice: &[OpenSplicePragma]) -> KeyMap {
    let mut map: KeyMap = HashMap::new();
    for kl in keylists {
        let set = map.entry(kl.type_name.clone()).or_default();
        for k in &kl.keys {
            set.insert(k.clone());
        }
    }
    for p in opensplice {
        match p {
            OpenSplicePragma::DataKey {
                type_name, fields, ..
            } => {
                let set = map.entry(type_name.clone()).or_default();
                for f in fields {
                    set.insert(f.clone());
                }
            }
            OpenSplicePragma::Cats {
                type_name, keys, ..
            } => {
                let set = map.entry(type_name.clone()).or_default();
                for k in keys {
                    set.insert(k.clone());
                }
            }
            // DataType markiert nur Topic-Faehigkeit (kein Key);
            // GenEquality ist ein Codegen-Flag — beide hier irrelevant.
            OpenSplicePragma::DataType { .. } | OpenSplicePragma::GenEquality { .. } => {}
        }
    }
    map
}

/// Wendet die Key-Pragmas auf den AST an. Liefert die Anzahl der
/// Member, die dadurch neu als `@key` markiert wurden.
pub fn apply_key_pragmas(spec: &mut Specification, keys: &KeyMap) -> usize {
    if keys.is_empty() {
        return 0;
    }
    let mut patched = 0;
    for def in &mut spec.definitions {
        patched += patch_definition(def, &[], keys);
    }
    patched
}

/// Rekursiver Walk. `scope` ist der Modul-Pfad bis hierher.
///
/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn patch_definition(def: &mut Definition, scope: &[String], keys: &KeyMap) -> usize {
    match def {
        Definition::Module(m) => {
            let mut inner_scope = scope.to_vec();
            inner_scope.push(m.name.text.clone());
            let mut n = 0;
            for d in &mut m.definitions {
                n += patch_definition(d, &inner_scope, keys);
            }
            n
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
            patch_struct(s, scope, keys)
        }
        _ => 0,
    }
}

/// Markiert die Key-Member eines Structs.
fn patch_struct(s: &mut StructDef, scope: &[String], keys: &KeyMap) -> usize {
    // Pragma-Type-Name kann scoped (`A::B::Pose`) oder lokal (`Pose`)
    // sein — beide Schreibweisen akzeptieren.
    let local = s.name.text.clone();
    let scoped = {
        let mut parts = scope.to_vec();
        parts.push(local.clone());
        parts.join("::")
    };
    let Some(key_fields) = keys.get(&scoped).or_else(|| keys.get(&local)) else {
        return 0;
    };
    let mut patched = 0;
    for member in &mut s.members {
        for decl in &member.declarators {
            if key_fields.contains(&decl.name().text) && !has_key_annotation(member) {
                member.annotations.push(make_key_annotation());
                patched += 1;
            }
        }
    }
    patched
}

/// Hat der Member bereits eine `@key`-Annotation?
fn has_key_annotation(member: &Member) -> bool {
    member
        .annotations
        .iter()
        .any(|a| a.name.parts.last().is_some_and(|p| p.text == "key"))
}

/// Synthetische `@key`-Annotation (kein Klammerpaar → `Params::None`).
fn make_key_annotation() -> Annotation {
    Annotation {
        name: ScopedName::single(Identifier::new("key", SYNTH)),
        params: AnnotationParams::None,
        span: SYNTH,
    }
}
