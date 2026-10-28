// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java PSM — DDS 1.4 §B.8.

use alloc::format;
use alloc::string::{String, ToString};

use crate::{DlrlAttributeType, DlrlCollection, DlrlFieldType, DlrlMonoType, DlrlTypeInfo};

/// A basic type's Java type in the IDL-to-Java mapping (unsigned types
/// map to the signed type of the same width), and its boxed form for a
/// collection's type argument.
fn java_basic(ty: DlrlFieldType) -> (&'static str, &'static str) {
    match ty {
        DlrlFieldType::Boolean => ("boolean", "Boolean"),
        DlrlFieldType::Char => ("char", "Character"),
        DlrlFieldType::Octet | DlrlFieldType::Int8 | DlrlFieldType::UInt8 => ("byte", "Byte"),
        DlrlFieldType::Short | DlrlFieldType::UShort => ("short", "Short"),
        DlrlFieldType::Long | DlrlFieldType::ULong => ("int", "Integer"),
        DlrlFieldType::LongLong | DlrlFieldType::ULongLong => ("long", "Long"),
        DlrlFieldType::Float => ("float", "Float"),
        DlrlFieldType::Double => ("double", "Double"),
        DlrlFieldType::String => ("String", "String"),
    }
}

/// A mono-valued type in the mapping `idlc generate --lang java` uses for
/// the same IDL: an enumeration is an `int` (its generated class holds
/// the enumerators as `int` constants), a structure the class nested in
/// `ZdTypes` by module. The second element is the boxed form. An
/// unresolved key names a class that does not exist.
fn java_mono(ty: Option<&DlrlMonoType>) -> (String, String) {
    match ty {
        Some(DlrlMonoType::Basic(basic)) => {
            let (plain, boxed) = java_basic(*basic);
            (plain.to_string(), boxed.to_string())
        }
        Some(DlrlMonoType::Enum { .. }) => ("int".to_string(), "Integer".to_string()),
        Some(DlrlMonoType::Struct { name }) => {
            let class = format!("ZdTypes.{}", name.replace("::", "."));
            (class.clone(), class)
        }
        None => (
            "UnresolvedDlrlFieldType".to_string(),
            "UnresolvedDlrlFieldType".to_string(),
        ),
    }
}

/// A multi-valued attribute's field type and initial value: a
/// `java.util.List` for a list, a `java.util.Map` in insertion order for
/// either map.
fn java_collection(basis: &DlrlCollection, item: &DlrlMonoType) -> (String, &'static str) {
    let (_, item) = java_mono(Some(item));
    match basis {
        DlrlCollection::List => (
            format!("java.util.List<{item}>"),
            "new java.util.ArrayList<>()",
        ),
        DlrlCollection::StrMap => (
            format!("java.util.Map<String, {item}>"),
            "new java.util.LinkedHashMap<>()",
        ),
        DlrlCollection::IntMap(key) => (
            format!("java.util.Map<{}, {item}>", java_basic(*key).1),
            "new java.util.LinkedHashMap<>()",
        ),
    }
}

/// The object class. Java has no multiple inheritance, so §B.8 puts the
/// object behaviour in a base class and nothing else needs a mix-in.
#[must_use]
pub fn generate_java_object(info: &DlrlTypeInfo) -> String {
    let class = info.simple_name();
    let mut out = format!(
        "// Generated DLRL object — DDS 1.4 §B.8\n\
         public class {class} extends org.omg.dds.dlrl.ObjectRoot {{\n"
    );
    for key in &info.keys {
        let (ty, _) = java_mono(info.key_type(key));
        out.push_str(&format!("    private {ty} {key};\n"));
        out.push_str(&format!(
            "    public {ty} get{}() {{ return {key}; }}\n",
            capitalise(key)
        ));
    }
    for (attribute, ty) in &info.attributes {
        let name = capitalise(attribute);
        match ty {
            DlrlAttributeType::Mono(mono) => {
                let (ty, _) = java_mono(Some(mono));
                out.push_str(&format!("    private {ty} {attribute};\n"));
                out.push_str(&format!(
                    "    public {ty} get{name}() {{ return {attribute}; }}\n"
                ));
                out.push_str(&format!(
                    "    public void set{name}({ty} v) {{ {attribute} = v; }}\n"
                ));
            }
            DlrlAttributeType::Multi { basis, item } => {
                let (ty, initial) = java_collection(basis, item);
                out.push_str(&format!(
                    "    private final {ty} {attribute} = {initial};\n"
                ));
                out.push_str(&format!(
                    "    public {ty} get{name}() {{ return {attribute}; }}\n"
                ));
            }
        }
    }
    for (relation, target) in &info.relations {
        let target_class = crate::simple_name(target);
        out.push_str(&format!(
            "    private java.util.List<{target_class}> {relation} = new java.util.ArrayList<>();\n"
        ));
        out.push_str(&format!(
            "    public java.util.List<{target_class}> get{}() {{ return {relation}; }}\n",
            capitalise(relation)
        ));
    }
    out.push_str("}\n");
    out
}

/// The home class — §B.7.
#[must_use]
pub fn generate_java_home(info: &DlrlTypeInfo) -> String {
    let class = info.simple_name();
    format!(
        "// Generated DLRL home — DDS 1.4 §B.7\n\
         public class {class}Home extends org.omg.dds.dlrl.ObjectHome {{\n    \
             public {class} createObject() {{ return new {class}(); }}\n\
         }}\n"
    )
}

/// `venue` becomes `getVenue`, which is what a Java caller writes.
fn capitalise(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}
