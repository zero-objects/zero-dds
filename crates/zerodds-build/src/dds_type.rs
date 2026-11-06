// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `zerodds_dcps::DdsType` impl for a generated type.
//!
//! The codec the generator writes is a pair of inherent functions over
//! `zerodds_wire::Writer`/`Reader`, and `DomainParticipant::create_topic`
//! takes a `T: zerodds_dcps::DdsType`. Every consumer in this tree used
//! to close that gap by hand — `tests/issue32/src/robot.rs` and
//! `interop/cyclone-xtypes-27/reader/src/robot.rs` are two of them — and
//! a hand-written bridge is where the wire questions get answered a
//! second time, differently. This module writes the impl from the same
//! model the codec came from.
//!
//! # Why the text is built as a string
//!
//! `type_objects_module` and `type_identifiers_module` next door do the
//! same, and for the same reason: what is emitted here names types from
//! **outside** the module tree the CST fold produced, so it is appended
//! to a rendered file rather than folded into it.

use core::fmt::Write as _;

use zerodds_ast::facet::Extensibility;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey, Purpose};

use crate::{encoding_for, has_codec};

/// Every `impl zerodds_dcps::DdsType` the model calls for, as Rust
/// source.
///
/// `xcdr1` names the types whose XCDR1 codec pair the generator managed
/// to write. A type absent from it keeps the trait's own
/// `encode_xcdr1`/`decode_xcdr1`/`encode_xcdr1_be`/`decode_xcdr1_be`,
/// which **refuse** rather than fall back — see the doc comments on
/// those methods in `crates/dcps/src/dds_type.rs`. A comment naming the
/// type is emitted in its place, so a reader who misses the four
/// overrides finds the reason where they would have been.
///
/// # Errors
/// Names the type when its key holder cannot be selected or planned. A
/// failed key must not leave the generated type with keyless defaults.
pub(crate) fn dds_type_impls(
    ast: &Ast,
    xcdr1: &[Id<TypeNode>],
    key_for: &dyn Fn(Encoding) -> Result<LayoutKey, String>,
) -> Result<String, String> {
    let mut out = String::new();
    for (id, node) in ast.types() {
        if node.name.is_empty() || !has_codec(&node.kind) {
            continue;
        }
        // A `bitmask` is emitted as `pub type X = u32`, not as a type of its
        // own — so `impl zerodds_dcps::DdsType for X` is `impl … for u32`,
        // and neither the trait nor the type is local: `error[E0117]`, with
        // an `E0308` for every member of it that follows. The other four
        // kinds `has_codec` admits do get a type of their own (`bitset` is a
        // struct, an enumeration is an enum), so this is the one exception
        // and it is named rather than filtered silently.
        //
        // Nothing is lost: a bitmask has no members and no key, so it is a
        // member type and never a topic type. What a topic needs is the
        // `DdsType` of the aggregate that carries it, and that one is here.
        if matches!(node.kind, TypeKind::Bitmask { .. }) {
            let _ = write!(
                out,
                "\n// `{}`: a bitmask is a `u32` alias in Rust, so a `DdsType` \
                 impl for it\n// would be an impl for `u32` and that is `error[E0117]`. It is a member type,\n// never a topic type; the aggregate that carries it has the impl.\n",
                node.name
            );
            continue;
        }
        // DDS-XTypes 1.3 §7.6.2: "The only types that may be associated
        // with a DDS Topic are the Aggregated Types" — structures and
        // unions here. An enumeration and a bitset are member types: a
        // `DdsType` impl would let `create_topic` accept them, so there is
        // none.
        if matches!(
            node.kind,
            TypeKind::Enumeration { .. } | TypeKind::Bitset { .. }
        ) {
            let _ = write!(
                out,
                "\n// `{}`: {} is a member type. DDS-XTypes 1.3 \u{a7}7.6.2 allows only the \
                 aggregated types as topic types,\n// so there is no `DdsType` impl for it; the \
                 aggregate that carries it has one.\n",
                node.name,
                if matches!(node.kind, TypeKind::Enumeration { .. }) {
                    "an enumeration"
                } else {
                    "a bitset"
                }
            );
            continue;
        }
        out.push_str(&one_impl(ast, id, node, xcdr1.contains(&id), key_for)?);
    }
    Ok(out)
}

/// The impl for one type, or a comment saying why there is none.
fn one_impl(
    ast: &Ast,
    id: Id<TypeNode>,
    node: &TypeNode,
    has_xcdr1: bool,
    key_for: &dyn Fn(Encoding) -> Result<LayoutKey, String>,
) -> Result<String, String> {
    let Ok(path) = zerodds_lang_rules::adapter::cst::path_from_file_root(ast, id) else {
        return Ok(format!(
            "\n// `{}`: the model has no module chain for it, so it cannot be named \
             from the file root and gets no `DdsType` impl.\n",
            node.name
        ));
    };
    let type_name = qualified_idl_name(ast, id, node);
    let mut body = String::new();

    let _ = write!(
        body,
        "    /// The peer's topic type name, which XTypes 1.3 \u{a7}7.6.3 \
         matches\n    /// by strict equality \u{2014} the IDL scoped name, \
         without the\n    /// leading `::` that Cyclone, Fast DDS and RTI all \
         omit.\n    const TYPE_NAME: &'static str = \"{type_name}\";\n"
    );

    let _ = write!(
        body,
        "    const EXTENSIBILITY: zerodds_dcps::Extensibility =\n        \
         zerodds_dcps::Extensibility::{};\n",
        extensibility_variant(ast.effective_extensibility(id))
    );

    if is_nested(node) {
        let _ = write!(
            body,
            "    /// `@nested`, XTypes 1.3 \u{a7}7.4.6.3.5: a member type and not \
             a topic\n    /// type. `create_topic` refuses it with \
             `PreconditionNotMet`.\n    const IS_NESTED: bool = true;\n"
        );
    }

    body.push_str(&key_items(ast, id, &path, key_for)?);
    body.push_str(&identity_items(ast, id));
    body.push_str(&codec_items(
        &path,
        ast.effective_extensibility(id),
        has_xcdr1,
        &key_for(encoding_for(ast, id))?,
        &key_for(xcdr1_encoding(ast.effective_extensibility(id)))?,
    ));
    if zerodds_lang_rules::encode::has_key_members_read(ast, id, key_for(encoding_for(ast, id))?) {
        body.push_str(&key_members_item(
            &path,
            has_xcdr1,
            &key_for(encoding_for(ast, id))?,
            &key_for(xcdr1_encoding(ast.effective_extensibility(id)))?,
            ast.effective_extensibility(id),
        ));
    }
    body.push_str(&field_value_item(ast, id, node));

    let mut out = format!("\nimpl zerodds_dcps::DdsType for {path} {{\n{body}}}\n");
    if !has_xcdr1 {
        out.push_str(&format!(
            "// `{}`: no XCDR1 codec could be planned for this type, so its four \
             XCDR1\n// methods stay the trait's, which refuse. A pair that \
             delegated to the XCDR2\n// codec would put version-2 octets under a \
             `CDR_LE`/`CDR_BE` encapsulation, and\n// no peer can tell that from a \
             sample it is supposed to read.\n",
            node.name
        ));
    }
    Ok(out)
}

