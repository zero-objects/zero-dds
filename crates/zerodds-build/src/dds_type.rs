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
    body.push_str(&field_value_item(ast, node));

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
    // A union's discriminator is a key without being a MemberNode. The
    // Rust union codec currently returns before emitting the inherent
    // key_holder_be/key_from_holder_be methods used by this bridge. Do not
    // advertise a keyless topic, or emit calls to methods that do not exist.
    if zerodds_ast_layout::keyhash::union_key_discriminator(ast, id).is_some() {
        return Err(format!(
            "DdsType key holder for `{path}`: a keyed union discriminator requires \
             Rust key_holder_be/key_from_holder_be methods, which the union codec \
             does not yet generate"
        ));
    }
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
    if members.is_empty() {
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
    if has_xcdr1 {
        let v1 = xcdr1_encoding(extensibility).max_alignment();
        let (v1_write, v1_read) = (wire_settings(v1_key, true), wire_settings(v1_key, false));
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
    out
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
fn field_value_item(ast: &Ast, node: &TypeNode) -> String {
    let TypeKind::Structure { members, .. } = &node.kind else {
        return String::new();
    };
    let mut arms = String::new();
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
            }
            _ => {}
        }
    }
    if arms.is_empty() {
        return String::new();
    }
    format!(
        "\n    fn field_value(&self, path: &str) -> Option<zerodds_dcps::FilterValue> {{\n        \
         let (head, rest) = match path.split_once('.') {{\n            \
         Some((head, rest)) => (head, Some(rest)),\n            \
         None => (path, None),\n        }};\n        \
         match (head, rest) {{\n{arms}            _ => None,\n        }}\n    }}\n"
    )
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

    #[test]
    fn a_carried_union_discriminator_cannot_acquire_keyless_dds_defaults() {
        // Build the internal model directly: the default text frontend refuses
        // this annotation earlier, so a Config test cannot reach this guard.
        let mut ast = Ast::new();
        let discriminator = ast
            .add_type(
                ast.root_module(),
                "Long",
                TypeKind::Primitive(PrimitiveKind::Int32),
            )
            .expect("discriminator");
        let union = ast
            .add_type(
                ast.root_module(),
                "Command",
                TypeKind::Union {
                    discriminator: zerodds_ast_core::Ref::Resolved(discriminator),
                    cases: Vec::new(),
                    default_case: None,
                },
            )
            .expect("union");
        let zero = ast.add_int_expr(0).expect("label");
        ast.add_case(
            union,
            vec![zerodds_ast::CaseLabel::Value(zero)],
            zerodds_ast::MemberSpec::new("value", zerodds_ast_core::Ref::Resolved(discriminator)),
        )
        .expect("case");
        let any = ast.any_of_type(union).expect("union id");
        ast.facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .discriminator_is_key = true;
        assert!(
            zerodds_ast_layout::keyhash::key_members_with(
                &ast,
                union,
                Default::default(),
                Default::default()
            )
            .expect("members")
            .is_empty(),
            "the discriminator is deliberately not a MemberNode"
        );
        let error = dds_type_impls(&ast, &[], &default_key_for)
            .expect_err("a carried discriminator must not become keyless");
        assert!(
            error.contains("DdsType key holder for `Command`"),
            "{error}"
        );
        assert!(error.contains("keyed union discriminator"), "{error}");
        assert!(
            error.contains("key_holder_be/key_from_holder_be"),
            "{error}"
        );
        ast.facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .discriminator_is_key = false;
        let keyless =
            dds_type_impls(&ast, &[], &default_key_for).expect("same union without a key");
        assert!(keyless.contains("impl zerodds_dcps::DdsType for Command"));
        assert!(!keyless.contains("const HAS_KEY: bool = true"));
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
    /// methods quietly delegating to the XCDR2 codec — is what
    /// `docs/OPEN-ITEMS.md` records as the fault class this bridge
    /// exists to avoid.
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
