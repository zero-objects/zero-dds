// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The current C# codec connected to the public typed DDS binding.
//!
//! A separate companion keeps the main generated file usable with only the
//! pure wire runtime. The companion requires ZeroDDS.Cdr and declares the
//! representation it actually implements, instead of accepting a different
//! wire representation through a permissive default interface method.

use std::fmt::Write as _;
use zerodds_ast::facet::Extensibility;
use zerodds_ast::{AnyId, Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey, PidFlagPolicy};
use zerodds_lang_cst::common::{Doc, Ident};
use zerodds_lang_cst::csharp;
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::csharp::CSharp;
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, key_holder};

/// A companion for all eligible topic aggregates, or none for an enum-only
/// file. Planning failures are generation errors, never an invented key hash.
pub fn source(ast: &Ast, encoding: Encoding, key: LayoutKey) -> Result<Option<String>, String> {
    let mut bodies = Vec::new();
    for (id, node) in ast.types() {
        if node.name.is_empty()
            || !matches!(
                node.kind,
                TypeKind::Structure { .. } | TypeKind::Union { .. }
            )
            || node.facets.xtypes.as_ref().is_some_and(|x| x.is_nested)
        {
            continue;
        }
        bodies.push(support(ast, id, node, encoding, key)?);
    }
    if bodies.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "// Generated typed DDS support. Do not edit.\n\
         // Compile with the matching generated codec, ZeroDds.Wire and ZeroDDS.Cdr.\n\
         #nullable enable\n{}\n",
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

/// `policy` as the `global::ZeroDds.Wire.PidFlagPolicy` enum literal the
/// runtime's `PlCdr1Framing` constructor actually takes.
///
/// Not `quoted(policy.as_str())`: the constructor's first parameter is
/// the enum (`runtime/csharp/src/PlCdr1Framing.cs`), not a string it
/// parses, so a string literal there was a value of the wrong type —
/// `CS1503` from every generated companion whose dialect was anything
/// but the default, since `SentinelMustUnderstand` is
/// `PidFlagPolicy::default()` and the mismatch is invisible until a
/// caller reaches for a second one. The three names match
/// [`PidFlagPolicy`]'s own variants exactly, which is what makes the
/// match exhaustive rather than a fourth spelling to keep in step.
const fn csharp_pid_flags(policy: PidFlagPolicy) -> &'static str {
    match policy {
        PidFlagPolicy::SentinelMustUnderstand => {
            "global::ZeroDds.Wire.PidFlagPolicy.SentinelMustUnderstand"
        }
        PidFlagPolicy::Bare => "global::ZeroDds.Wire.PidFlagPolicy.Bare",
        PidFlagPolicy::ImplExtensionOnMembers => {
            "global::ZeroDds.Wire.PidFlagPolicy.ImplExtensionOnMembers"
        }
    }
}