/// `HAS_KEY`, `KEY_HOLDER_MAX_SIZE`, `encode_key_holder_be` and
/// `decode_key_holder_be`.
///
/// Empty for a type with neither a `@key` member nor a keyed union discriminator:
/// the trait's defaults are `false`, `None` and an empty write, which is
/// what a keyless type means.
fn key_items(
    ast: &Ast,
    id: Id<TypeNode>,
    path: &str,
    key_for: &dyn Fn(Encoding) -> Result<LayoutKey, String>,
) -> Result<String, String> {
    // A union's discriminator is a key without being a MemberNode
    // (`switch (@key D)`, XTypes 1.3 §7.2.2.4.4.4.8): `key_members_with`
    // answers the empty list for it, and the union is a keyed topic type
    // all the same, whose holder is the discriminator. The Rust union
    // codec emits `key_holder_be`/`key_from_holder_be` for it.
    let keyed_union = zerodds_ast_layout::keyhash::union_key_discriminator(ast, id).is_some();
    // The holder's key, not the sample's: XTypes 1.3 §7.6.8 fixes the
    // purpose and the byte order, and the rest of the dialect carries
    // over from the plan the codec was built with.
    let data_key = key_for(encoding_for(ast, id))?;
    let key = LayoutKey {
        purpose: Purpose::Key,
        endianness: Endianness::Big,
        ..data_key
    };
    let members = zerodds_ast_layout::keyhash::key_members_with(
        ast,
        id,
        key.key_holder_nesting,
        key.key_holder_order,
    )
    .map_err(|error| format!("DdsType key holder for `{path}`: {error}"))?;
    if members.is_empty() && !keyed_union {
        return Ok(String::new());
    }
    // §7.6.8 step 5 branches on the holder's **maximum** serialised
    // size, including bounded variable-length members. `Some(16)` selects the
    // zero-padded branch and `None` the MD5 one, which is exactly the
    // question `key_hash_form_for` answered; passing the number itself would
    // be a second computation of it.
    let form = zerodds_ast_layout::keyhash::key_hash_form_for(ast, id, key)
        .map_err(|error| format!("DdsType key holder for `{path}`: {error}"))?;
    let max_size = match form {
        zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "Some(16)",
        zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "None",
    };

    let mut items = format!(
        "    const HAS_KEY: bool = true;\n    \
         /// XTypes 1.3 \u{a7}7.6.8 step 5, from the holder's maximum size: \
         at most\n    /// sixteen octets means the octets themselves, \
         zero-padded; a larger or unbounded\n    /// maximum means their MD5.\n    \
         const KEY_HOLDER_MAX_SIZE: Option<usize> = {max_size};\n\n    \
         // The refusal below is a `panic!`, which this tree's lints deny \
         by\n    // default; the allow is narrower than asking every consumer \
         of a\n    // generated file to turn the lint off.\n    \
         #[allow(clippy::panic)]\n    \
         fn encode_key_holder_be(&self, holder: &mut zerodds_dcps::dds_type::PlainCdr2BeKeyHolder) \
         {{\n        \
         match {path}::key_holder_be(self) {{\n            \
         Ok(bytes) => holder.write_bytes(&bytes),\n            \
         // The generated holder refuses exactly the values the IDL's own \
         bounds\n            // forbid \u{2014} a `@key char` above U+00FF, a \
         `@key string<8>` holding\n            // nine. The trait method has no \
         way to report, and leaving the holder\n            // empty would give \
         every such instance one KeyHash and silently merge\n            // \
         instances the application keeps apart. Refusing by name is the \
         lesser\n            // of the two.\n            \
         Err(error) => panic!(\n                \
         \"the key holder of `{path}` refused this instance's key: {{error}}\"\n            \
         ),\n        }}\n    }}\n\n"
    );
    // The inverse of the writer above. §7.6.8 step 4 fixes both the order and
    // the alignment cap of the holder — big-endian, PLAIN_CDR2 `MAXALIGN`
    // four — whatever encoding the sample itself goes out in, unless the
    // dialect aligns the holder to its data encoding or packs it
    // (`KeyHolderAlignment`), as the writer does.
    items.push_str(&decode_fn(
        path,
        "decode_key_holder_be",
        "key_from_holder_be",
        "Big",
        key.key_holder_alignment.max_alignment(key.encoding),
        "",
    ));
    Ok(items)
}

/// `TYPE_IDENTIFIER` and `type_object()`.
///
/// Both in the **complete** form and not one of each. `DcpsRuntime::
/// register_type_object` keys the registry by the hash it computes from
/// the object it is handed, so a `CompleteTypeObject` registered under a
/// `TYPE_IDENTIFIER` that is a *minimal* hash is an entry no lookup of
/// this endpoint's own identifier ever finds — the endpoint would
/// publish an id it also fails to resolve.
fn identity_items(ast: &Ast, id: Id<TypeNode>) -> String {
    let mut out = String::new();

    match zerodds_types::from_ast::type_identifier(ast, id, zerodds_types::from_ast::Form::Complete)
    {
        Ok(zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash)) => {
            let _ = write!(
                out,
                "    /// The complete equivalence hash, XTypes 1.3 \u{a7}7.3.4.6, \
                 computed\n    /// from the same model the codec was built \
                 from.\n    const TYPE_IDENTIFIER: zerodds_types::TypeIdentifier \
                 =\n        zerodds_types::TypeIdentifier::EquivalenceHashComplete(\n            \
                 zerodds_types::EquivalenceHash({}),\n        );\n",
                octet_array(&hash.0)
            );
        }
        // A type in a strongly connected component (XTypes 1.3 §7.3.4.9):
        // its identifier is the component's hash and its own position, and
        // the component is handed to the runtime whole, because the hash
        // belongs to all of its members and no one object can be filed
        // under it. `type_object()` below is still emitted: it is the
        // member's own object, which a peer reaches through the component.
        Ok(zerodds_types::TypeIdentifier::StronglyConnectedComponent(scc))
            if scc.equivalence_kind == zerodds_types::type_identifier::kinds::EK_COMPLETE =>
        {
            let _ = write!(
                out,
                "    /// The identifier of a type in a strongly connected component, XTypes \
                 1.3\n    /// \u{a7}7.3.4.9.3: the component's complete hash, its length and the \
                 type's\n    /// position in it.\n    const TYPE_IDENTIFIER: \
                 zerodds_types::TypeIdentifier =\n        \
                 zerodds_types::TypeIdentifier::StronglyConnectedComponent(\n            \
                 zerodds_types::StronglyConnectedComponentId {{\n                \
                 equivalence_kind: 0x{:02X},\n                hash: \
                 zerodds_types::EquivalenceHash({}),\n                scc_length: {},\n                \
                 scc_index: {},\n            }},\n        );\n",
                scc.equivalence_kind,
                octet_array(&scc.hash.0),
                scc.scc_length,
                scc.scc_index
            );
            out.push_str(&component_items(ast, id));
        }
        // Every other shape — a fully descriptive identifier — is left to
        // the trait's `None`, which says "no type id provided" and sends
        // reader/writer matching down the type-name path of DDS 1.4
        // §2.2.3. Emitting something else here would state an identity the
        // runtime cannot resolve.
        Ok(other) => {
            let _ = writeln!(
                out,
                "    // No `TYPE_IDENTIFIER`: {other:?} is not an equivalence hash, \
                 and matching falls back to the type name."
            );
        }
        Err(error) => {
            let _ = writeln!(
                out,
                "    // No `TYPE_IDENTIFIER`: the identifier could not be computed \
                 \u{2014} {error:?}."
            );
        }
    }

    // The object is emitted only if it survives its own round trip. A
    // blob that does not parse would reach `from_bytes_le` at run time
    // and answer `None` through the `.ok()` below, which is the same
    // thing as no object at all but says so several hours later.
    let object = zerodds_types::from_ast::complete_type_object(ast, id)
        .ok()
        .map(zerodds_types::type_object::TypeObject::Complete)
        .and_then(|object| object.to_bytes_le().ok())
        .filter(|bytes| zerodds_types::type_object::TypeObject::from_bytes_le(bytes).is_ok());
    if let Some(bytes) = object {
        let _ = write!(
            out,
            "\n    /// The `CompleteTypeObject` this endpoint registers with the \
             TypeLookup\n    /// server, so a peer that asks for \
             `TYPE_IDENTIFIER` gets an answer.\n    /// XCDR2-LE, \
             XTypes 1.3 \u{a7}7.3.4.\n    fn type_object() -> \
             Option<zerodds_types::type_object::TypeObject> {{\n        \
             zerodds_types::type_object::TypeObject::from_bytes_le(&{})\n            \
             .ok()\n    }}\n",
            octet_array(&bytes)
        );
        // The closure the object names by hash — base type, member
        // aggregates, enumerations, aliases — registered beside it.
        // Each blob is filtered by the same round trip as the object
        // above; one that does not parse is left out here rather than
        // answered `None` at run time.
        let dependencies: Vec<Vec<u8>> = zerodds_types::from_ast::dependency_type_objects(ast, id)
            .unwrap_or_default()
            .into_iter()
            .map(zerodds_types::type_object::TypeObject::Complete)
            .filter_map(|object| object.to_bytes_le().ok())
            .filter(|bytes| zerodds_types::type_object::TypeObject::from_bytes_le(bytes).is_ok())
            .collect();
        if !dependencies.is_empty() {
            let arrays: Vec<String> = dependencies
                .iter()
                .map(|bytes| format!("            &{},\n", octet_array(bytes)))
                .collect();
            let _ = write!(
                out,
                "\n    /// The `CompleteTypeObject`s the one above names by hash, \
                 transitively:\n    /// base type, member aggregates, enumerations, \
                 aliases. Registered beside\n    /// it so the closure resolves locally \
                 and `getTypeDependencies` is\n    /// answerable. XTypes 1.3 \
                 \u{a7}7.3.4.6, \u{a7}7.6.3.3.4.\n    fn type_object_dependencies() -> \
                 Vec<zerodds_types::type_object::TypeObject> {{\n        \
                 let blobs: &[&[u8]] = &[\n{}        ];\n        blobs\n            \
                 .iter()\n            .filter_map(|bytes| \
                 zerodds_types::type_object::TypeObject::from_bytes_le(bytes).ok())\n            \
                 .collect()\n    }}\n",
                arrays.concat()
            );
        }
    }
    out
}

