// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Typed Python descriptors over the current shared wire codec.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use zerodds_ast::facet::Extensibility;
use zerodds_ast::{AnyId, Ast, TypeKind};
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::python;
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, key_holder};
use zerodds_lang_rules::emit::python::Python;

fn literal(text: &str) -> String {
    zerodds_lang_render::python::string_literal(text)
}

/// Appended to the same generated module, so arbitrary input file stems never
/// enter an import statement. Only the pure wire runtime is required.
pub fn source(ast: &Ast, encoding: Encoding) -> Result<Option<String>, String> {
    let names = ast
        .types()
        .map(|(id, node)| AnyId::Type(id, node.node_id))
        .chain(
            ast.modules()
                .map(|(id, node)| AnyId::Module(id, node.node_id)),
        )
        .chain(
            ast.exceptions()
                .map(|(id, node)| AnyId::Exception(id, node.node_id)),
        )
        .chain(
            ast.interfaces()
                .map(|(id, node)| AnyId::Interface(id, node.node_id)),
        );
    for name in names {
        if let Some(scoped) = ast.scoped_name_of(name) {
            if let Some(first) = scoped.segments().first() {
                if [
                    "ZD_TOPIC_DESCRIPTORS",
                    "_zd_make_topic_descriptors",
                    "_ZdTopicDescriptor",
                    "_zd_module",
                ]
                .contains(&first.as_str())
                    || first.starts_with("_zd_topic_key_")
                    || first.starts_with("zd_keyread_")
                {
                    return Err(format!(
                        "generated Python descriptor name collides with {first}"
                    ));
                }
            }
        }
    }
    let mut declarations = BTreeMap::<String, String>::new();
    let mut wrappers = String::new();
    let mut descriptors = Vec::new();
    let key = LayoutKey::key(Encoding::PlainCdr2);
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
        let path = zerodds_lang_rules::adapter::python::declared_path(ast, id)
            .map_err(|e| e.to_string())?;
        let path = path
            .segments()
            .iter()
            .map(|part| part.as_str())
            .collect::<Vec<_>>()
            .join(".");
        if path == "ZD_TOPIC_DESCRIPTORS" || path == "_zd_make_topic_descriptors" {
            return Err(format!(
                "generated Python descriptor name collides with {path}"
            ));
        }
        let scoped = ast
            .any_of_type(id)
            .ok()
            .and_then(|any| ast.scoped_name_of(any))
            .ok_or_else(|| format!("no IDL scoped name for {}", node.name))?;
        let type_name = scoped.segments().join("::");
        let encoder = zerodds_lang_rules::emit::encode::codec_name::<Python>(
            ast,
            id,
            zerodds_ast_layout::key::Purpose::Data,
        );
        let decoder = zerodds_lang_rules::emit::decode::decoder_name::<Python>(ast, id);
        let mut metadata_error = String::new();
        let mut dependencies = "()".to_string();
        let metadata = match crate::type_metadata::standard_metadata(ast, id) {
            Ok((_, objects)) => {
                let mut complete = objects.into_iter().filter(|object| {
                    matches!(object, zerodds_types::type_object::TypeObject::Complete(_))
                });
                let object = complete
                    .next()
                    .ok_or("metadata closure has no complete root")?;
                let dependent = complete
                    .map(|object| {
                        let bytes = object
                            .to_bytes_le()
                            .map_err(|e| format!("{type_name}: dependency: {e:?}"))?;
                        Ok(format!(
                            "bytes.fromhex({})",
                            literal(&bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                if !dependent.is_empty() {
                    dependencies = format!("({},)", dependent.join(", "));
                }
                let bytes = object
                    .to_bytes_le()
                    .map_err(|e| format!("{type_name}: {e:?}"))?;
                zerodds_types::type_object::TypeObject::from_bytes_le(&bytes)
                    .map_err(|e| format!("{type_name}: invalid TypeObject: {e:?}"))?;
                let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
                format!("bytes.fromhex({})", literal(&hex))
            }
            Err(error) => {
                metadata_error = format!("{type_name}: CompleteTypeObject unavailable: {error:?}");
                "None".to_string()
            }
        };
        let mut hash_name = "None".to_string();
        let mut holder_callback = "None".to_string();
        let mut holder_decoder = "None".to_string();
        let mut holder_decode_error = "keyless topic has no key holder".to_string();
        if let Some(mut holder) = key_holder::<Python>(ast, id, key).map_err(|e| e.to_string())? {
            let python::Statement::Function(ref mut function) = holder else {
                return Err("shared Python key holder is not a function".to_string());
            };
            function
                .params
                .get_mut(1)
                .ok_or("key holder lacks sample parameter")?
                .type_ = Some(Python::value_type(ast, id));
            let holder_name = function.name.as_str().to_string();
            let mut functions = codec_from::<Python>(ast, id, key, CodecOptions::default())
                .map_err(|e| e.to_string())?;
            functions.extend(
                codec_from::<Python>(
                    ast,
                    id,
                    LayoutKey::data(Encoding::PlainCdr2, Endianness::Big),
                    CodecOptions::default(),
                )
                .map_err(|e| e.to_string())?,
            );
            functions.push(holder);
            for function in functions {
                let python::Statement::Function(ref method) = function else {
                    return Err("shared Python encoder is not a function".to_string());
                };
                let name = method.name.as_str().to_string();
                let text = zerodds_lang_render::python::statement(&function);
                if let Some(previous) = declarations.insert(name.clone(), text.clone()) {
                    if previous != text {
                        return Err(format!("conflicting Python key encoder: {name}"));
                    }
                }
            }
            let finish = match zerodds_ast_layout::keyhash::key_hash_form_for(ast, id, key)
                .map_err(|e| format!("{type_name}: key maximum refused: {e:?}"))?
            {
                zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "zero_padded",
                zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "md5",
            };
            hash_name = format!("_zd_topic_key_{}", descriptors.len());
            holder_callback = format!("{hash_name}_holder");
            let _ = writeln!(
                wrappers,
                "def {hash_name}_writer(sample):\n    w = Writer(Endianness.BIG, max_align=4)\n    {holder_name}(w, sample)\n    return w\n\ndef {holder_callback}(sample):\n    return {hash_name}_writer(sample).finish()\n\ndef {hash_name}(sample):\n    return {hash_name}_writer(sample).{finish}()\n"
            );
            match zerodds_lang_rules::emit::decode::key_holder_decoder_from::<Python>(
                ast,
                id,
                key,
                CodecOptions::default(),
            ) {
                Ok(Some(functions)) => {
                    for function in functions {
                        let python::Statement::Function(ref method) = function else {
                            return Err(
                                "shared Python holder decoder is not a function".to_string()
                            );
                        };
                        let name = method.name.as_str().to_string();
                        let text = zerodds_lang_render::python::statement(&function);
                        if let Some(previous) = declarations.insert(name.clone(), text.clone()) {
                            if previous != text {
                                return Err(format!("conflicting Python key decoder: {name}"));
                            }
                        }
                    }
                    holder_decoder = zerodds_lang_rules::emit::decode::key_holder_decoder_name::<
                        Python,
                    >(ast, id);
                    holder_decode_error.clear();
                }
                Ok(None) => holder_decode_error = "shared inverse found no key holder".to_string(),
                Err(error) => {
                    holder_decode_error =
                        format!("{type_name}: key holder inverse unavailable: {error}")
                }
            }
        }
        let keyed = if hash_name == "None" { "False" } else { "True" };
        let extensibility = match ast.effective_extensibility(id) {
            Extensibility::Final => 0,
            Extensibility::Appendable => 1,
            Extensibility::Mutable => 2,
        };
        descriptors.push(format!(
            "{}: _ZdTopicDescriptor(cls={path}, type_name={}, _type_object={metadata}, keyed={keyed}, extensibility={extensibility}, representation={}, _encode=_zd_module[{}], _decode=_zd_module[{}], _key_hash={hash_name}, _key_holder={holder_callback}, _decode_key_holder={holder_decoder}, _key_decode_error={}, _metadata_error={}, _type_dependencies={dependencies})",
            literal(&type_name), literal(&type_name), if encoding.is_v1() { 0 } else { 2 },
            literal(&encoder), literal(&decoder), literal(&holder_decode_error), literal(&metadata_error),
        ));
    }
    if descriptors.is_empty() {
        return Ok(None);
    }
    let mut body = declarations.into_values().collect::<Vec<_>>().join("\n");
    body.push_str(&wrappers);
    body.push_str(&format!(
        "return {{\n    {}\n}}\n",
        descriptors.join(",\n    ")
    ));
    let indented = body
        .lines()
        .map(|line| format!("    {line}\n"))
        .collect::<String>();
    Ok(Some(format!(
        "\n# Typed DDS descriptors, using the same classes and wire functions above.\nfrom zerodds_wire.topic_descriptor import TopicDescriptor as _ZdTopicDescriptor\ndef _zd_make_topic_descriptors(_zd_module):\n{indented}\nZD_TOPIC_DESCRIPTORS = _zd_make_topic_descriptors(globals())\ndel _zd_make_topic_descriptors\n"
    )))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    fn model(idl: &str) -> Ast {
        let result = zerodds_idl_compose::compose_source(
            "support.idl",
            idl,
            &zerodds_idl_compose::PathLoader::bare(),
        );
        assert!(!result.failed(), "{:?}", result.diagnostics);
        result.ast
    }

    #[test]
    fn python_descriptor_is_in_the_same_arbitrary_stem_module() {
        let ast = model("module m { @final struct S { @key long id; }; };");
        let files = crate::generate::files(
            &ast,
            zerodds_lang_cst::Language::Python,
            "odd-stem.idl",
            "odd-stem",
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
            zerodds_lang_rules::emit::encode::CodecOptions::default(),
            None,
        )
        .expect("files");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "odd-stem.py");
        assert!(files[0].text.contains("cls=m.S"));
        assert!(files[0].text.contains("ZD_TOPIC_DESCRIPTORS ="));
        assert!(!files[0].text.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("from odd-stem") || line.starts_with("import odd-stem")
        }));
        assert!(!files[0].text.contains("import zerodds\n"));
        // Shared bounded-string readers call these runtime helpers by bare
        // name, including readers appended inside the descriptor factory.
        // Verify the actual CLI preamble, not just the companion fragment.
        let imports = files[0]
            .text
            .lines()
            .find_map(|line| line.strip_prefix("from zerodds_wire import "))
            .expect("shared runtime imports");
        let imported: Vec<_> = imports.split(',').map(str::trim).collect();
        assert!(imported.contains(&"utf8_octets"));
        assert!(imported.contains(&"utf16_units"));
    }

    #[test]
    fn python_keys_use_shared_holder_and_maximum_hash_form() {
        for (bound, finish) in [(8, "zero_padded"), (32, "md5")] {
            let ast = model(&format!(
                "@final struct Keyed {{ @key string<{bound}> name; }};"
            ));
            let root = ast
                .types()
                .find(|(_, node)| node.name == "Keyed")
                .expect("fixture root")
                .0;
            let holder = key_holder::<Python>(&ast, root, LayoutKey::key(Encoding::PlainCdr2))
                .expect("shared holder")
                .expect("keyed type");
            let python::Statement::Function(function) = holder else {
                panic!("shared key holder must be a function")
            };
            let text = source(&ast, Encoding::PlainCdr2)
                .expect("generate")
                .expect("descriptors");
            let expected = format!(
                "    def _zd_topic_key_0_writer(sample):\n        w = Writer(Endianness.BIG, max_align=4)\n        {}(w, sample)\n        return w\n    \n    def _zd_topic_key_0_holder(sample):\n        return _zd_topic_key_0_writer(sample).finish()\n    \n    def _zd_topic_key_0(sample):\n        return _zd_topic_key_0_writer(sample).{finish}()",
                function.name.as_str(),
            );
            assert!(
                text.contains(&expected),
                "bound {bound}: shared writer delegation differs\n{text}"
            );
            assert!(
                text.contains("_key_hash=_zd_topic_key_0, _key_holder=_zd_topic_key_0_holder,")
            );
            assert_eq!(
                text.matches("def _zd_topic_key_0_writer(sample):").count(),
                1
            );
        }
    }

    #[test]
    fn python_descriptor_preserves_both_dds_representation_ids() {
        let ast = model("@appendable struct S { long value; };");
        for (encoding, representation) in [(Encoding::PlainCdr1, 0), (Encoding::PlainCdr2, 2)] {
            let text = source(&ast, encoding)
                .expect("generate")
                .expect("descriptor");
            assert!(text.contains(&format!("representation={representation}")));
            assert!(text.contains("extensibility=1"));
            assert!(text.contains("_encode=_zd_module["));
        }
    }

    #[test]
    fn python_nested_and_enum_only_do_not_get_topic_descriptors() {
        for idl in ["@nested struct S { long value; };", "enum E { A, B };"] {
            assert!(
                source(&model(idl), Encoding::PlainCdr2)
                    .expect("generate")
                    .is_none()
            );
        }
    }

    #[test]
    fn python_inverse_holder_keeps_complex_keys_explicitly_unsupported() {
        let ast = model(
            "@nested struct Part { @key long id; long ignored; }; struct S { @key Part key_fields; long value; };",
        );
        let text = source(&ast, Encoding::PlainCdr2)
            .expect("generate")
            .expect("descriptor");
        assert!(text.contains("_decode_key_holder=None"));
        assert!(text.contains("unsupported nested, alias, collection or indirect key context"));
        assert!(text.contains("_key_holder=_zd_topic_key_0_holder"));
    }

    #[test]
    fn shared_inverse_refuses_unimplemented_target_initialization() {
        let ast = model("struct S { @key long id; };");
        let root = ast
            .types()
            .find(|(_, node)| node.name == "S")
            .expect("root")
            .0;
        let result = zerodds_lang_rules::emit::decode::key_holder_decoder_from::<
            zerodds_lang_rules::emit::c::C,
        >(
            &ast,
            root,
            LayoutKey::key(Encoding::PlainCdr2),
            CodecOptions::default(),
        );
        assert!(
            result
                .expect_err("no implicit C initialization contract")
                .to_string()
                .contains("target does not support explicit key-holder reader initialization")
        );
    }

    #[test]
    fn python_reserved_descriptor_namespace_refuses_collision() {
        let ast = model("module ZD_TOPIC_DESCRIPTORS { struct S { long value; }; };");
        assert!(
            source(&ast, Encoding::PlainCdr2)
                .expect_err("collision")
                .contains("collides")
        );
    }
}
