// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Typed DDS companion for the existing Java wire codec, with no second serializer.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use zerodds_ast::facet::Extensibility;
use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey, PidFlagPolicy};
use zerodds_lang_cst::common::{Doc, Ident};
use zerodds_lang_cst::java;
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, key_holder};
use zerodds_lang_rules::emit::java::Java;

/// Emit `ZdTypeSupports.java`, separately from wire-only `ZdTypes`/`ZdCodec`.
/// A codec build implements one representation, and both byte orders. Topic
/// support is omitted for enum-only files and explicitly nested aggregates.
pub fn source(ast: &Ast, encoding: Encoding, key: LayoutKey) -> Result<Option<String>, String> {
    let mut bodies = Vec::new();
    let mut names = BTreeSet::new();
    // The common types of DDS-RPC 1.0 §7.5.1.1.1 and the request and reply
    // types of a pair (§7.3.1.4) are served by the RPC binding.
    let by_rpc_binding = zerodds_lang_rules::emit::type_pair_cpp::excluded_types(ast);
    for (id, node) in ast.types() {
        if by_rpc_binding.contains(&id)
            || node.name.is_empty()
            || !matches!(
                node.kind,
                TypeKind::Structure { .. } | TypeKind::Union { .. }
            )
            || node.facets.xtypes.as_ref().is_some_and(|x| x.is_nested)
        {
            continue;
        }
        let tag =
            zerodds_lang_rules::adapter::java::type_tag(ast, id, 0).map_err(|e| e.to_string())?;
        let name = format!("Support_{tag}");
        if !names.insert(name.clone()) {
            return Err(format!("generated Java support name collision: {name}"));
        }
        bodies.push(support(ast, id, encoding, key, &name)?);
    }
    if bodies.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "// Generated typed DDS adapters. Do not edit.\n\
         // Compile with ZdTypes.java, ZdCodec.java, the wire runtime and the DDS Java binding.\n\
         import org.zerodds.wire.*;\n\
         public final class ZdTypeSupports {{\n\
         private ZdTypeSupports() {{}}\n{}\n}}\n",
        bodies.join("\n")
    )))
}

fn quoted(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
    )
}

/// `policy` as the `org.zerodds.wire.PidFlagPolicy` enum constant
/// `PlCdr1Framing`'s constructor takes.
///
/// `runtime/java/src/main/java/org/zerodds/wire/PidFlagPolicy.java`
/// names its three constants in `SCREAMING_SNAKE_CASE`; the match below
/// is exhaustive over [`PidFlagPolicy`], so a fourth policy fails this
/// crate's own build rather than falling back to a wrong constant.
const fn java_pid_flags(policy: PidFlagPolicy) -> &'static str {
    match policy {
        PidFlagPolicy::SentinelMustUnderstand => "PidFlagPolicy.SENTINEL_MUST_UNDERSTAND",
        PidFlagPolicy::Bare => "PidFlagPolicy.BARE",
        PidFlagPolicy::ImplExtensionOnMembers => "PidFlagPolicy.IMPL_EXTENSION_ON_MEMBERS",
    }
}

fn standard_type_information(ast: &Ast, id: Id<TypeNode>) -> Result<Vec<u8>, String> {
    Ok(standard_metadata(ast, id)?.0)
}

use crate::type_metadata::standard_metadata;

fn java_bytes(bytes: &[u8]) -> String {
    // Array literals emit one JVM instruction sequence per byte and make a
    // normal dependency closure exceed the 64 KiB method limit. Hex constants
    // preserve the measured bytes without generating initialization bytecode
    // for every octet. Keep individual constants below the JVM UTF8 limit too.
    let chunks = bytes
        .chunks(8192)
        .map(|chunk| {
            let hex = chunk
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            quoted(&hex)
        })
        .collect::<Vec<_>>();
    let value = match chunks.as_slice() {
        [] => "\"\"".to_string(),
        [only] => only.clone(),
        _ => format!("String.join(\"\", {})", chunks.join(", ")),
    };
    format!("org.zerodds.cdr.HexBytes.decode({value})")
}