/// `type_component()`: the strongly connected component the type is a
/// member of, in both forms, so the runtime registers it whole.
///
/// The objects go in as the octets `TypeObject::to_bytes_le` writes, as
/// `type_object()` does, and the generated function answers `None` unless
/// every one of them parses and the counts are the component's length: a
/// component with a member missing would put a type at an index that is
/// not its own.
fn component_items(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(Some(component)) = zerodds_types::from_ast::type_component(ast, id) else {
        return String::from(
            "\n    // No `type_component()`: the component could not be built, and the \
             runtime\n    // cannot announce a TypeInformation for a type it cannot serve.\n",
        );
    };
    let encode = |object: zerodds_types::type_object::TypeObject| object.to_bytes_le().ok();
    let complete: Option<Vec<Vec<u8>>> = component
        .complete
        .iter()
        .cloned()
        .map(|object| encode(zerodds_types::type_object::TypeObject::Complete(object)))
        .collect();
    let minimal: Option<Vec<Vec<u8>>> = component
        .minimal
        .iter()
        .cloned()
        .map(|object| encode(zerodds_types::type_object::TypeObject::Minimal(object)))
        .collect();
    let (Some(complete), Some(minimal)) = (complete, minimal) else {
        return String::from(
            "\n    // No `type_component()`: a member of the component did not encode.\n",
        );
    };
    let arrays = |blobs: &[Vec<u8>]| -> String {
        blobs
            .iter()
            .map(|bytes| format!("            &{},\n", octet_array(bytes)))
            .collect::<String>()
    };
    format!(
        "\n    /// The component this type belongs to, in both forms, XTypes 1.3 \
         \u{a7}7.3.4.9.2.\n    /// Registered whole: the hash identifies the component, not a \
         member.\n    fn type_component() -> \
         Option<zerodds_types::resolve::TypeComponent> {{\n        \
         use zerodds_types::type_object::TypeObject;\n        \
         let complete_blobs: &[&[u8]] = &[\n{complete_blobs}        ];\n        \
         let minimal_blobs: &[&[u8]] = &[\n{minimal_blobs}        ];\n        \
         let complete: Vec<_> = complete_blobs\n            .iter()\n            \
         .filter_map(|bytes| match TypeObject::from_bytes_le(bytes).ok()? {{\n                \
         TypeObject::Complete(object) => Some(object),\n                _ => None,\n            \
         }})\n            .collect();\n        \
         let minimal: Vec<_> = minimal_blobs\n            .iter()\n            \
         .filter_map(|bytes| match TypeObject::from_bytes_le(bytes).ok()? {{\n                \
         TypeObject::Minimal(object) => Some(object),\n                _ => None,\n            \
         }})\n            .collect();\n        \
         if complete.len() != {length} || minimal.len() != {length} {{\n            \
         return None;\n        }}\n        \
         Some(zerodds_types::resolve::TypeComponent {{\n            complete_hash: \
         zerodds_types::EquivalenceHash({complete_hash}),\n            minimal_hash: \
         zerodds_types::EquivalenceHash({minimal_hash}),\n            complete,\n            \
         minimal,\n        }})\n    }}\n",
        complete_blobs = arrays(&complete),
        minimal_blobs = arrays(&minimal),
        length = component.complete.len(),
        complete_hash = octet_array(&component.complete_hash.0),
        minimal_hash = octet_array(&component.minimal_hash.0),
    )
}

/// The four codec pairs.
///
/// The byte order is the `Writer`'s and the `Reader`'s, so one generated
/// body serves both orders of one XCDR version; the alignment cap is the
/// version's, `MAXALIGN` from XTypes 1.3 §7.4.1, and it is the one thing
/// a caller of the generated codec has to get right.
///
/// The settings the layout key carries beyond its plan — the PL_CDR1
/// framing, the `wchar` width and framing, the `wstring` unit order and,
/// on the writer, the short length codes — are spelled into the
/// constructed `Reader` and `Writer` by [`wire_settings`] where the
/// dialect `v2_key` and `v1_key` were planned in answers them differently
/// from the constructors' defaults. The defaults are the specification
/// dialect's, so a module built in it constructs both exactly as before.
/// A module built with `Config::dialect` used to get its plan in the
/// dialect and its reader and writer in the specification's: a codec built
/// for Fast DDS refused the bare `0x3f02` sentinel Fast DDS closes a
/// PL_CDR1 parameter list with.
fn codec_items(
    path: &str,
    extensibility: Extensibility,
    has_xcdr1: bool,
    v2_key: &LayoutKey,
    v1_key: &LayoutKey,
) -> String {
    // The second pair's name comes from the emitter that wrote it, not
    // from a string spelled again here: the two would drift, and the
    // failure would be an impl calling a function the codec no longer
    // has.
    let second = zerodds_lang_rules::encode::SECONDARY_SUFFIX;

    let mut out = String::new();
    let v2 = Encoding::PlainCdr2.max_alignment();
    let (v2_write, v2_read) = (wire_settings(v2_key, true), wire_settings(v2_key, false));
    for (name, order) in [("encode", "Little"), ("encode_be", "Big")] {
        out.push_str(&encode_fn(path, name, "encode", order, v2, &v2_write));
    }
    for (name, order) in [("decode", "Little"), ("decode_be", "Big")] {
        out.push_str(&decode_fn(path, name, "decode", order, v2, &v2_read));
    }
    let mut v1_continuing = None;
    if has_xcdr1 {
        let v1 = xcdr1_encoding(extensibility).max_alignment();
        let (v1_write, v1_read) = (wire_settings(v1_key, true), wire_settings(v1_key, false));
        v1_continuing = Some((v1, v1_write.clone(), v1_read.clone()));
        for (name, order) in [("encode_xcdr1", "Little"), ("encode_xcdr1_be", "Big")] {
            out.push_str(&encode_fn(
                path,
                name,
                &format!("encode{second}"),
                order,
                v1,
                &v1_write,
            ));
        }
        for (name, order) in [("decode_xcdr1", "Little"), ("decode_xcdr1_be", "Big")] {
            out.push_str(&decode_fn(
                path,
                name,
                &format!("decode{second}"),
                order,
                v1,
                &v1_read,
            ));
        }
    }
    out.push_str(&continuing_fns(
        path,
        second,
        (v2, &v2_write, &v2_read),
        v1_continuing
            .as_ref()
            .map(|(align, write, read)| (*align, write.as_str(), read.as_str())),
    ));
    out
}

