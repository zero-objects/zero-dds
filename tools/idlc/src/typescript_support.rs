// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The current TypeScript codec connected to `dds.ts`'s `TypeSupport<T>` bridge.
//!
//! Appended to the same generated module, like Python's companion and unlike
//! C#/Java's: this one needs no package beyond `@zerodds/wire`, which the base
//! file already imports — there is no `ZeroDDS.Cdr`-shaped separate assembly
//! for a TypeScript consumer to avoid pulling in.
//!
//! `crates/ts-node/src/dds.ts` already documents the target shape (`A
//! TypeSupport as emitted by idlc ts (the <Type>TypeSupport const)`) and
//! `createTypedTopic` already accepts it — this module is what was missing.

use std::collections::BTreeMap;
use zerodds_ast::facet::Extensibility;
use zerodds_ast::{AnyId, Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey, PidFlagPolicy};
use zerodds_lang_cst::typescript as ts;
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, key_holder};
use zerodds_lang_rules::emit::typescript::TypeScript;

/// A companion const per eligible topic aggregate, or none for an
/// enum/typedef-only file. Planning failures are generation errors, never an
/// invented key hash.
pub fn source(ast: &Ast, encoding: Encoding, key: LayoutKey) -> Result<Option<String>, String> {
    let mut bodies = Vec::new();
    let mut key_declarations = BTreeMap::<String, String>::new();
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
        bodies.push(support(
            ast,
            id,
            node,
            encoding,
            key,
            &mut key_declarations,
        )?);
    }
    if bodies.is_empty() {
        return Ok(None);
    }
    let mut out = String::from(
        "\n// Generated typed DDS support. Do not edit.\n\
         // Requires the generated codec above and @zerodds/wire; no other package.\n\
         // `PL_CDR1_DEFAULT` is not in the base codec's own runtime import list\n\
         // (only the key holder below needs it, and it never frames a PL_CDR1\n\
         // parameter list — the value doesn't reach the KeyHash's bytes), so it\n\
         // is imported again here rather than added to every generated file.\n\
         import { PL_CDR1_DEFAULT } from \"@zerodds/wire\";\n",
    );
    // One shared namespace for every type's key-holder helpers in this file,
    // so a primitive leaf encoder two types' keys both reach (`zd_enc_p_long`)
    // is declared once. Unlike Python, where the same top-level `def` twice
    // just keeps the last one, a bare top-level `function zd_enc_p_long`
    // declared twice in one TypeScript module is `Duplicate function
    // implementation` — the reason C# nests its own copy in `KeyCodec`
    // rather than appending it at the outer scope, and the reason this one
    // does too.
    if !key_declarations.is_empty() {
        out.push_str("namespace ZdKeyCodec {\n");
        for text in key_declarations.values() {
            out.push_str(text);
            out.push('\n');
        }
        out.push_str("}\n\n");
    }
    out.push_str(&bodies.join("\n"));
    Ok(Some(out))
}

fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// `policy` as the exported `@zerodds/wire` `PidFlags` constant the runtime's
/// `PlCdr1Framing.pidFlags` field actually takes — one of the three values a
/// generated module already imports (see `probe.ts`'s import list), not a
/// string it would have to parse. Exhaustive on [`PidFlagPolicy`]'s own
/// variants, matching `csharp_support::csharp_pid_flags`'s reasoning.
const fn typescript_pid_flags(policy: PidFlagPolicy) -> &'static str {
    match policy {
        PidFlagPolicy::SentinelMustUnderstand => "PID_FLAGS_SENTINEL_MUST_UNDERSTAND",
        PidFlagPolicy::Bare => "PID_FLAGS_BARE",
        PidFlagPolicy::ImplExtensionOnMembers => "PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS",
    }
}