fn support(
    ast: &Ast,
    id: Id<TypeNode>,
    node: &TypeNode,
    encoding: Encoding,
    key: LayoutKey,
) -> Result<String, String> {
    let path =
        zerodds_lang_rules::adapter::csharp::declared_path(ast, id).map_err(|e| e.to_string())?;
    let plain_path = path
        .strip_prefix("global::")
        .ok_or("C# type path is not rooted")?;
    let (namespace, name) = plain_path.rsplit_once('.').unwrap_or(("", plain_path));
    let support_name = format!("{}TypeSupport", name.trim_start_matches('@'));
    let support_path = if namespace.is_empty() {
        format!("global::{support_name}")
    } else {
        format!("global::{namespace}.{support_name}")
    };
    for (other, _) in ast.types() {
        if zerodds_lang_rules::adapter::csharp::declared_path(ast, other)
            .is_ok_and(|candidate| candidate == support_path)
        {
            return Err(format!(
                "generated C# support name collides with {support_path}"
            ));
        }
    }
    // Exceptions and interfaces emit declarations too, but are not in types().
    // A module occupies the same C# namespace declaration space as a class.
    let other_declarations = ast
        .exceptions()
        .map(|(id, node)| AnyId::Exception(id, node.node_id))
        .chain(
            ast.interfaces()
                .map(|(id, node)| AnyId::Interface(id, node.node_id)),
        )
        .chain(
            ast.modules()
                .map(|(id, node)| AnyId::Module(id, node.node_id)),
        );
    for other in other_declarations {
        if ast.scoped_name_of(other).is_some_and(|scoped| {
            let segments = scoped
                .segments()
                .iter()
                .map(|segment| csharp::escape_keyword(segment))
                .collect::<Vec<_>>();
            format!("global::{}", segments.join(".")) == support_path
        }) {
            return Err(format!(
                "generated C# support name collides with {support_path}"
            ));
        }
    }
    let scoped = ast
        .any_of_type(id)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))
        .ok_or_else(|| format!("no scoped name for {}", node.name))?;
    let type_name = scoped.segments().join("::");
    let encoder = zerodds_lang_rules::emit::encode::codec_name::<CSharp>(
        ast,
        id,
        zerodds_ast_layout::key::Purpose::Data,
    );
    let decoder = zerodds_lang_rules::emit::decode::decoder_name::<CSharp>(ast, id);
    let representation = if encoding.is_v1() { 0 } else { 1 };
    let alignment = encoding.max_alignment();
    let headers = key.pl_cdr1_headers;
    let short_codes = key.length_codes.short_code_where_it_fits();
    let framing = format!(
        "new global::ZeroDds.Wire.PlCdr1Framing({pid_flags}, {highest_short_id}, \
         {pad_declared}, {pad_optional}, {member_must_understand}, {pad_after})",
        pid_flags = csharp_pid_flags(key.pid_flags),
        highest_short_id = key.short_pid_range.highest_short_id(),
        pad_declared = headers.pads_declared_length(),
        pad_optional = headers.pads_declared_optional_length(),
        member_must_understand = key.member_pid_carries_must_understand(),
        pad_after = headers.pads_after_value(),
    );
    let extensibility = match ast.effective_extensibility(id) {
        Extensibility::Final => "Final",
        Extensibility::Appendable => "Appendable",
        Extensibility::Mutable => "Mutable",
    };
    let (key_members, key_codecs, hash_body) = key_support(ast, id)?;
    let metadata = match zerodds_types::from_ast::complete_type_object(ast, id) {
        Ok(object) => {
            let object = zerodds_types::type_object::TypeObject::Complete(object);
            let bytes = object
                .to_bytes_le()
                .map_err(|e| format!("{type_name}: {e:?}"))?;
            zerodds_types::type_object::TypeObject::from_bytes_le(&bytes)
                .map_err(|e| format!("{type_name}: emitted TypeObject is invalid: {e:?}"))?;
            let octets = bytes
                .iter()
                .map(|b| format!("0x{b:02x}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("new byte[] {{ {octets} }}")
        }
        Err(reason) => format!(
            "throw new global::System.NotSupportedException({})",
            quoted(&format!("{type_name}: no CompleteTypeObject: {reason:?}"))
        ),
    };
    let mut out = String::new();
    if !namespace.is_empty() {
        let _ = writeln!(out, "namespace {namespace} {{");
    }
    let _ = write!(
        out,
        r#"
public sealed class {support_name} : global::ZeroDDS.Cdr.IDdsTopicType<{path}>, global::ZeroDDS.Cdr.IGeneratedWireTypeSupport
{{
    public static readonly {support_name} Instance = new();
    public string TypeName => {type_literal};
    public bool IsKeyed => {key_members};
    public global::Omg.Types.ExtensibilityKind Extensibility => global::Omg.Types.ExtensibilityKind.{extensibility};
    public int Representation => {representation};
    public byte[] TypeObject => {metadata};

    private static global::ZeroDds.Wire.Endianness Order(global::ZeroDDS.Cdr.EndianMode endian) => endian switch
    {{
        global::ZeroDDS.Cdr.EndianMode.LittleEndian => global::ZeroDds.Wire.Endianness.Little,
        global::ZeroDDS.Cdr.EndianMode.BigEndian => global::ZeroDds.Wire.Endianness.Big,
        _ => throw new global::System.ArgumentOutOfRangeException(nameof(endian))
    }};

    private static void RequireRepresentation(int representation)
    {{
        if (representation != {representation})
            throw new global::System.NotSupportedException("This generated codec does not implement the requested XCDR representation.");
    }}

    public byte[] Encode({path} sample) => Encode(sample, global::ZeroDDS.Cdr.EndianMode.LittleEndian, {representation});
    public byte[] Encode({path} sample, global::ZeroDDS.Cdr.EndianMode endian) => Encode(sample, endian, {representation});
    public byte[] Encode({path} sample, global::ZeroDDS.Cdr.EndianMode endian, int representation)
    {{
        RequireRepresentation(representation);
        global::System.ArgumentNullException.ThrowIfNull(sample);
        var w = new global::ZeroDds.Wire.Writer(Order(endian), {alignment}, {framing}, {short_codes});
        global::ZdCodec.{encoder}(w, sample);
        return w.Finish();
    }}

    public {path} Decode(global::System.ReadOnlySpan<byte> bytes) => Decode(bytes, global::ZeroDDS.Cdr.EndianMode.LittleEndian, {representation});
    public {path} Decode(global::System.ReadOnlySpan<byte> bytes, global::ZeroDDS.Cdr.EndianMode endian) => Decode(bytes, endian, {representation});
    public {path} Decode(global::System.ReadOnlySpan<byte> bytes, global::ZeroDDS.Cdr.EndianMode endian, int representation)
    {{
        RequireRepresentation(representation);
        var r = new global::ZeroDds.Wire.Reader(bytes.ToArray(), Order(endian), {alignment}, {framing});
        var sample = global::ZdCodec.{decoder}(r, global::ZeroDds.Wire.TryConstruct.Discard);
        if (r.ReaderStatus() != global::ZeroDds.Wire.Status.Ok || r.Remaining() != 0)
            throw new global::System.IO.InvalidDataException("Generated decoder refused the sample or did not consume its complete body.");
        return sample;
    }}

    public byte[] KeyHash({path} sample)
    {{
        global::System.ArgumentNullException.ThrowIfNull(sample);
        {hash_body}
    }}
{key_codecs}
}}
"#,
        type_literal = quoted(&type_name)
    );
    if !namespace.is_empty() {
        out.push_str("}\n");
    }
    Ok(out)
}

/// Key serialization uses the shared key-holder walk, including nested key
/// policy. The bound for MD5 selection comes from the layout, never this sample.
fn key_support(ast: &Ast, id: Id<TypeNode>) -> Result<(bool, String, String), String> {
    let key = LayoutKey::key(Encoding::PlainCdr2);
    let Some(mut holder) = key_holder::<CSharp>(ast, id, key).map_err(|e| e.to_string())? else {
        return Ok((false, String::new(), "return new byte[16];".to_string()));
    };
    // The generic holder's method tag is not a declared C# type name. Preserve
    // its shared body, but give the parameter the materializer's rooted type.
    let csharp::Member::Method(ref mut method) = holder else {
        return Err("the shared C# key-holder emitter did not return a method".to_string());
    };
    let sample = method
        .params
        .get_mut(1)
        .ok_or("the key holder has no sample parameter")?;
    sample.type_ = CSharp::value_type(ast, id);
    let holder_name = method.name.as_str().to_string();
    let mut members =
        codec_from::<CSharp>(ast, id, key, CodecOptions::default()).map_err(|e| e.to_string())?;
    // A keyless nested holder can require a whole-value data encoder. Keep
    // both named walks available; their prefixes distinguish them.
    members.extend(
        codec_from::<CSharp>(
            ast,
            id,
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Big),
            CodecOptions::default(),
        )
        .map_err(|e| e.to_string())?,
    );
    members.push(holder);
    let key_codecs =
        zerodds_lang_render::csharp::type_decl(&csharp::TypeDecl::Class(csharp::Class {
            doc: Doc::none(),
            attributes: Vec::new(),
            modifiers: vec![csharp::Modifier::Private, csharp::Modifier::Static],
            name: Ident::new("KeyCodec"),
            generics: Vec::new(),
            bases: Vec::new(),
            constraints: Vec::new(),
            members,
        }));
    let finish = match zerodds_ast_layout::keyhash::key_hash_form_for(ast, id, key)
        .map_err(|e| format!("key maximum cannot be planned: {e:?}"))?
    {
        zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "ZeroPadded",
        zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "Md5Digest",
    };
    Ok((
        true,
        key_codecs,
        format!(
            "var w = new global::ZeroDds.Wire.Writer(global::ZeroDds.Wire.Endianness.Big, 4, global::ZeroDds.Wire.PlCdr1Framing.Defaults(), false);\n        KeyCodec.{holder_name}(w, sample);\n        return w.{finish}();"
        ),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn model(idl: &str) -> Ast {
        let composed = zerodds_idl_compose::compose_source(
            "support.idl",
            idl,
            &zerodds_idl_compose::PathLoader::bare(),
        );
        assert!(!composed.failed(), "{:?}", composed.diagnostics);
        composed.ast
    }

    fn default_key() -> LayoutKey {
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
    }

    #[test]
    fn the_companion_is_separate_from_the_pure_wire_file() {
        let ast = model("module m { @final struct S { @key long id; }; };");
        let files = crate::generate::files(
            &ast,
            zerodds_lang_cst::Language::CSharp,
            "support.idl",
            "support",
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
            zerodds_lang_rules::emit::encode::CodecOptions::default(),
            None,
        )
        .expect("both files generate");
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "support.cs");
        assert!(!files[0].text.contains("IDdsTopicType"));
        assert!(!files[0].text.contains("class IdlNameAttribute"));
        assert!(!files[0].text.contains("struct Float128"));
        assert!(files[0].text.contains("#nullable disable"));
        assert_eq!(files[1].name, "support.TypeSupport.cs");
        assert!(files[1].text.contains("IDdsTopicType<global::m.S>"));
        assert!(files[1].text.contains("TypeName => \"m::S\""));
        assert!(files[1].text.contains("return w.ZeroPadded();"));
    }

    #[test]
    fn key_hash_form_follows_the_maximum_not_an_example_value() {
        let ast = model("@final struct S { @key string<32> name; };");
        let text = source(&ast, Encoding::PlainCdr2, default_key())
            .expect("generate")
            .expect("support");
        assert!(text.contains("return w.Md5Digest();"));
        assert!(!text.contains("return w.ZeroPadded();"));
    }

    #[test]
    fn key_holder_uses_the_same_member_lowering_as_the_data_codec() {
        let ast = model("@final struct S { @key long id; @key string<8> name; };");
        let (_, text, _) = key_support(
            &ast,
            ast.types().find(|(_, node)| node.name == "S").expect("S").0,
        )
        .expect("key holder generates");
        // Inspect the holder itself, not the ordinary encoder that already
        // uses the materializer's field names and could hide this defect.
        let holder = text.split("zd_keyholder_").nth(1).expect("holder method");
        assert!(holder.contains(".Id"), "{holder}");
        assert!(holder.contains(".Name"), "{holder}");
        assert!(!holder.contains(".id"), "{holder}");
        assert!(!holder.contains(".name"), "{holder}");
    }

    #[test]
    fn representation_is_explicit_and_other_versions_are_refused() {
        let ast = model("@appendable struct S { long id; };");
        for (encoding, representation, alignment) in
            [(Encoding::PlainCdr1, 0, 8), (Encoding::PlainCdr2, 1, 4)]
        {
            let text = source(&ast, encoding, default_key())
                .expect("generate")
                .expect("support");
            assert!(text.contains(&format!("Representation => {representation}")));
            assert!(text.contains(&format!("if (representation != {representation})")));
            assert!(text.contains(&format!("Order(endian), {alignment},")));
            assert!(text.contains("ExtensibilityKind.Appendable"));
            assert!(text.contains(
                "r.ReaderStatus() != global::ZeroDds.Wire.Status.Ok || r.Remaining() != 0"
            ));
        }
    }

    #[test]
    fn the_framing_names_a_pid_flag_policy_enum_value_not_a_string() {
        // Regression: `quoted(key.pid_flags.as_str())` put a C# string
        // literal where `PlCdr1Framing`'s constructor takes the
        // `PidFlagPolicy` enum, which is `CS1503` for every dialect
        // whose `pid_flags` differs from the default — nothing in this
        // crate's own test suite ever compiles the C# it emits, so a
        // type mismatch here does not fail `cargo test`.
        let ast = model("@final struct S { long id; };");
        let text = source(&ast, Encoding::PlainCdr2, default_key())
            .expect("generate")
            .expect("support");
        assert!(
            text.contains("global::ZeroDds.Wire.PidFlagPolicy.SentinelMustUnderstand"),
            "{text}"
        );
        assert!(!text.contains("\"sentinel-must-understand\""), "{text}");
    }

    #[test]
    fn the_framing_follows_the_keys_pid_flags_under_a_non_default_dialect() {
        let ast = model("@mutable struct S { long id; };");
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
            text.contains("global::ZeroDds.Wire.PidFlagPolicy.ImplExtensionOnMembers"),
            "{text}"
        );
    }

    #[test]
    fn metadata_is_the_shared_ast_object_and_survives_its_own_parse() {
        let ast = model("module m { @final struct S { long value; }; };");
        let text = source(&ast, Encoding::PlainCdr2, default_key())
            .expect("generate")
            .expect("support");
        let bytes = text
            .split("public byte[] TypeObject => new byte[] { ")
            .nth(1)
            .expect("nonempty metadata")
            .split(" };")
            .next()
            .expect("array ends")
            .split(", ")
            .map(|b| u8::from_str_radix(b.trim_start_matches("0x"), 16).expect("hex octet"))
            .collect::<Vec<_>>();
        let decoded = zerodds_types::type_object::TypeObject::from_bytes_le(&bytes)
            .expect("generated metadata parses");
        let (id, _) = ast.types().find(|(_, t)| t.name == "S").expect("S");
        let expected = zerodds_types::type_object::TypeObject::Complete(
            zerodds_types::from_ast::complete_type_object(&ast, id).expect("S lowers"),
        );
        assert_eq!(decoded, expected);
    }

    #[test]
    fn nested_types_and_enum_only_files_do_not_acquire_topic_support() {
        let ast = model("@nested struct Member { long value; };");
        assert!(
            source(&ast, Encoding::PlainCdr2, default_key())
                .expect("generate")
                .is_none()
        );
        let ast = model("enum Mode { ON, OFF };");
        assert!(
            source(&ast, Encoding::PlainCdr2, default_key())
                .expect("generate")
                .is_none()
        );
    }

    #[test]
    fn a_user_type_cannot_be_overwritten_by_its_companion() {
        let ast = model("struct S { long value; }; struct STypeSupport { long other; };");
        let error =
            source(&ast, Encoding::PlainCdr2, default_key()).expect_err("name collision refuses");
        assert!(error.contains("collides"), "{error}");
    }

    #[test]
    fn non_type_declarations_also_reserve_the_companion_name() {
        for declaration in [
            "exception STypeSupport { long reason; };",
            "module STypeSupport { struct Inner { long value; }; };",
            "interface STypeSupport {};",
        ] {
            let ast = model(&format!(
                "module m {{ struct S {{ long value; }}; {declaration} }};"
            ));
            let error = source(&ast, Encoding::PlainCdr2, default_key())
                .expect_err("a non-type declaration collision must refuse generation");
            assert!(error.contains("global::m.STypeSupport"), "{error}");
        }
        // The same leaf name in a different namespace must remain usable.
        let ast = model(
            "module m { struct S { long value; }; }; \
             module other { exception STypeSupport { long reason; }; };",
        );
        assert!(
            source(&ast, Encoding::PlainCdr2, default_key())
                .expect("distinct scopes")
                .is_some()
        );
    }
}