/// `decode_key_members` (`zerodds_dcps::DdsType`): the codec's
/// `decode_keys` pair, which reads the `@key` members and steps over the
/// others, under the encoding and byte order the caller names. A type with
/// no XCDR1 plan answers `None` for XCDR1; the reader decodes it whole.
fn key_members_item(
    path: &str,
    has_xcdr1: bool,
    v2_key: &LayoutKey,
    v1_key: &LayoutKey,
    extensibility: Extensibility,
) -> String {
    let second = zerodds_lang_rules::encode::SECONDARY_SUFFIX;
    let v2 = Encoding::PlainCdr2.max_alignment();
    let v2_read = wire_settings(v2_key, false);
    let xcdr1 = if has_xcdr1 {
        let v1 = xcdr1_encoding(extensibility).max_alignment();
        let v1_read = wire_settings(v1_key, false);
        format!(
            "if xcdr1 {{\n            \
             let mut r = zerodds_wire::Reader::with_max_align(bytes, endianness, {v1}){v1_read};\n            \
             return Ok({path}::decode_keys{second}(&mut r)?);\n        }}"
        )
    } else {
        "if xcdr1 {\n            return Err(zerodds_dcps::DecodeError::Invalid { what: \"no XCDR1 plan\" });\n        }"
            .to_string()
    };
    let unsupported = if has_xcdr1 {
        String::new()
    } else {
        "\n        if xcdr1 {\n            return None;\n        }".to_string()
    };
    format!(
        "\n    fn decode_key_members(\n        bytes: &[u8],\n        xcdr1: bool,\n        big_endian: bool,\n    \
         ) -> Option<Result<Self, zerodds_dcps::DecodeError>> {{{unsupported}\n        \
         let endianness = if big_endian {{\n            zerodds_wire::Endianness::Big\n        }} else {{\n            \
         zerodds_wire::Endianness::Little\n        }};\n        \
         let read = || -> Result<Self, zerodds_dcps::DecodeError> {{\n        {xcdr1}\n        \
         let mut r = zerodds_wire::Reader::with_max_align(bytes, endianness, {v2}){v2_read};\n        \
         Ok({path}::decode_keys(&mut r)?)\n        }};\n        Some(read())\n    }}\n"
    )
}

/// `encode_continuing` and `decode_continuing`: the value as the
/// continuation of a sample whose first octets someone else wrote, little
/// endian, alignment counted from the start of the sample. DDS-RPC 1.0
/// §7.5.1.1 puts a request behind its `RequestHeader` and a reply behind its
/// `ReplyHeader`, whose 28 octets are no multiple of XCDR1's alignment cap
/// of 8.
///
/// The trait's default refuses an offset that is not a multiple of the cap,
/// because it cannot know whether a member cares; the generated codec
/// starts at any offset (`Writer::continuing`, `Reader::skip_to`), as the
/// interface emitter's topic types already did. Without it a generated
/// reply type behind a `ReplyHeader` was refused under XCDR1 and the
/// replier sent `REMOTE_EX_UNKNOWN_EXCEPTION` in place of the reply.
///
/// `v1` is `None` for a type with no XCDR1 plan: its `xcdr1` branch refuses
/// as the trait's XCDR1 methods do.
fn continuing_fns(
    path: &str,
    second: &str,
    v2: (u8, &str, &str),
    v1: Option<(u8, &str, &str)>,
) -> String {
    let (v2_align, v2_write, v2_read) = v2;
    let (encode_v1, decode_v1) = match v1 {
        Some((v1_align, v1_write, v1_read)) => (
            format!(
                "let mut w = zerodds_wire::Writer::continuing(\n                \
                 zerodds_wire::Endianness::Little,\n                {v1_align},\n                \
                 core::mem::take(prefix),\n            ){v1_write};\n            \
                 let written = {path}::encode{second}(self, &mut w).and_then(|_| w.check_deferred());\n            \
                 *prefix = w.finish();\n            written"
            ),
            format!(
                "let mut r = zerodds_wire::Reader::with_max_align(\n                bytes,\n                \
                 zerodds_wire::Endianness::Little,\n                {v1_align},\n            ){v1_read};\n            \
                 r.skip_to(offset)?;\n            \
                 Ok({path}::decode{second}(&mut r)?)"
            ),
        ),
        None => (
            "return Err(zerodds_dcps::EncodeError::Invalid {\n                \
             what: \"XCDR1 encoding not implemented for this type\",\n            })"
                .to_string(),
            "Err(zerodds_dcps::DecodeError::Invalid {\n                \
             what: \"XCDR1 decoding not implemented for this type\",\n            })"
                .to_string(),
        ),
    };
    format!(
        "\n    fn encode_continuing(\n        &self,\n        prefix: &mut Vec<u8>,\n        xcdr1: bool,\n    \
         ) -> Result<(), zerodds_dcps::EncodeError> {{\n        \
         let start = prefix.len();\n        \
         let written = if xcdr1 {{\n            {encode_v1}\n        }} else {{\n            \
         let mut w = zerodds_wire::Writer::continuing(\n                \
         zerodds_wire::Endianness::Little,\n                {v2_align},\n                \
         core::mem::take(prefix),\n            ){v2_write};\n            \
         let written = {path}::encode(self, &mut w).and_then(|_| w.check_deferred());\n            \
         *prefix = w.finish();\n            written\n        }};\n        \
         if written.is_err() {{\n            prefix.truncate(start);\n        }}\n        \
         written?;\n        Ok(())\n    }}\n\n    \
         fn decode_continuing(\n        bytes: &[u8],\n        offset: usize,\n        xcdr1: bool,\n    \
         ) -> Result<Self, zerodds_dcps::DecodeError> {{\n        \
         if xcdr1 {{\n            {decode_v1}\n        }} else {{\n            \
         let mut r = zerodds_wire::Reader::with_max_align(\n                bytes,\n                \
         zerodds_wire::Endianness::Little,\n                {v2_align},\n            ){v2_read};\n            \
         r.skip_to(offset)?;\n            \
         Ok({path}::decode(&mut r)?)\n        }}\n    }}\n"
    )
}