fn support(
    ast: &Ast,
    id: Id<TypeNode>,
    node: &TypeNode,
    encoding: Encoding,
    key: LayoutKey,
    key_declarations: &mut BTreeMap<String, String>,
) -> Result<String, String> {
    let path = zerodds_lang_rules::adapter::typescript::declared_path(ast, id)
        .map_err(|e| e.to_string())?;
    let segments = path
        .segments()
        .iter()
        .map(|s| s.as_str().to_string())
        .collect::<Vec<_>>();
    let (module_segments, leaf) = segments
        .split_at_checked(segments.len().saturating_sub(1))
        .ok_or("TypeScript type path has no leaf segment")?;
    let leaf_name = leaf
        .first()
        .ok_or("TypeScript type path has no leaf segment")?;
    let support_leaf = format!("{leaf_name}TypeSupport");
    let mut support_segments = module_segments.to_vec();
    support_segments.push(support_leaf.clone());
    let support_path = support_segments.join(".");

    for (other, _) in ast.types() {
        if zerodds_lang_rules::adapter::typescript::declared_path(ast, other).is_ok_and(
            |candidate| {
                candidate
                    .segments()
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
                    == support_path
            },
        ) {
            return Err(format!(
                "generated TypeScript support name collides with {support_path}"
            ));
        }
    }
    // Exceptions, interfaces and modules occupy the same declaration space
    // (namespace merging aside, a `namespace`/`interface`/value of the same
    // name in one scope is still a collision worth refusing rather than
    // silently shadowing).
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
            scoped
                .segments()
                .iter()
                .map(|segment| ts::escape_keyword(segment))
                .collect::<Vec<_>>()
                .join(".")
                == support_path
        }) {
            return Err(format!(
                "generated TypeScript support name collides with {support_path}"
            ));
        }
    }

    let scoped = ast
        .any_of_type(id)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))
        .ok_or_else(|| format!("no scoped name for {}", node.name))?;
    let type_name = scoped.segments().join("::");
    let encoder = zerodds_lang_rules::emit::encode::codec_name::<TypeScript>(
        ast,
        id,
        zerodds_ast_layout::key::Purpose::Data,
    );
    let decoder = zerodds_lang_rules::emit::decode::decoder_name::<TypeScript>(ast, id);
    let representation = if encoding.is_v1() { 0 } else { 2 };
    let alignment = encoding.max_alignment();
    let headers = key.pl_cdr1_headers;
    let framing = format!(
        "{{ pidFlags: {pid_flags}, shortMaxId: {short_max_id}, padsDeclaredLength: {pad_declared}, padsOptionalLength: {pad_optional}, memberMustUnderstand: {must_understand}, padAfterValue: {pad_after} }}",
        pid_flags = typescript_pid_flags(key.pid_flags),
        short_max_id = key.short_pid_range.highest_short_id(),
        pad_declared = headers.pads_declared_length(),
        pad_optional = headers.pads_declared_optional_length(),
        must_understand = key.member_pid_carries_must_understand(),
        pad_after = headers.pads_after_value(),
    );
    let short_codes = key.length_codes.short_code_where_it_fits();
    let extensibility = match ast.effective_extensibility(id) {
        Extensibility::Final => 0,
        Extensibility::Appendable => 1,
        Extensibility::Mutable => 2,
    };
    let value_ty = ts::Type::named(path.clone());
    let value_type = zerodds_lang_render::typescript::type_(&value_ty);
    let (keyed, key_hash_body) = key_support(ast, id, &value_ty, key_declarations)?;
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
            format!("return new Uint8Array([{octets}]);")
        }
        Err(reason) => format!(
            "throw new Error({});",
            quoted(&format!("{type_name}: no CompleteTypeObject: {reason:?}"))
        ),
    };

    let key_hash_property = if keyed {
        format!("\n  keyHash(sample: {value_type}): Uint8Array {{\n{key_hash_body}\n  }},")
    } else {
        String::new()
    };

    let body = format!(
        r#"export const {support_leaf} = {{
  typeName: {type_literal},
  isKeyed: {keyed},
  extensibility: {extensibility},
  representation: {representation},
  get typeObject(): Uint8Array {{
    {metadata}
  }},
  encode(sample: {value_type}, endian: "le" | "be" = "le", representation: number = {representation}): Uint8Array {{
    if (representation !== {representation}) {{
      throw new Error("this generated codec does not implement the requested XCDR representation");
    }}
    const w = new Writer(endian === "be" ? Endianness.Big : Endianness.Little, {alignment}, {framing}, {short_codes});
    {encoder}(w, sample);
    return w.finish();
  }},
  decode(bytes: Uint8Array, offset: number = 0, length: number = bytes.length - offset, endian: "le" | "be" = "le", representation: number = {representation}): {value_type} {{
    if (representation !== {representation}) {{
      throw new Error("this generated codec does not implement the requested XCDR representation");
    }}
    const r = new Reader(bytes.subarray(offset, offset + length), endian === "be" ? Endianness.Big : Endianness.Little, {alignment}, {framing});
    const sample = {decoder}(r, TryConstruct.Discard);
    if (r.readerStatus() !== ZdReaderStatus.Ok || r.remaining() !== 0) {{
      throw new Error("generated decoder refused the sample or did not consume its complete body");
    }}
    return sample;
  }},{key_hash_property}
}};
"#,
        type_literal = quoted(&type_name),
    );
    // A type nested in an IDL module gets its companion nested the same
    // way, by reopening that module's namespace (TypeScript merges
    // repeated `namespace X { … }` blocks) — `bridge.PointTypeSupport`,
    // matching where `bridge.Point` itself lives, and matching the
    // collision check above, which compares against the DOTTED
    // `support_path`, not the bare leaf: a bare top-level const here
    // would still compile for one module but collide the moment two
    // modules each declared a same-named type.
    if module_segments.is_empty() {
        Ok(body)
    } else {
        // `export`, not bare: every reopening of a merged namespace has to
        // agree on that (TS2395 "Individual declarations in merged
        // declaration ... must be all exported or all local"), and the
        // materialiser's own first declaration of this namespace already
        // is one.
        Ok(format!(
            "export namespace {} {{\n{body}}}\n",
            module_segments.join(".")
        ))
    }
}

