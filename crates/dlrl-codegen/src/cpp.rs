// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ PSM — DDS 1.4 §B.6.

use alloc::format;
use alloc::string::{String, ToString};

use crate::{DlrlAttributeType, DlrlCollection, DlrlFieldType, DlrlMonoType, DlrlTypeInfo};

/// A basic type's member type, getter return type and what the getter
/// appends to the member, in the classic IDL-to-C++ mapping the C++ PSM
/// uses (`DDS::Long`, a string held in `DDS::String_var` and returned as
/// `const char*`).
fn cpp_basic(ty: DlrlFieldType) -> (&'static str, &'static str, &'static str) {
    let scalar = |t: &'static str| (t, t, "");
    match ty {
        DlrlFieldType::Boolean => scalar("DDS::Boolean"),
        DlrlFieldType::Char => scalar("DDS::Char"),
        DlrlFieldType::Octet => scalar("DDS::Octet"),
        DlrlFieldType::Int8 => scalar("DDS::Int8"),
        DlrlFieldType::UInt8 => scalar("DDS::UInt8"),
        DlrlFieldType::Short => scalar("DDS::Short"),
        DlrlFieldType::UShort => scalar("DDS::UShort"),
        DlrlFieldType::Long => scalar("DDS::Long"),
        DlrlFieldType::ULong => scalar("DDS::ULong"),
        DlrlFieldType::LongLong => scalar("DDS::LongLong"),
        DlrlFieldType::ULongLong => scalar("DDS::ULongLong"),
        DlrlFieldType::Float => scalar("DDS::Float"),
        DlrlFieldType::Double => scalar("DDS::Double"),
        DlrlFieldType::String => ("DDS::String_var", "const char*", ".in()"),
    }
}

/// How one mono-valued type is spelled: the member, the getter's return
/// type, what the getter appends to the member, and the setter's
/// parameter. An enumeration or a structure is the type `idlc generate
/// --lang cpp` declares for it, by its fully qualified name; a structure
/// is returned and taken by `const&`.
struct Spelling {
    member: String,
    getter: String,
    read: &'static str,
    param: String,
}

fn cpp_mono(ty: Option<&DlrlMonoType>) -> Spelling {
    let plain = |t: String| Spelling {
        member: t.clone(),
        getter: t.clone(),
        read: "",
        param: t,
    };
    match ty {
        Some(DlrlMonoType::Basic(DlrlFieldType::String)) => Spelling {
            member: "DDS::String_var".to_string(),
            getter: "const char*".to_string(),
            read: ".in()",
            param: "const char*".to_string(),
        },
        Some(DlrlMonoType::Basic(basic)) => plain(cpp_basic(*basic).0.to_string()),
        Some(DlrlMonoType::Enum { name, .. }) => plain(format!("::{name}")),
        Some(DlrlMonoType::Struct { name }) => Spelling {
            member: format!("::{name}"),
            getter: format!("const ::{name}&"),
            read: "",
            param: format!("const ::{name}&"),
        },
        // An unresolved key names a type no header declares.
        None => plain("DLRL_UNRESOLVED_KEY_TYPE".to_string()),
    }
}

/// The collection a multi-valued attribute is held in: `DLRL::List`,
/// `DLRL::StrMap` or `DLRL::IntMap`, the templates behind the PSM's
/// `FooList`/`FooStrMap`/`FooIntMap` (DDS 1.2 §8.2.2.3.2.3 reads
/// `<templateDef name="BarStrMap" pattern="StrMap" itemType="Bar"/>` as
/// "a hypothetical typedef StrMap<Foo> FooStrMap"). `IntMap` carries its
/// key type.
fn cpp_collection(basis: &DlrlCollection, item: &DlrlMonoType) -> String {
    let item = cpp_mono(Some(item)).member;
    match basis {
        DlrlCollection::List => format!("DLRL::List< {item} >"),
        DlrlCollection::StrMap => format!("DLRL::StrMap< {item} >"),
        DlrlCollection::IntMap(key) => format!("DLRL::IntMap< {}, {item} >", cpp_basic(*key).0),
    }
}

/// The object class: derives from `ObjectRoot`, one member and getter per
/// key, one member, getter and setter per mono-valued attribute, one
/// collection and accessor per multi-valued attribute, one `RefList` and
/// accessor per relation. Every member is value-initialised.
#[must_use]
pub fn generate_cpp_object(info: &DlrlTypeInfo) -> String {
    let class = info.simple_name();
    let mut out = format!(
        "// Generated DLRL object — DDS 1.4 §B.6\n\
         class {class} : public DLRL::ObjectRoot {{\n\
         public:\n"
    );
    for key in &info.keys {
        let Spelling {
            member,
            getter,
            read,
            ..
        } = cpp_mono(info.key_type(key));
        out.push_str(&format!("    {member} {key}_{{}};\n"));
        out.push_str(&format!(
            "    {getter} get_{key}() const {{ return {key}_{read}; }}\n"
        ));
    }
    for (attribute, ty) in &info.attributes {
        match ty {
            DlrlAttributeType::Mono(mono) => {
                let Spelling {
                    member,
                    getter,
                    read,
                    param,
                } = cpp_mono(Some(mono));
                out.push_str(&format!("    {member} {attribute}_{{}};\n"));
                out.push_str(&format!(
                    "    {getter} get_{attribute}() const {{ return {attribute}_{read}; }}\n"
                ));
                out.push_str(&format!(
                    "    void set_{attribute}({param} v) {{ {attribute}_ = v; }}\n"
                ));
            }
            DlrlAttributeType::Multi { basis, item } => {
                let collection = cpp_collection(basis, item);
                out.push_str(&format!("    {collection} {attribute}_{{}};\n"));
                out.push_str(&format!(
                    "    {collection}& get_{attribute}() {{ return {attribute}_; }}\n"
                ));
            }
        }
    }
    for (relation, target) in &info.relations {
        let target_class = crate::simple_name(target);
        out.push_str(&format!(
            "    DLRL::RefList< {target_class}* > {relation}_;\n"
        ));
        out.push_str(&format!(
            "    DLRL::RefList< {target_class}* >& get_{relation}() {{ return {relation}_; }}\n"
        ));
    }
    out.push_str("};\n");
    out
}

/// The home class: §B.7 makes it the factory and the registry for its
/// object class.
#[must_use]
pub fn generate_cpp_home(info: &DlrlTypeInfo) -> String {
    let class = info.simple_name();
    format!(
        "// Generated DLRL home — DDS 1.4 §B.7\n\
         class {class}Home : public DLRL::ObjectHome {{\n\
         public:\n    \
             {class}* create_object() {{ return new {class}(); }}\n    \
             void delete_object({class}* o) {{ delete o; }}\n\
         }};\n"
    )
}