/// The `with_*` calls that set a generated `Reader` (`writer == false`) or
/// `Writer` to what `key` answers beyond its plan, one per setting that
/// differs from the constructors' default — the specification dialect's.
/// Empty in that dialect.
///
/// The PL_CDR1 values come from [`LayoutKey::pl_cdr1_framing`], the one
/// place `zerodds-wire-codec` takes them from as well.
fn wire_settings(key: &LayoutKey, writer: bool) -> String {
    use zerodds_ast_layout::key::{PidFlagPolicy, WcharFraming, WcharWidth, WstringUnitOrder};
    let default = LayoutKey::data(key.encoding, key.endianness);
    let mut out = String::new();
    let framing = key.pl_cdr1_framing();
    if framing != default.pl_cdr1_framing() {
        let pid_flags = match framing.pid_flags {
            PidFlagPolicy::SentinelMustUnderstand => "SentinelMustUnderstand",
            PidFlagPolicy::Bare => "Bare",
            PidFlagPolicy::ImplExtensionOnMembers => "ImplExtensionOnMembers",
        };
        let _ = write!(
            out,
            "\n        .with_pl_cdr1_framing(zerodds_wire::PlCdr1Framing {{\n            \
             pid_flags: zerodds_wire::PidFlagPolicy::{pid_flags},\n            \
             highest_short_id: {},\n            \
             pad_declared_length: {},\n            \
             pad_declared_optional_length: {},\n            \
             member_must_understand: {},\n            \
             require_extended: {},\n            \
             pad_after_value: {},\n        }})",
            framing.highest_short_id,
            framing.pad_declared_length,
            framing.pad_declared_optional_length,
            framing.member_must_understand,
            framing.require_extended,
            framing.pad_after_value,
        );
    }
    // The reading is the reader's alone; a writer has no such setting.
    if !writer && key.pl_cdr1_reading != default.pl_cdr1_reading {
        out.push_str("\n        .with_pl_cdr1_reading(zerodds_wire::PlCdr1Reading::Tolerant)");
    }
    if key.wchar_width != default.wchar_width {
        let width = match key.wchar_width {
            WcharWidth::Two => "Two",
            WcharWidth::Four => "Four",
        };
        let _ = write!(
            out,
            "\n        .with_wchar_width(zerodds_wire::WcharWidth::{width})"
        );
    }
    if key.wchar_framing != default.wchar_framing {
        let framing = match key.wchar_framing {
            WcharFraming::Fixed => "Fixed",
            WcharFraming::GiopLengthPrefixed => "GiopLengthPrefixed",
            WcharFraming::GiopLengthPrefixedMemberOnly => "GiopLengthPrefixedMemberOnly",
        };
        let _ = write!(
            out,
            "\n        .with_wchar_framing(zerodds_wire::WcharFraming::{framing})"
        );
    }
    if key.wstring_unit_order != default.wstring_unit_order {
        let order = match key.wstring_unit_order {
            WstringUnitOrder::Stream => "Stream",
            WstringUnitOrder::BigEndianAlways => "BigEndianAlways",
            WstringUnitOrder::ByteOrderMark => "ByteOrderMark",
            WstringUnitOrder::HostUnswapped => "HostUnswapped",
        };
        let _ = write!(
            out,
            "\n        .with_wstring_unit_order(zerodds_wire::WstringUnitOrder::{order})"
        );
    }
    // The reader takes a length code from the stream; only the writer
    // chooses one.
    if writer
        && key.length_codes.short_code_where_it_fits()
            != default.length_codes.short_code_where_it_fits()
    {
        let _ = write!(
            out,
            "\n        .with_short_length_codes({})",
            key.length_codes.short_code_where_it_fits()
        );
    }
    out
}

/// One `encode`-shaped method over the generated `inherent` function.
///
/// The writer appends to `out` itself (`Writer::appending`), so the sample
/// is written once: a writer of its own grew from empty and was then copied
/// over, four reallocations and a copy per 1 kB sample on the Linux test
/// host. A refused sample leaves `out` as it was.
fn encode_fn(
    path: &str,
    name: &str,
    inherent: &str,
    order: &str,
    max_align: u8,
    settings: &str,
) -> String {
    format!(
        "\n    fn {name}(&self, out: &mut Vec<u8>) -> Result<(), zerodds_dcps::EncodeError> {{\n        \
         let start = out.len();\n        \
         let mut w = zerodds_wire::Writer::appending(\n            \
         zerodds_wire::Endianness::{order},\n            {max_align},\n            \
         core::mem::take(out),\n        ){settings};\n        \
         let written = {path}::{inherent}(self, &mut w).and_then(|_| w.check_deferred());\n        \
         *out = w.finish();\n        \
         if written.is_err() {{\n            out.truncate(start);\n        }}\n        \
         written?;\n        Ok(())\n    }}\n"
    )
}

/// One `decode`-shaped method over the generated inherent function.
fn decode_fn(
    path: &str,
    name: &str,
    inherent: &str,
    order: &str,
    max_align: u8,
    settings: &str,
) -> String {
    format!(
        "\n    fn {name}(bytes: &[u8]) -> Result<Self, zerodds_dcps::DecodeError> {{\n        \
         let mut r = zerodds_wire::Reader::with_max_align(\n            bytes,\n            \
         zerodds_wire::Endianness::{order},\n            {max_align},\n        ){settings};\n        \
         Ok({path}::{inherent}(&mut r)?)\n    }}\n"
    )
}

