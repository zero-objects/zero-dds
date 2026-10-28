// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C# mapping.
//!
//! DDS 1.4 Annex B has no C# PSM — the specification maps DLRL to C++,
//! Java and IDL. What is emitted here follows the Java shape, which is
//! the closest of the three, and says so in the generated file: a caller
//! reading it should know it is a mapping this implementation chose and
//! not one an OMG document fixed.

use alloc::format;
use alloc::string::{String, ToString};

use crate::{DlrlAttributeType, DlrlCollection, DlrlFieldType, DlrlMonoType, DlrlTypeInfo};

/// A basic type's C# type.
fn csharp_basic(ty: DlrlFieldType) -> &'static str {
    match ty {
        DlrlFieldType::Boolean => "bool",
        DlrlFieldType::Char => "char",
        DlrlFieldType::Octet | DlrlFieldType::UInt8 => "byte",
        DlrlFieldType::Int8 => "sbyte",
        DlrlFieldType::Short => "short",
        DlrlFieldType::UShort => "ushort",
        DlrlFieldType::Long => "int",
        DlrlFieldType::ULong => "uint",
        DlrlFieldType::LongLong => "long",
        DlrlFieldType::ULongLong => "ulong",
        DlrlFieldType::Float => "float",
        DlrlFieldType::Double => "double",
        DlrlFieldType::String => "string",
    }
}

/// A mono-valued type: an enumeration or a structure is the type `idlc
/// generate --lang csharp` declares for it (a module is a namespace), by
/// its `global::`-qualified name. An unresolved key names a type that
/// does not exist.
fn csharp_mono(ty: Option<&DlrlMonoType>) -> String {
    match ty {
        Some(DlrlMonoType::Basic(basic)) => csharp_basic(*basic).to_string(),
        Some(DlrlMonoType::Enum { name, .. } | DlrlMonoType::Struct { name }) => {
            format!("global::{}", name.replace("::", "."))
        }
        None => "UnresolvedDlrlFieldType".to_string(),
    }
}

/// A multi-valued attribute's property type.
fn csharp_collection(basis: &DlrlCollection, item: &DlrlMonoType) -> String {
    let item = csharp_mono(Some(item));
    match basis {
        DlrlCollection::List => format!("System.Collections.Generic.List<{item}>"),
        DlrlCollection::StrMap => {
            format!("System.Collections.Generic.Dictionary<string, {item}>")
        }
        DlrlCollection::IntMap(key) => format!(
            "System.Collections.Generic.Dictionary<{}, {item}>",
            csharp_basic(*key)
        ),
    }
}

/// The object class, as a partial class so an application can add to it
/// without editing generated code.
#[must_use]
pub fn generate_csharp_object(info: &DlrlTypeInfo) -> String {
    let class = info.simple_name();
    let mut out = format!(
        "// Generated DLRL object — mapped after the Java PSM (§B.8);\n\
         // DDS 1.4 Annex B has no C# PSM.\n\
         [DlrlObject]\n\
         public partial class {class} : ObjectRoot\n{{\n"
    );
    for key in &info.keys {
        // The attribute carries the IDL name. The property is
        // PascalCase because that is what a C# caller expects, and that
        // rename is what makes the attribute load-bearing: the DCPS
        // topic's key is `venue`, and a class that only knows `Venue`
        // cannot be matched back to it.
        out.push_str(&format!(
            "    [DlrlKey(\"{key}\")] public {} {} {{ get; set; }}\n",
            csharp_mono(info.key_type(key)),
            capitalise(key)
        ));
    }
    for (attribute, ty) in &info.attributes {
        // Named after the metamodel's `MonoAttribute` and `MultiAttribute`
        // (§8.1.4.4.2, §8.1.4.4.3), and carrying the IDL name for the same
        // reason a key does.
        match ty {
            DlrlAttributeType::Mono(mono) => out.push_str(&format!(
                "    [DlrlMonoAttribute(\"{attribute}\")] public {} {} {{ get; set; }}\n",
                csharp_mono(Some(mono)),
                capitalise(attribute)
            )),
            DlrlAttributeType::Multi { basis, item } => out.push_str(&format!(
                "    [DlrlMultiAttribute(\"{attribute}\")] public {} {} {{ get; }} = new();\n",
                csharp_collection(basis, item),
                capitalise(attribute)
            )),
        }
    }
    for (relation, target) in &info.relations {
        let target_class = crate::simple_name(target);
        out.push_str(&format!(
            "    [DlrlRelation(\"{relation}\")] public System.Collections.Generic.List<{target_class}> {} {{ get; }} = new();\n",
            capitalise(relation)
        ));
    }
    out.push_str("}\n");
    out
}

/// The home class — §B.7's role, in the same mapping.
#[must_use]
pub fn generate_csharp_home(info: &DlrlTypeInfo) -> String {
    let class = info.simple_name();
    format!(
        "// Generated DLRL home — mapped after the Java PSM (§B.7).\n\
         public partial class {class}Home : ObjectHome\n{{\n    \
             public {class} CreateObject() => new {class}();\n\
         }}\n"
    )
}

/// C# properties are `PascalCase`, so a key written `venue` in IDL is
/// `Venue` here. The IDL name stays reachable through the attribute,
/// which carries it verbatim.
fn capitalise(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}
