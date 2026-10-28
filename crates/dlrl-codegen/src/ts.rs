// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeScript mapping.
//!
//! As with C#, DDS 1.4 Annex B fixes no TypeScript PSM. Two artefacts
//! are emitted rather than one: an interface, which is what application
//! code should refer to, and a class that implements it, which is what
//! the runtime instantiates. Emitting only the class would make every
//! consumer depend on the implementation.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{DlrlAttributeType, DlrlCollection, DlrlFieldType, DlrlMonoType, DlrlTypeInfo};

/// A basic type's TypeScript type and the initial value the class gives
/// it: the integers up to 32 bits and both floats are `number`, the
/// 64-bit integers `bigint` (as ZeroDDS' TypeScript binding maps them),
/// `char` and `string` a `string`.
fn ts_basic(ty: DlrlFieldType) -> (&'static str, &'static str) {
    match ty {
        DlrlFieldType::Boolean => ("boolean", "false"),
        DlrlFieldType::Char | DlrlFieldType::String => ("string", "\"\""),
        DlrlFieldType::LongLong | DlrlFieldType::ULongLong => ("bigint", "0n"),
        _ => ("number", "0"),
    }
}

/// A mono-valued type and its initial value, in the mapping `idlc
/// generate --lang typescript` uses for the same IDL: an enumeration is
/// the string-literal union it declares and starts at its default
/// enumerator, a structure is the interface it declares and starts at
/// what its `new<Name>()` returns; a module is a namespace. An unresolved
/// key is `never`, which no value satisfies, and has no initial value.
fn ts_mono(ty: Option<&DlrlMonoType>) -> (String, Option<String>) {
    match ty {
        Some(DlrlMonoType::Basic(basic)) => {
            let (ty, initial) = ts_basic(*basic);
            (ty.to_string(), Some(initial.to_string()))
        }
        Some(DlrlMonoType::Enum { name, default }) => {
            let path = name.replace("::", ".");
            let initial = format!("{path}.{default}");
            (path, Some(initial))
        }
        Some(DlrlMonoType::Struct { name }) => {
            let path = name.replace("::", ".");
            let initial = match path.rsplit_once('.') {
                Some((scope, simple)) => format!("{scope}.new{simple}()"),
                None => format!("new{path}()"),
            };
            (path, Some(initial))
        }
        None => ("never".to_string(), None),
    }
}

/// A multi-valued attribute's type and initial value: an array for a
/// list, a `Map` for either map.
fn ts_collection(basis: &DlrlCollection, item: &DlrlMonoType) -> (String, &'static str) {
    let (item, _) = ts_mono(Some(item));
    match basis {
        DlrlCollection::List => (format!("{item}[]"), "[]"),
        DlrlCollection::StrMap => (format!("Map<string, {item}>"), "new Map()"),
        DlrlCollection::IntMap(key) => (format!("Map<{}, {item}>", ts_basic(*key).0), "new Map()"),
    }
}

/// The `import` of the data types `idlc generate --lang typescript` wrote
/// to `module` (`./trade` for `trade.ts`), when any class names one: the
/// outermost module of a type declared in a module, else the type itself
/// and, for a structure, its `new<Name>` factory. `None` when no class
/// names an IDL-declared type.
#[must_use]
pub fn generate_ts_import<'a>(
    infos: impl IntoIterator<Item = &'a DlrlTypeInfo>,
    module: &str,
) -> Option<String> {
    let mut names = BTreeSet::new();
    for ty in infos.into_iter().flat_map(DlrlTypeInfo::value_types) {
        let Some(name) = ty.declared_name() else {
            continue;
        };
        match name.split_once("::") {
            Some((outermost, _)) => {
                names.insert(outermost.to_string());
            }
            None => {
                names.insert(name.to_string());
                if matches!(ty, DlrlMonoType::Struct { .. }) {
                    names.insert(format!("new{name}"));
                }
            }
        }
    }
    if names.is_empty() {
        return None;
    }
    let names: Vec<String> = names.into_iter().collect();
    Some(format!(
        "import {{ {} }} from \"{module}\";\n",
        names.join(", ")
    ))
}

/// The interface an application programs against.
#[must_use]
pub fn generate_ts_interface(info: &DlrlTypeInfo) -> String {
    let name = info.simple_name();
    let mut out = format!(
        "// Generated DLRL object — no OMG TypeScript PSM exists;\n\
         // this follows the Java PSM's shape (§B.8).\n\
         export interface {name} {{\n"
    );
    for key in &info.keys {
        let (ty, _) = ts_mono(info.key_type(key));
        out.push_str(&format!("  readonly {key}: {ty};\n"));
    }
    for (attribute, ty) in &info.attributes {
        match ty {
            DlrlAttributeType::Mono(mono) => {
                let (ty, _) = ts_mono(Some(mono));
                out.push_str(&format!("  {attribute}: {ty};\n"));
            }
            DlrlAttributeType::Multi { basis, item } => {
                let (ty, _) = ts_collection(basis, item);
                out.push_str(&format!("  readonly {attribute}: {ty};\n"));
            }
        }
    }
    for (relation, target) in &info.relations {
        let target_name = crate::simple_name(target);
        out.push_str(&format!("  readonly {relation}: {target_name}[];\n"));
    }
    out.push_str("}\n");
    out
}

/// The class the runtime instantiates.
#[must_use]
pub fn generate_ts_class(info: &DlrlTypeInfo) -> String {
    let name = info.simple_name();
    let mut out = format!("export class {name}Impl implements {name} {{\n");
    for key in &info.keys {
        match ts_mono(info.key_type(key)) {
            (ty, Some(initial)) => out.push_str(&format!("  readonly {key}: {ty} = {initial};\n")),
            (ty, None) => out.push_str(&format!("  declare readonly {key}: {ty};\n")),
        }
    }
    for (attribute, ty) in &info.attributes {
        match ty {
            DlrlAttributeType::Mono(mono) => {
                if let (ty, Some(initial)) = ts_mono(Some(mono)) {
                    out.push_str(&format!("  {attribute}: {ty} = {initial};\n"));
                }
            }
            DlrlAttributeType::Multi { basis, item } => {
                let (ty, initial) = ts_collection(basis, item);
                out.push_str(&format!("  readonly {attribute}: {ty} = {initial};\n"));
            }
        }
    }
    for (relation, target) in &info.relations {
        let target_name = crate::simple_name(target);
        out.push_str(&format!("  readonly {relation}: {target_name}[] = [];\n"));
    }
    out.push_str("}\n");
    out
}