/// `field_value`, for the SQL filter behind `QueryCondition` and
/// `ContentFilteredTopic`. DDS 1.4 §B.2.1 with §2.2.2.5.9.
///
/// A member this cannot answer for gets **no arm**, so the path falls to
/// `None` and the filter denies the sample rather than comparing against
/// a substituted value. What has no arm: an `@optional` member that is an
/// aggregate or a collection (a scalar or string one answers, and its
/// absence is `null`, §7.6.7.2); a collection, which §B.2.1 reaches by
/// index and this does not carry; and an `unsigned long long`, whose
/// range does not fit the filter's `i64`.
fn field_value_item(ast: &Ast, id: Id<TypeNode>, node: &TypeNode) -> String {
    if matches!(node.kind, TypeKind::Union { .. }) {
        return union_filter_items(ast, id);
    }
    let TypeKind::Structure { members, .. } = &node.kind else {
        return String::new();
    };
    let mut arms = String::new();
    // DDS-XTypes 1.4 Beta 1 §7.6.7.1: whether a path reaches a union
    // member its discriminator does not select, through the members that
    // are aggregates.
    let mut unselected_arms = String::new();
    for member in members {
        let Ok(member_node) = ast.member(*member) else {
            continue;
        };
        let optional = member_node
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_optional);
        let Some(target) = member_node
            .type_
            .resolved()
            .map(|id| resolve_alias(ast, id))
        else {
            continue;
        };
        let Ok(target_node) = ast.type_(target) else {
            continue;
        };
        let field = zerodds_lang_cst::rust::escape_keyword(&member_node.name);
        // A member held as a `rust_type` mapping: the filter compares the
        // IDL value the Rust value stands for, written through `to_wire`
        // and read back, as a peer would receive it.
        if let Some(mapping) =
            zerodds_lang_rules::adapter::cst::member_rust_type_mapping(ast, member_node)
        {
            if let Some(arm) = mapped_filter_arm(
                &member_node.name,
                &field,
                &target_node.kind,
                &mapping,
                optional,
                member_node
                    .facets
                    .xtypes
                    .as_ref()
                    .is_some_and(|x| x.is_external),
            ) {
                arms.push_str(&arm);
            }
            continue;
        }
        // §7.6.7.2: "A DataReader's filter [...] shall consider an
        // optional member that is absent to be equal to null." The
        // field is an `Option` (boxed when it is also `@external`); a
        // present value is the scalar or string the plain member is, an
        // absent one is `null`. Only the members the filter has a value
        // shape for get an arm: an optional aggregate or collection has
        // none, as the plain one has none.
        if optional {
            let external = member_node
                .facets
                .xtypes
                .as_ref()
                .is_some_and(|x| x.is_external);
            let deref = if external { "**v" } else { "*v" };
            let name = &member_node.name;
            let present = match &target_node.kind {
                TypeKind::Primitive(kind) => {
                    scalar_value(*kind, "v").map(|value| format!("{{ let v = {deref}; {value} }}"))
                }
                TypeKind::String { .. } => Some(String::from(
                    "zerodds_dcps::FilterValue::String(String::clone(v))",
                )),
                _ => None,
            };
            if let Some(present) = present {
                let _ = writeln!(
                    arms,
                    "            (\"{name}\", None) => Some(match &self.{field} {{\n                \
                     Some(v) => {present},\n                \
                     None => zerodds_dcps::FilterValue::Null,\n            }}),"
                );
            }
            continue;
        }
        let value = if member_node
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_external)
        {
            format!("(*self.{field})")
        } else {
            format!("self.{field}")
        };
        let name = &member_node.name;
        match &target_node.kind {
            TypeKind::Primitive(kind) => {
                if let Some(value) = scalar_value(*kind, &value) {
                    let _ = writeln!(arms, "            (\"{name}\", None) => Some({value}),");
                }
            }
            TypeKind::String { .. } => {
                let _ = writeln!(
                    arms,
                    "            (\"{name}\", None) => \
                     Some(zerodds_dcps::FilterValue::String({value}.clone())),"
                );
            }
            // A nested aggregate answers for its own members: `a.b` is
            // §B.2.1's dotted field reference, and the nested type has
            // the same impl one level down.
            TypeKind::Structure { .. } | TypeKind::Union { .. } => {
                let _ = writeln!(
                    arms,
                    "            (\"{name}\", Some(rest)) => \
                     zerodds_dcps::DdsType::field_value(&{value}, rest),"
                );
                let _ = writeln!(
                    unselected_arms,
                    "            Some((\"{name}\", rest)) => \
                     zerodds_dcps::DdsType::field_unselected(&{value}, rest),"
                );
            }
            // Annex B.2 ENUMERATEDVALUE: an enumerated member compares
            // with the name of its enumerator.
            TypeKind::Enumeration { .. } => {
                // The place itself, without the parentheses an
                // `@external` member's value carries.
                let place = value
                    .strip_prefix('(')
                    .and_then(|v| v.strip_suffix(')'))
                    .unwrap_or(&value);
                if let Some(text) = enumerator_name(ast, target, place) {
                    let _ = writeln!(arms, "            (\"{name}\", None) => Some({text}),");
                }
            }
            _ => {}
        }
    }
    let mut out = String::new();
    if !arms.is_empty() {
        let _ = write!(
            out,
            "\n    fn field_value(&self, path: &str) -> Option<zerodds_dcps::FilterValue> {{\n        \
             let (head, rest) = match path.split_once('.') {{\n            \
             Some((head, rest)) => (head, Some(rest)),\n            \
             None => (path, None),\n        }};\n        \
             match (head, rest) {{\n{arms}            _ => None,\n        }}\n    }}\n"
        );
    }
    if !unselected_arms.is_empty() {
        let _ = write!(
            out,
            "\n    fn field_unselected(&self, path: &str) -> bool {{\n        \
             match path.split_once('.') {{\n{unselected_arms}            _ => false,\n        }}\n    }}\n"
        );
    }
    out
}

/// `FilterValue::String` of the name of the enumerator `value` holds, for
/// the enumeration `enumeration` (DDS 1.4 Annex B.2 ENUMERATEDVALUE).
fn enumerator_name(ast: &Ast, enumeration: Id<TypeNode>, value: &str) -> Option<String> {
    let path = zerodds_lang_rules::adapter::cst::path_from_file_root(ast, enumeration).ok()?;
    let mut arms = String::new();
    for enumerator in ast.enumerators_of(enumeration) {
        let enumerator = ast.enumerator(*enumerator).ok()?;
        let _ = write!(
            arms,
            "{path}::{} => \"{}\", ",
            zerodds_lang_cst::rust::escape_keyword(&enumerator.name),
            enumerator.name
        );
    }
    Some(format!(
        "zerodds_dcps::FilterValue::String(String::from(match {value} {{ {arms}}}))"
    ))
}