/// Key serialization uses the shared key-holder walk, including nested key
/// policy. The bound for MD5 selection comes from the layout, never this
/// sample. Every declaration the walk emits is merged into `key_declarations`
/// (module-wide, deduplicated by name — see [`source`]) rather than returned
/// inline, since it has to be declared exactly once in the shared
/// `ZdKeyCodec` namespace and not once per type.
fn key_support(
    ast: &Ast,
    id: Id<TypeNode>,
    value_type: &ts::Type,
    key_declarations: &mut BTreeMap<String, String>,
) -> Result<(bool, String), String> {
    let key = LayoutKey::key(Encoding::PlainCdr2);
    let Some(mut holder) = key_holder::<TypeScript>(ast, id, key).map_err(|e| e.to_string())?
    else {
        return Ok((false, String::new()));
    };
    let ts::Decl::Function(ref mut function) = holder else {
        return Err(
            "the shared TypeScript key-holder emitter did not return a function".to_string(),
        );
    };
    // The generic holder's sample parameter carries a synthetic type tag;
    // the materializer's own rooted type is what the file elsewhere calls it.
    let sample = function
        .params
        .get_mut(1)
        .ok_or("the key holder has no sample parameter")?;
    sample.type_ = Some(value_type.clone());
    let holder_name = function.name.as_str().to_string();
    let mut declarations = codec_from::<TypeScript>(ast, id, key, CodecOptions::default())
        .map_err(|e| e.to_string())?;
    // A keyless nested holder can require a whole-value data encoder. Keep
    // both named walks available; their prefixes distinguish them.
    declarations.extend(
        codec_from::<TypeScript>(
            ast,
            id,
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Big),
            CodecOptions::default(),
        )
        .map_err(|e| e.to_string())?,
    );
    declarations.push(holder);
    for decl in declarations {
        let ts::Decl::Function(ref f) = decl else {
            return Err("a shared TypeScript key encoder is not a function".to_string());
        };
        let name = f.name.as_str().to_string();
        let mut exported = f.clone();
        exported.exported = true;
        let text = zerodds_lang_render::typescript::decl(&ts::Decl::Function(exported));
        if let Some(previous) = key_declarations.insert(name.clone(), text.clone()) {
            if previous != text {
                return Err(format!("conflicting TypeScript key encoder: {name}"));
            }
        }
    }
    let finish = match zerodds_ast_layout::keyhash::key_hash_form_for(ast, id, key)
        .map_err(|e| format!("key maximum cannot be planned: {e:?}"))?
    {
        zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "zeroPadded",
        zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "md5",
    };
    Ok((
        true,
        format!(
            "    const w = new Writer(Endianness.Big, 4, PL_CDR1_DEFAULT, false);\n    ZdKeyCodec.{holder_name}(w, sample);\n    return w.{finish}();"
        ),
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
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
    fn the_companion_is_appended_to_the_same_file() {
        let ast = model("module m { @final struct S { @key long id; }; };");
        let files = crate::generate::files(
            &ast,
            zerodds_lang_cst::Language::TypeScript,
            "support.idl",
            "support",
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
            zerodds_lang_rules::emit::encode::CodecOptions::default(),
            None,
        )
        .expect("generate");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "support.ts");
        assert!(files[0].text.contains("export const STypeSupport"));
        assert!(files[0].text.contains("typeName: \"m::S\""));
        assert!(files[0].text.contains("return w.zeroPadded();"));
        assert!(
            files[0]
                .text
                .contains("import { PL_CDR1_DEFAULT } from \"@zerodds/wire\";")
        );
    }

    #[test]
    fn key_hash_form_follows_the_maximum_not_an_example_value() {
        let ast = model("@final struct S { @key string<32> name; };");
        let text = source(&ast, Encoding::PlainCdr2, default_key())
            .expect("generate")
            .expect("support");
        assert!(text.contains("return w.md5();"));
        assert!(!text.contains("return w.zeroPadded();"));
    }

    #[test]
    fn representation_is_explicit_and_other_versions_are_refused() {
        let ast = model("@appendable struct S { long id; };");
        for (encoding, representation, alignment) in
            [(Encoding::PlainCdr1, 0, 8), (Encoding::PlainCdr2, 2, 4)]
        {
            let text = source(&ast, encoding, default_key())
                .expect("generate")
                .expect("support");
            assert!(text.contains(&format!("representation: {representation},")));
            assert!(text.contains(&format!("representation !== {representation}")));
            assert!(text.contains(&format!(", {alignment}, {{")));
            assert!(text.contains("extensibility: 1,"));
            assert!(text.contains("r.readerStatus() !== ZdReaderStatus.Ok || r.remaining() !== 0"));
        }
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
    fn shared_key_helpers_are_declared_once_across_types() {
        let ast = model(
            "module m { @final struct A { @key long id; }; @final struct B { @key long id; }; };",
        );
        let text = source(&ast, Encoding::PlainCdr2, default_key())
            .expect("generate")
            .expect("support");
        assert_eq!(text.matches("namespace ZdKeyCodec {").count(), 1);
        assert!(text.contains("export const ATypeSupport"));
        assert!(text.contains("export const BTypeSupport"));
    }

    #[test]
    fn keyless_type_gets_no_key_hash_method() {
        let ast = model("@final struct S { long id; };");
        let text = source(&ast, Encoding::PlainCdr2, default_key())
            .expect("generate")
            .expect("support");
        assert!(text.contains("isKeyed: false,"));
        assert!(!text.contains("keyHash("));
    }
}