/// Immutable service data, not pre-encoded replies or advertised service support.
fn lookup_registry(ast: &Ast, id: Id<TypeNode>) -> Result<String, String> {
    use zerodds_types::{TypeIdentifier, type_object::TypeObject};
    let (_, objects) = standard_metadata(ast, id)?;
    let mut mappings = std::collections::BTreeMap::new();
    for (candidate, _) in ast.types() {
        let Ok(complete) = zerodds_types::from_ast::complete_type_object(ast, candidate) else {
            continue;
        };
        let Ok(minimal) = zerodds_types::from_ast::minimal_type_object(ast, candidate) else {
            continue;
        };
        let complete_hash = zerodds_types::hash::compute_complete_hash(&complete)
            .map_err(|e| format!("complete hash: {e:?}"))?;
        let minimal_hash = zerodds_types::hash::compute_minimal_hash(&minimal)
            .map_err(|e| format!("minimal hash: {e:?}"))?;
        if let Some(previous) = mappings.insert(complete_hash, minimal_hash)
            && previous != minimal_hash
        {
            return Err("ambiguous complete-to-minimal mapping".into());
        }
    }
    let mut entries = Vec::new();
    let mut pairs = Vec::new();
    for object in objects {
        let hash =
            zerodds_types::hash::compute_hash(&object).map_err(|e| format!("hash: {e:?}"))?;
        let type_id = match &object {
            TypeObject::Minimal(_) => TypeIdentifier::EquivalenceHashMinimal(hash),
            TypeObject::Complete(_) => TypeIdentifier::EquivalenceHashComplete(hash),
            _ => return Err("unsupported TypeObject kind in Java registry".into()),
        };
        let identifier = type_id
            .to_bytes_le()
            .map_err(|e| format!("identifier: {e:?}"))?;
        let bytes = object.to_bytes_le().map_err(|e| format!("object: {e:?}"))?;
        let dependencies = zerodds_types::resolve::collect_referenced_hashes(
            &object,
            zerodds_types::resolve::DEFAULT_MAX_RESOLVE_DEPTH,
        )
        .map_err(|e| format!("direct dependencies: {e:?}"))?
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|h| {
            let dependency = match &object {
                TypeObject::Minimal(_) => TypeIdentifier::EquivalenceHashMinimal(h),
                TypeObject::Complete(_) => TypeIdentifier::EquivalenceHashComplete(h),
                _ => return Err("unsupported dependency object kind".to_string()),
            };
            dependency
                .to_bytes_le()
                .map(|bytes| java_bytes(&bytes))
                .map_err(|e| format!("dependency identifier: {e:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
        if matches!(&object, TypeObject::Complete(_)) {
            let minimal_hash = mappings
                .get(&hash)
                .ok_or("complete object has no minimal mapping")?;
            let minimal_id = TypeIdentifier::EquivalenceHashMinimal(*minimal_hash)
                .to_bytes_le()
                .map_err(|e| format!("minimal identifier: {e:?}"))?;
            pairs.push(format!(
                "new org.zerodds.cdr.TypeObjectRegistry.IdentifierPair({}, {})",
                java_bytes(&identifier),
                java_bytes(&minimal_id)
            ));
        }
        entries.push(format!(
            "new org.zerodds.cdr.TypeObjectRegistry.Entry({}, {}, new byte[][] {{ {} }})",
            java_bytes(&identifier),
            java_bytes(&bytes),
            dependencies.join(", ")
        ));
    }
    Ok(format!(
        "return new org.zerodds.cdr.TypeObjectRegistry(new org.zerodds.cdr.TypeObjectRegistry.Entry[] {{ {} }}, new org.zerodds.cdr.TypeObjectRegistry.IdentifierPair[] {{ {} }});",
        entries.join(", "),
        pairs.join(", ")
    ))
}

fn support(
    ast: &Ast,
    id: Id<TypeNode>,
    encoding: Encoding,
    key: LayoutKey,
    name: &str,
) -> Result<String, String> {
    let path =
        zerodds_lang_rules::adapter::java::declared_path(ast, id).map_err(|e| e.to_string())?;
    let scoped = ast
        .any_of_type(id)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))
        .ok_or_else(|| format!("no scoped DDS name for {path}"))?;
    let type_name = scoped.segments().join("::");
    let type_literal = quoted(&type_name);
    let encoder = zerodds_lang_rules::emit::encode::codec_name::<Java>(
        ast,
        id,
        zerodds_ast_layout::key::Purpose::Data,
    );
    let decoder = zerodds_lang_rules::emit::decode::decoder_name::<Java>(ast, id);
    // DDS DataRepresentationId values, not a runtime-specific version enum.
    let representation = if encoding.is_v1() { 0 } else { 2 };
    let alignment = encoding.max_alignment();
    let headers = key.pl_cdr1_headers;
    let short_codes = key.length_codes.short_code_where_it_fits();
    let framing = format!(
        "new PlCdr1Framing({pid_flags}, {highest_short_id}, {pad_declared}, {pad_optional}, \
         {member_must_understand}, {pad_after})",
        pid_flags = java_pid_flags(key.pid_flags),
        highest_short_id = key.short_pid_range.highest_short_id(),
        pad_declared = headers.pads_declared_length(),
        pad_optional = headers.pads_declared_optional_length(),
        member_must_understand = key.member_pid_carries_must_understand(),
        pad_after = headers.pads_after_value(),
    );
    let extensibility = match ast.effective_extensibility(id) {
        Extensibility::Final => "FINAL",
        Extensibility::Appendable => "APPENDABLE",
        Extensibility::Mutable => "MUTABLE",
    };
    let (keyed, key_codecs, key_body) = key_support(ast, id)?;
    let registry = lookup_registry(ast, id).unwrap_or_else(|reason| {
        format!(
            "throw new UnsupportedOperationException({});",
            quoted(&format!(
                "{type_name}: TypeLookup registry unavailable: {reason}"
            ))
        )
    });
    let standard_metadata = match standard_type_information(ast, id) {
        Ok(bytes) => format!(
            "return new byte[] {{ {} }};",
            bytes
                .iter()
                .map(|b| format!("(byte)0x{b:02x}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Err(reason) => format!(
            "throw new UnsupportedOperationException({});",
            quoted(&format!(
                "{type_name}: standard TypeInformation unavailable: {reason}"
            ))
        ),
    };
    let metadata = match zerodds_types::from_ast::complete_type_object(ast, id) {
        Ok(object) => {
            let object = zerodds_types::type_object::TypeObject::Complete(object);
            let bytes = object
                .to_bytes_le()
                .map_err(|e| format!("{type_name}: {e:?}"))?;
            zerodds_types::type_object::TypeObject::from_bytes_le(&bytes)
                .map_err(|e| format!("{type_name}: invalid emitted TypeObject: {e:?}"))?;
            let octets = bytes
                .iter()
                .map(|b| format!("(byte)0x{b:02x}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("return new byte[] {{ {octets} }};")
        }
        Err(reason) => format!(
            "throw new UnsupportedOperationException({});",
            quoted(&format!(
                "{type_name}: CompleteTypeObject unavailable: {reason:?}"
            ))
        ),
    };
    let mut out = String::new();
    let _ = write!(
        out,
        r#"
public static final class {name} extends org.omg.dds.type.TypeSupport<{path}>
        implements org.zerodds.cdr.TopicTypeSupport<{path}> {{
    public static final {name} INSTANCE = new {name}();
    private {name}() {{}}
    // DDS-Java-PSM 1.0: org.omg.dds.type.TypeSupport is the base class of every TypeSupport (DDSObject, newData, getType)
    private volatile org.omg.dds.core.ServiceEnvironment environment;
    public org.omg.dds.core.ServiceEnvironment getEnvironment() {{
        org.omg.dds.core.ServiceEnvironment e = environment;
        if (e == null) {{
            e = org.omg.dds.core.ServiceEnvironment.createInstance({name}.class.getClassLoader());
            environment = e;
        }}
        return e;
    }}
    public {path} newData() {{ return new {path}(); }}
    public Class<{path}> getType() {{ return {path}.class; }}
    public String getTypeName() {{ return {type_literal}; }}
    public boolean isKeyed() {{ return {keyed}; }}
    public org.zerodds.cdr.ExtensibilityKind getExtensibility() {{
        return org.zerodds.cdr.ExtensibilityKind.{extensibility};
    }}
    public int writeRepresentation() {{ return {representation}; }}
    public int[] readRepresentations() {{ return new int[] {{ {representation} }}; }}
    // Source-derived metadata. This method alone does not publish standard TypeInformation.
    public byte[] typeObject() {{ {metadata} }}
    // Shared XTypes serialization, not the historical vendor hash. No TypeLookup service is implied.
    public byte[] typeInformation() {{ {standard_metadata} }}
    public org.zerodds.cdr.TypeObjectRegistry typeObjects() {{ {registry} }}

    private static Endianness order(org.zerodds.cdr.EndianMode endian) {{
        java.util.Objects.requireNonNull(endian, "endian");
        switch (endian) {{
            case LITTLE_ENDIAN: return Endianness.LITTLE;
            case BIG_ENDIAN: return Endianness.BIG;
            default: throw new IllegalArgumentException("Unsupported byte order");
        }}
    }}
    private static void requireRepresentation(int representation) {{
        if (representation != {representation})
            throw new org.zerodds.cdr.XcdrException("Generated codec does not implement this representation");
    }}
    public byte[] encode({path} sample) {{
        return encode(sample, org.zerodds.cdr.EndianMode.LITTLE_ENDIAN, {representation});
    }}
    public byte[] encode({path} sample, org.zerodds.cdr.EndianMode endian) {{
        return encode(sample, endian, {representation});
    }}
    public byte[] encode({path} sample, org.zerodds.cdr.EndianMode endian, int representation) {{
        requireRepresentation(representation);
        java.util.Objects.requireNonNull(sample, "sample");
        Writer w = new Writer(order(endian), {alignment}, {framing}, {short_codes});
        ZdCodec.{encoder}(w, sample);
        return w.finish();
    }}
    public {path} decode(byte[] bytes) {{ return decode(bytes, 0, bytes.length); }}
    public {path} decode(byte[] bytes, int offset, int length) {{
        return decode(bytes, offset, length, org.zerodds.cdr.EndianMode.LITTLE_ENDIAN, {representation});
    }}
    public {path} decode(byte[] bytes, int offset, int length,
            org.zerodds.cdr.EndianMode endian, int representation) {{
        requireRepresentation(representation);
        if (offset < 0 || length < 0 || offset > bytes.length - length)
            throw new IndexOutOfBoundsException("Invalid serialized sample range");
        Reader r = new Reader(java.util.Arrays.copyOfRange(bytes, offset, offset + length),
                order(endian), {alignment}, {framing});
        {path} sample = ZdCodec.{decoder}(r, TryConstruct.DISCARD);
        if (r.readerStatus() != Reader.Status.OK || r.remaining() != 0)
            throw new org.zerodds.cdr.XcdrException("Generated decoder refused or did not consume the complete sample");
        return sample;
    }}
    public byte[] keyHash({path} sample) {{
        java.util.Objects.requireNonNull(sample, "sample");
        {key_body}
    }}
{key_codecs}
}}
"#
    );
    Ok(out)
}

fn key_support(ast: &Ast, id: Id<TypeNode>) -> Result<(bool, String, String), String> {
    let key = LayoutKey::key(Encoding::PlainCdr2);
    let Some(mut holder) = key_holder::<Java>(ast, id, key).map_err(|e| e.to_string())? else {
        return Ok((false, String::new(), "return new byte[16];".to_string()));
    };
    let java::Member::Method(ref mut method) = holder else {
        return Err("shared Java key holder is not a method".to_string());
    };
    let sample = method
        .params
        .get_mut(1)
        .ok_or("key holder has no sample parameter")?;
    sample.type_ = Java::value_type(ast, id);
    let holder_name = method.name.as_str().to_string();
    let mut members =
        codec_from::<Java>(ast, id, key, CodecOptions::default()).map_err(|e| e.to_string())?;
    // Nested keyless aggregates can require the full-value data walk too.
    members.extend(
        codec_from::<Java>(
            ast,
            id,
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Big),
            CodecOptions::default(),
        )
        .map_err(|e| e.to_string())?,
    );
    members.push(holder);
    let key_codecs = zerodds_lang_render::java::type_decl(&java::TypeDecl::Class(java::Class {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![
            java::Modifier::Private,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        name: Ident::new("KeyCodec"),
        generics: Vec::new(),
        extends: None,
        implements: Vec::new(),
        members,
    }));
    let finish = match zerodds_ast_layout::keyhash::key_hash_form_for(ast, id, key)
        .map_err(|e| format!("key maximum cannot be planned: {e:?}"))?
    {
        zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "zeroPadded",
        zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "md5",
    };
    Ok((
        true,
        key_codecs,
        format!(
            "Writer w = new Writer(Endianness.BIG, 4, PlCdr1Framing.defaults(), false);\n        KeyCodec.{holder_name}(w, sample);\n        return w.{finish}();"
        ),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn model(idl: &str) -> Ast {
        let composed = zerodds_idl_compose::compose_source(
            "java-support.idl",
            idl,
            &zerodds_idl_compose::PathLoader::bare(),
        );
        assert!(!composed.failed(), "{:?}", composed.diagnostics);
        composed.ast
    }

    /// The specification's key, little endian, for a test that has no
    /// dialect of its own to reproduce.
    fn key_for(encoding: Encoding) -> LayoutKey {
        LayoutKey::data(encoding, Endianness::Little)
    }

    #[test]
    fn standard_metadata_contains_complete_transitive_dependencies() {
        let ast = model(
            "@final struct Leaf { long id; }; @final struct Middle { Leaf value; }; @final struct Root { Middle left; sequence<Leaf, 2> right; };",
        );
        let root = ast.types().find(|(_, n)| n.name == "Root").expect("Root").0;
        let bytes = standard_type_information(&ast, root).expect("standard metadata");
        let info = zerodds_types::TypeInformation::from_bytes_le(&bytes).expect("shared decoder");
        assert_eq!(info.minimal.dependent_typeid_count, 2);
        assert_eq!(info.complete.dependent_typeid_count, 2);
        let (_, objects) = standard_metadata(&ast, root).expect("registry closure");
        assert_eq!(objects.len(), 6);
        for object in objects {
            let bytes = object.to_bytes_le().expect("object bytes");
            assert_eq!(
                zerodds_types::type_object::TypeObject::from_bytes_le(&bytes).expect("decode"),
                object
            );
        }
        for name in ["Leaf", "Middle"] {
            let id = ast
                .types()
                .find(|(_, n)| n.name == name)
                .expect("dependency")
                .0;
            let minimal = zerodds_types::from_ast::minimal_type_object(&ast, id).expect("minimal");
            let complete =
                zerodds_types::from_ast::complete_type_object(&ast, id).expect("complete");
            assert!(info.minimal.dependent_typeids.iter().any(|d| d.type_id
                == zerodds_types::TypeIdentifier::EquivalenceHashMinimal(
                    zerodds_types::hash::compute_minimal_hash(&minimal).expect("hash")
                )
                && d.typeobject_serialized_size
                    == zerodds_types::hash::minimal_serialized_size(&minimal).expect("size")));
            assert!(info.complete.dependent_typeids.iter().any(|d| d.type_id
                == zerodds_types::TypeIdentifier::EquivalenceHashComplete(
                    zerodds_types::hash::compute_complete_hash(&complete).expect("hash")
                )
                && d.typeobject_serialized_size
                    == zerodds_types::hash::complete_serialized_size(&complete).expect("size")));
        }
        let leaf = ast.types().find(|(_, n)| n.name == "Leaf").expect("Leaf").0;
        let leaf = zerodds_types::TypeInformation::from_bytes_le(
            &standard_type_information(&ast, leaf).expect("leaf metadata"),
        )
        .expect("leaf decode");
        assert_eq!(leaf.minimal.dependent_typeid_count, 0);
        assert_eq!(leaf.complete.dependent_typeid_count, 0);
        assert!(
            zerodds_types::TypeInformation::from_bytes_le(&bytes[..bytes.len() - 1]).is_err(),
            "truncated metadata must not decode"
        );
    }

    #[test]
    fn recursive_standard_metadata_is_unavailable_not_a_false_empty_closure() {
        let ast = model("@final struct Node { sequence<Node, 1> children; };");
        let root = ast.types().find(|(_, n)| n.name == "Node").expect("Node").0;
        assert!(
            standard_type_information(&ast, root).is_err(),
            "SCC metadata is outside this hash-only collector contract"
        );
    }

    #[test]
    fn the_framing_follows_the_keys_pid_flags_under_a_non_default_dialect() {
        // Regression: `source` used to take only `Encoding`, so
        // `Writer`/`Reader` were always built with `PlCdr1Framing.defaults()`
        // — a module generated under `--dialect opendds` still encoded and
        // decoded the specification's framing end to end.
        let ast = model("@mutable struct S { @key long id; };");
        let key = LayoutKey::in_dialect(
            zerodds_ast_layout::Dialect::OpenDds,
            Encoding::PlCdr1,
            Endianness::Little,
            zerodds_ast_layout::key::Purpose::Data,
        );
        assert_eq!(key.pid_flags, PidFlagPolicy::ImplExtensionOnMembers);
        let text = source(&ast, Encoding::PlCdr1, key)
            .expect("generate")
            .expect("support");
        assert!(
            text.contains("PidFlagPolicy.IMPL_EXTENSION_ON_MEMBERS"),
            "{text}"
        );
        // The KeyHash writer stays the specification's: DDSI-RTPS 2.5
        // §9.6.3.8 fixes the algorithm, which a dialect does not reach.
        assert!(text.contains("new Writer(Endianness.BIG, 4, PlCdr1Framing.defaults(), false)"));
    }

    #[test]
    fn companion_delegates_data_and_uses_shared_key_and_metadata() {
        let ast = model("module m { @final struct S { @key long id; }; };");
        let text = source(&ast, Encoding::PlainCdr2, key_for(Encoding::PlainCdr2))
            .expect("generate")
            .expect("support");
        assert!(text.contains("TopicTypeSupport<ZdTypes.m.S>"));
        assert!(text.contains("return \"m::S\""));
        assert!(text.contains("ZdCodec.zd_enc_"));
        assert!(text.contains("return w.zeroPadded();"));
        assert!(text.contains("public byte[] typeObject() { return new byte[]"));
        assert!(text.contains("public byte[] typeInformation() { return new byte[]"));
        assert!(text.contains("Reader.Status.OK || r.remaining() != 0"));
    }

    #[test]
    fn representation_is_dds_zero_or_two_not_an_ordinal() {
        let ast = model("@appendable struct S { long id; };");
        for (encoding, repr, alignment) in
            [(Encoding::PlainCdr1, 0, 8), (Encoding::PlainCdr2, 2, 4)]
        {
            let text = source(&ast, encoding, key_for(encoding))
                .expect("generate")
                .expect("support");
            assert!(text.contains(&format!("writeRepresentation() {{ return {repr}; }}")));
            assert!(text.contains(&format!("if (representation != {repr})")));
            assert!(text.contains(&format!("order(endian), {alignment},")));
            assert!(text.contains("ExtensibilityKind.APPENDABLE"));
        }
    }

    #[test]
    fn key_hash_selection_uses_maximum_and_nested_topics_are_omitted() {
        let ast = model("@final struct S { @key string<32> name; };");
        assert!(
            source(&ast, Encoding::PlainCdr2, key_for(Encoding::PlainCdr2))
                .expect("generate")
                .expect("support")
                .contains("return w.md5();")
        );
        for idl in ["@nested struct S { long id; };", "enum E { A, B };"] {
            assert!(
                source(
                    &model(idl),
                    Encoding::PlainCdr2,
                    key_for(Encoding::PlainCdr2)
                )
                .expect("generate")
                .is_none()
            );
        }
    }

    #[test]
    fn source_metadata_decodes_to_the_shared_complete_object() {
        let ast = model("module m { @final struct S { long value; }; };");
        let text = source(&ast, Encoding::PlainCdr2, key_for(Encoding::PlainCdr2))
            .expect("generate")
            .expect("support");
        let bytes = text
            .split("public byte[] typeObject() { return new byte[] { ")
            .nth(1)
            .expect("metadata")
            .split(" };")
            .next()
            .expect("array")
            .split(", ")
            .map(|s| u8::from_str_radix(s.trim_start_matches("(byte)0x"), 16).expect("octet"))
            .collect::<Vec<_>>();
        let decoded =
            zerodds_types::type_object::TypeObject::from_bytes_le(&bytes).expect("valid object");
        let (id, _) = ast.types().find(|(_, t)| t.name == "S").expect("S");
        let expected = zerodds_types::type_object::TypeObject::Complete(
            zerodds_types::from_ast::complete_type_object(&ast, id).expect("lowerable S"),
        );
        assert_eq!(decoded, expected);
    }

    #[test]
    fn flattened_support_name_collisions_are_not_silently_overwritten() {
        let ast = model(
            "module a_b { @final struct C { long x; }; }; module a { @final struct b_C { long y; }; };",
        );
        assert!(source(&ast, Encoding::PlainCdr2, key_for(Encoding::PlainCdr2)).is_err());
    }
}