/// `field_value` and `field_unselected` of a union (DDS-XTypes 1.4 Beta 1
/// §7.6.7.1): a branch by its member's name, the discriminator by
/// `#discriminator`; a branch the discriminator does not select has no
/// value and makes the filter that names it false.
///
/// A branch this cannot answer for gets no arm, as a struct member does:
/// a `rust_type` mapping, a collection, an `unsigned long long`.
fn union_filter_items(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(shape) = zerodds_lang_rules::union_variants(ast, id) else {
        return String::new();
    };
    let discriminator = shape.discriminator;
    let Ok(discriminator_node) = ast.type_(discriminator) else {
        return String::new();
    };
    // The discriminator a variant carries, `d` a reference to it.
    let carried = match &discriminator_node.kind {
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            Some(String::from("Some(zerodds_dcps::FilterValue::Bool(*d))"))
        }
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => Some(String::from(
            "Some(zerodds_dcps::FilterValue::String(d.to_string()))",
        )),
        // `unsigned long long` overruns the filter's `i64`: the value
        // where it fits, none where it does not.
        TypeKind::Primitive(PrimitiveKind::UInt64) => Some(String::from(
            "i64::try_from(*d).ok().map(zerodds_dcps::FilterValue::Int)",
        )),
        TypeKind::Primitive(kind) => {
            scalar_value(*kind, "*d").map(|value| format!("Some({value})"))
        }
        TypeKind::Enumeration { .. } => {
            enumerator_name(ast, discriminator, "d").map(|text| format!("Some({text})"))
        }
        _ => None,
    };
    // The discriminator a one-label variant stands for, from its label.
    let fixed = |label: i128| -> Option<String> {
        Some(match &discriminator_node.kind {
            TypeKind::Primitive(PrimitiveKind::Boolean) => {
                format!("Some(zerodds_dcps::FilterValue::Bool({}))", label != 0)
            }
            TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
                let character = char::from_u32(u32::try_from(label).ok()?)?;
                format!(
                    "Some(zerodds_dcps::FilterValue::String(String::from({:?})))",
                    character.to_string()
                )
            }
            TypeKind::Primitive(_) => match i64::try_from(label) {
                Ok(value) => format!("Some(zerodds_dcps::FilterValue::Int({value}))"),
                Err(_) => String::from("None"),
            },
            TypeKind::Enumeration { .. } => {
                let enumerator = ast
                    .enumerators_of(discriminator)
                    .iter()
                    .copied()
                    .find(|e| ast.effective_enumerator_value(*e) == Some(label))?;
                let name = &ast.enumerator(enumerator).ok()?.name;
                format!("Some(zerodds_dcps::FilterValue::String(String::from(\"{name}\")))")
            }
            _ => return None,
        })
    };

    let mut discriminator_arms = String::new();
    let mut value_arms = String::new();
    let mut unselected_arms = String::new();
    let mut names = Vec::new();
    for case in &shape.cases {
        let variant = &case.variant;
        let name = &case.member.name;
        names.push(format!("\"{name}\""));
        // The discriminator.
        let discriminator_value = if case.carries_discriminator {
            carried
                .as_ref()
                .map(|value| (format!("Self::{variant}(d, _)"), value.clone()))
        } else {
            case.labels
                .first()
                .and_then(|label| fixed(*label))
                .map(|value| (format!("Self::{variant}(..)"), value))
        };
        if let Some((pattern, value)) = discriminator_value {
            let _ = writeln!(discriminator_arms, "                {pattern} => {value},");
        }
        // The branch.
        if zerodds_lang_rules::adapter::cst::member_rust_type_mapping(ast, case.member).is_some() {
            continue;
        }
        let bound = if case.carries_discriminator {
            format!("Self::{variant}(_, v)")
        } else {
            format!("Self::{variant}(v)")
        };
        let external = case
            .member
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_external);
        let deref = if external { "**v" } else { "*v" };
        let reference = if external { "&**v" } else { "v" };
        let Some(target) = case
            .member
            .type_
            .resolved()
            .map(|id| resolve_alias(ast, id))
        else {
            continue;
        };
        let Ok(target_node) = ast.type_(target) else {
            continue;
        };
        match &target_node.kind {
            // A character as the text it is (`v` is a reference).
            TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
                let _ = writeln!(
                    value_arms,
                    "            ({bound}, \"{name}\", None) => \
                     Some(zerodds_dcps::FilterValue::String(v.to_string())),"
                );
            }
            TypeKind::Primitive(kind) => {
                if let Some(value) = scalar_value(*kind, deref) {
                    let _ = writeln!(
                        value_arms,
                        "            ({bound}, \"{name}\", None) => Some({value}),"
                    );
                }
            }
            TypeKind::String { .. } => {
                let _ = writeln!(
                    value_arms,
                    "            ({bound}, \"{name}\", None) => \
                     Some(zerodds_dcps::FilterValue::String(String::clone({reference}))),"
                );
            }
            TypeKind::Enumeration { .. } => {
                if let Some(text) = enumerator_name(ast, target, reference) {
                    let _ = writeln!(
                        value_arms,
                        "            ({bound}, \"{name}\", None) => Some({text}),"
                    );
                }
            }
            TypeKind::Structure { .. } | TypeKind::Union { .. } => {
                let _ = writeln!(
                    value_arms,
                    "            ({bound}, \"{name}\", Some(rest)) => \
                     zerodds_dcps::DdsType::field_value({reference}, rest),"
                );
                let _ = writeln!(
                    unselected_arms,
                    "            ({bound}, \"{name}\") => \
                     rest.is_some_and(|rest| zerodds_dcps::DdsType::field_unselected({reference}, rest)),"
                );
            }
            _ => {}
        }
    }
    if let Some(no_member) = shape.no_member
        && let Some(value) = &carried
    {
        let _ = writeln!(
            discriminator_arms,
            "                Self::{no_member}(d) => {value},"
        );
    }
    let names = names.join(" | ");
    format!(
        "\n    // Every variant has a discriminator arm where the type allows\n    \
         // one; the wildcard is for those it does not.\n    \
         #[allow(unreachable_patterns)]\n    \
         fn field_value(&self, path: &str) -> Option<zerodds_dcps::FilterValue> {{\n        \
         let (head, rest) = match path.split_once('.') {{\n            \
         Some((head, rest)) => (head, Some(rest)),\n            \
         None => (path, None),\n        }};\n        \
         if head == \"#discriminator\" {{\n            \
         if rest.is_some() {{\n                return None;\n            }}\n            \
         return match self {{\n{discriminator_arms}                _ => None,\n            }};\n        }}\n        \
         match (self, head, rest) {{\n{value_arms}            _ => None,\n        }}\n    }}\n\n    \
         // A variant can match two arms below: an aggregate branch is\n    \
         // asked further, every other selected one is not.\n    \
         #[allow(unreachable_patterns)]\n    \
         fn field_unselected(&self, path: &str) -> bool {{\n        \
         let (head, rest) = match path.split_once('.') {{\n            \
         Some((head, rest)) => (head, Some(rest)),\n            \
         None => (path, None),\n        }};\n        \
         let _ = rest;\n        \
         match (self, head) {{\n{unselected_arms}{selected_arms}            (_, {names}) => true,\n            _ => false,\n        }}\n    }}\n",
        selected_arms = selected_arms(&shape),
    )
}

/// `(Self::V(..), "name") => false,` for every case: a branch the
/// discriminator selects, named without a further step.
fn selected_arms(shape: &zerodds_lang_rules::UnionVariants<'_>) -> String {
    let mut out = String::new();
    for case in &shape.cases {
        let _ = writeln!(
            out,
            "            (Self::{}(..), \"{}\") => false,",
            case.variant, case.member.name
        );
    }
    out
}

/// The `field_value` arm of a member held as a `rust_type` mapping, or
/// `None` where the filter has no value shape for its IDL type.
///
/// `zerodds_wire::repr::idl_string` and `idl_primitive` write the Rust
/// value through its `to_wire` and read the IDL value back; a value the
/// conversion refuses answers `None`, and the filter denies the sample
/// rather than compare a substitute. An absent `@optional` member is
/// `null` (§7.6.7.2).
fn mapped_filter_arm(
    name: &str,
    field: &str,
    idl: &TypeKind,
    mapping: &zerodds_lang_rules::adapter::cst::RustTypeMapping,
    optional: bool,
    external: bool,
) -> Option<String> {
    let (function, convert) = match idl {
        TypeKind::String { .. } => (
            "idl_string",
            String::from("zerodds_dcps::FilterValue::String"),
        ),
        TypeKind::Primitive(kind) => (
            "idl_primitive",
            format!("|v| {}", scalar_value(*kind, "v")?),
        ),
        _ => return None,
    };
    let call = |value: &str| match &mapping.adapter {
        None => format!("zerodds_wire::repr::{function}({value})"),
        Some(adapter) => {
            let adapter: Vec<&str> = adapter
                .segments()
                .iter()
                .map(zerodds_lang_cst::common::Ident::as_str)
                .collect();
            format!(
                "zerodds_wire::repr::{function}_with(&{}, {value})",
                adapter.join("::")
            )
        }
    };
    Some(if optional {
        let value = if external { "&**v" } else { "v" };
        format!(
            "            (\"{name}\", None) => match &self.{field} {{\n                \
             Some(v) => {}.ok().map({convert}),\n                \
             None => Some(zerodds_dcps::FilterValue::Null),\n            }},\n",
            call(value)
        )
    } else {
        let value = if external {
            format!("&*self.{field}")
        } else {
            format!("&self.{field}")
        };
        format!(
            "            (\"{name}\", None) => {}.ok().map({convert}),\n",
            call(&value)
        )
    })
}

/// The filter value one scalar member yields, if the filter has a shape
/// for it.
fn scalar_value(kind: PrimitiveKind, field: &str) -> Option<String> {
    // `from` takes its argument directly: an `@external` member arrives
    // as `(*self.x)` and the wrapper parens would be an `unused_parens`
    // defect under the warnings-as-errors a consumer may compile with.
    let bare = field
        .strip_prefix('(')
        .and_then(|f| f.strip_suffix(')'))
        .filter(|f| f.starts_with("*self."))
        .unwrap_or(field);
    Some(match kind {
        PrimitiveKind::Boolean => {
            format!("zerodds_dcps::FilterValue::Bool({bare})")
        }
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64 => {
            format!("zerodds_dcps::FilterValue::Int(i64::from({bare}))")
        }
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            format!("zerodds_dcps::FilterValue::Float(f64::from({bare}))")
        }
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            format!("zerodds_dcps::FilterValue::String({field}.to_string())")
        }
        // `unsigned long long` overruns the filter's `i64` and
        // `long double` has no Rust holder here. Both fall through to
        // `None`, which denies the sample rather than comparing a
        // wrapped or rounded number.
        PrimitiveKind::UInt64 | PrimitiveKind::Float128 => return None,
    })
}

/// The type behind however many `typedef`s stand in front of it.
fn resolve_alias(ast: &Ast, mut id: Id<TypeNode>) -> Id<TypeNode> {
    // Bounded rather than `loop`: a cyclic alias chain is a defect the
    // front end rejects, and a generator that trusted it would hang.
    for _ in 0..64 {
        match ast.type_(id).map(|node| &node.kind) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(next) => id = next,
                None => return id,
            },
            _ => return id,
        }
    }
    id
}

/// The XCDR1 encoding an aggregate of this extensibility is framed in.
///
/// XCDR1 has no `@appendable` framing at all — XTypes 1.3 Tab. 59 gives
/// version 1 no DHEADER — so `@final` and `@appendable` are the same
/// stream there, and only `@mutable` takes a parameter list.
pub(crate) const fn xcdr1_encoding(extensibility: Extensibility) -> Encoding {
    match extensibility {
        Extensibility::Final | Extensibility::Appendable => Encoding::PlainCdr1,
        Extensibility::Mutable => Encoding::PlCdr1,
    }
}

/// The IDL scoped name, without the leading `::`.
fn qualified_idl_name(ast: &Ast, id: Id<TypeNode>, node: &TypeNode) -> String {
    ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))
        .map(|scoped| {
            let text = scoped.to_string();
            text.strip_prefix("::").unwrap_or(&text).to_string()
        })
        .unwrap_or_else(|| node.name.clone())
}

/// Whether the declaration carries `@nested`.
fn is_nested(node: &TypeNode) -> bool {
    node.facets
        .xtypes
        .as_ref()
        .is_some_and(|xtypes| xtypes.is_nested)
}

/// The trait's spelling of an extensibility kind.
const fn extensibility_variant(extensibility: Extensibility) -> &'static str {
    match extensibility {
        Extensibility::Final => "Final",
        Extensibility::Appendable => "Appendable",
        Extensibility::Mutable => "Mutable",
    }
}

/// Octets as a Rust array literal.
fn octet_array(bytes: &[u8]) -> String {
    let octets: Vec<String> = bytes.iter().map(|b| format!("0x{b:02x}")).collect();
    format!("[{}]", octets.join(", "))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    /// The specification's `LayoutKey`, for a test with no dialect of
    /// its own — [`dds_type_impls`]'s production caller is
    /// `Config::layout_key`, and this is its default-dialect answer.
    fn default_key_for(encoding: Encoding) -> Result<LayoutKey, String> {
        Ok(LayoutKey::data(encoding, Endianness::Little))
    }

    /// Compose one source into a model.
    fn model(source: &str) -> Ast {
        let dir = std::env::temp_dir().join(format!(
            "zerodds-build-dds-type-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("scratch");
        let path = dir.join("Probe.idl");
        std::fs::write(&path, source).expect("write");
        let composed = zerodds_idl_compose::compose_with(
            &path.to_string_lossy(),
            &zerodds_idl_compose::PathLoader::new(Vec::new()),
            zerodds_idl_compose::ComposeOptions::default(),
        )
        .expect("compose");
        assert!(!composed.failed(), "{:?}", composed.diagnostics);
        composed.ast
    }

    /// XTypes 1.3 §7.2.2.4.4.4.8 and §7.6.8: a union keyed by its
    /// discriminator is a keyed topic type although it has no `@key`
    /// member, and its holder is the discriminator — a `long`, four
    /// octets, so step 5's zero-padded branch.
    #[test]
    fn xtypes_7_6_8_a_union_keyed_by_its_discriminator_is_a_keyed_topic_type() {
        let ast = model(
            "enum CommandKind { START, STOP, GO_LEFT, GO_RIGHT }; \
             union Command switch (@key CommandKind) { case START: float delay; \
             case STOP: float distance; case GO_LEFT: case GO_RIGHT: float angle; }; \
             union Plain switch (long) { case 0: float x; };",
        );
        let union = ast
            .types()
            .find(|(_, node)| node.name == "Command")
            .map(|(id, _)| id)
            .expect("Command");
        assert!(
            zerodds_ast_layout::keyhash::key_members_with(
                &ast,
                union,
                Default::default(),
                Default::default()
            )
            .expect("members")
            .is_empty(),
            "the discriminator is not a MemberNode"
        );
        let impls = dds_type_impls(&ast, &[], &default_key_for).expect("impls");
        let command = impls
            .split("impl zerodds_dcps::DdsType for ")
            .find(|block| block.starts_with("Command "))
            .expect("Command's impl");
        assert!(command.contains("const HAS_KEY: bool = true"), "{command}");
        assert!(command.contains("const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16)"));
        assert!(command.contains("Command::key_holder_be(self)"));
        let plain = impls
            .split("impl zerodds_dcps::DdsType for ")
            .find(|block| block.starts_with("Plain "))
            .expect("Plain's impl");
        assert!(!plain.contains("const HAS_KEY: bool = true"));
    }

    #[test]
    fn an_unplannable_key_refuses_the_impl_instead_of_becoming_keyless() {
        let ast = model("@final struct Broken { @key any id; };");
        let error = dds_type_impls(&ast, &[], &default_key_for)
            .expect_err("a key without a DDS wire form must refuse");
        assert!(error.contains("DdsType key holder for `Broken`"), "{error}");
        assert!(error.contains("any"), "{error}");
    }

    #[test]
    fn a_genuinely_keyless_type_keeps_the_trait_defaults() {
        let ast = model("@final struct Plain { long id; };");
        let code = dds_type_impls(&ast, &[], &default_key_for).expect("keyless is not an error");
        assert!(code.contains("impl zerodds_dcps::DdsType for Plain"));
        assert!(!code.contains("const HAS_KEY: bool = true"));
    }

    /// A type whose XCDR1 codec could not be planned keeps the trait's
    /// refusing defaults, and the file says which type and why.
    ///
    /// The list of types with an XCDR1 pair is passed in, so this holds
    /// the *shape* of the answer open even while every construct in the
    /// tree happens to plan under both versions. The alternative — four
    /// methods quietly delegating to the XCDR2 codec — is the fault class
    /// this bridge exists to avoid.
    #[test]
    fn without_an_xcdr1_plan_the_four_methods_are_left_refusing() {
        let ast = model("@final struct Pose { int32 id; double x; };");

        let with = dds_type_impls(
            &ast,
            &ast.types().map(|(id, _)| id).collect::<Vec<_>>(),
            &default_key_for,
        )
        .unwrap();
        assert!(with.contains("fn encode_xcdr1("), "{with}");
        assert!(with.contains("fn decode_xcdr1_be("), "{with}");

        let without = dds_type_impls(&ast, &[], &default_key_for).unwrap();
        assert!(
            !without.contains("fn encode_xcdr1("),
            "no XCDR1 codec means no override: {without}"
        );
        assert!(
            !without.contains("fn decode_xcdr1_be("),
            "no XCDR1 codec means no override: {without}"
        );
        assert!(
            without.contains("// `Pose`: no XCDR1 codec could be planned"),
            "the reason is named where the overrides would have been: {without}"
        );
        // The XCDR2 pair is untouched by the gap.
        assert!(without.contains("fn encode_be("), "{without}");
    }
}
