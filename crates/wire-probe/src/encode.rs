// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! One value, one type, the bytes between them — at
//! `serde_json::Value`.
//!
//! The walk is `zerodds_wire_codec::encode`'s and follows the layout
//! plan, not a second reading of the model. What is here is the value
//! file's end of it: the three entry points pinned to the representation
//! the probe reads, and the front end that turns IDL text into the model
//! they take — which is the probe's business and not the engine's, so
//! `zerodds-wire-codec` names no parser.
//!
//! The three carry a recursion marker for the reason `contract.rs`'s
//! module note gives: `dds_bounded_recursion` reads a path call by its
//! last segment, so forwarding to the function of the same name in the
//! other crate looks like recursion. Depth 1 each.

use serde_json::Value;
use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::contract::Consumed;
use zerodds_wire_codec::value::ProbeRequest;

pub use zerodds_wire_codec::encode::ProbeError;

/// Serialise `values` as `request.type_name`. See
/// [`zerodds_wire_codec::encode::encode`].
///
/// # Errors
/// [`ProbeError`], always naming the path in the value.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::encode::encode`, and the lint reads a
/// path call by its last segment. See the module note.
#[inline]
pub fn encode(ast: &Ast, request: &ProbeRequest, values: &Value) -> Result<Vec<u8>, ProbeError> {
    zerodds_wire_codec::encode::encode(ast, request, values)
}

/// As [`encode`], recording which scalars of the value were read. See
/// [`zerodds_wire_codec::encode::encode_recording`].
///
/// # Errors
/// As [`encode`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::encode::encode_recording`, and the
/// lint reads a path call by its last segment. See the module note.
#[inline]
pub fn encode_recording(
    ast: &Ast,
    request: &ProbeRequest,
    values: &Value,
    seen: &mut Consumed,
) -> Result<Vec<u8>, ProbeError> {
    zerodds_wire_codec::encode::encode_recording(ast, request, values, seen)
}

/// As [`encode`], and the offsets of the octets that carry no value. See
/// [`zerodds_wire_codec::encode::encode_with_padding`].
///
/// # Errors
/// As [`encode`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::encode::encode_with_padding`, and the
/// lint reads a path call by its last segment. See the module note.
#[inline]
pub fn encode_with_padding(
    ast: &Ast,
    request: &ProbeRequest,
    values: &Value,
) -> Result<(Vec<u8>, Vec<bool>), ProbeError> {
    zerodds_wire_codec::encode::encode_with_padding(ast, request, values)
}

/// Parse and lower one IDL source into a model with everything
/// resolved, which is what a probe needs before it can plan anything.
///
/// # Errors
/// [`ProbeError::NoLayout`] with the diagnostics, because a probe over
/// a model that did not parse would report a wire difference that is
/// really a syntax error.
pub fn model_from_idl(source: &str, path: &str) -> Result<Ast, ProbeError> {
    model_from_idl_in_dialect(source, path, zerodds_ast_layout::Dialect::default())
}

/// The same model, lowered under one dialect's front-end answers.
///
/// Most of a dialect reaches the bytes through the layout plan, which is
/// built later and takes a `LayoutKey`. One does not: whether a
/// `@hashid` member advances the sequential counter is settled in
/// `zerodds_idl_lower::complete`, before any plan exists, and it changes
/// the member id a peer has to match. DDS-XTypes 1.3 §7.3.1.2.1.1 states
/// the rule twice and not the same way; Cyclone and Fast DDS take one
/// reading and RTI the other. See
/// `Dialect::hashed_id_advances_the_counter`.
///
/// # Errors
/// As [`model_from_idl`].
pub fn model_from_idl_in_dialect(
    source: &str,
    path: &str,
    dialect: zerodds_ast_layout::Dialect,
) -> Result<Ast, ProbeError> {
    let options = zerodds_idl_lower::Options {
        hashed_id_advances_the_counter: if dialect.hashed_id_advances_the_counter() {
            zerodds_idl_lower::HashedIdCounter::Advances
        } else {
            zerodds_idl_lower::HashedIdCounter::Skipped
        },
        hashed_name_spelling: if dialect.hashid_reads_the_escaped_spelling() {
            zerodds_idl_lower::HashedNameSpelling::AsWritten
        } else {
            zerodds_idl_lower::HashedNameSpelling::Stripped
        },
        name_of_the_enclosing_scope: if dialect.admits_the_name_of_the_enclosing_scope() {
            zerodds_idl_lower::NameOfTheEnclosingScope::Admitted
        } else {
            zerodds_idl_lower::NameOfTheEnclosingScope::Refused
        },
        ..zerodds_idl_lower::Options::default()
    };
    model_from_idl_with(source, path, options)
}

/// The same model, with every front-end option spelled out.
///
/// # Errors
/// As [`model_from_idl`].
pub fn model_from_idl_with(
    source: &str,
    path: &str,
    options: zerodds_idl_lower::Options,
) -> Result<Ast, ProbeError> {
    // Through `zerodds-idl-compose`, which is the one load path — the
    // same one a `build.rs` and `zerodds-idlc` take. It used to be two
    // with the same intent, and they drifted: this one ran
    // `resolve_all_by_name` and checked `is_clean()`, the other ran no
    // resolve pass at all, so a member whose type lived in another
    // module reached the generator unresolved and the field never made
    // it onto the wire. The corpus held the case that would have shown
    // it (`module-issue41-shape`) and measured the correct answer, on a
    // path no user took.
    //
    // What stays different here is the strictness, not the substance: a
    // probe refuses a model with any diagnostic, because a wire
    // difference reported over a file that did not parse is not a wire
    // difference. A build reports every diagnostic and keeps going,
    // which is what a caller fixing six declarations needs.
    let composed = zerodds_idl_compose::compose_source_with(
        path,
        source,
        &zerodds_idl_compose::PathLoader::bare(),
        zerodds_idl_compose::ComposeOptions {
            default_extensibility: options.default_extensibility,
            default_nested: options.default_nested,
            hashed_id_advances_the_counter: options.hashed_id_advances_the_counter,
            hashed_name_spelling: options.hashed_name_spelling,
            name_of_the_enclosing_scope: options.name_of_the_enclosing_scope,
            defines: Vec::new(),
            // The clause permits `@key` on a union discriminator
            // (Table 21, §7.2.2.4.4.4.8), so the probe carries it: the
            // Rust column is the specification's answer, and a vendor that
            // refuses the file (measured 2026-08-10: Cyclone's `idlc`,
            // `rtiddsgen`, `fastddsgen`) shows as a refusal in its own
            // column. `Refused` stays an option of the front end.
            key_on_union_discriminator: zerodds_idl_compose::KeyOnUnionDiscriminator::Carried,
            // A probe reproduces a vendor, so it reads that vendor's
            // dialect by definition.
            dialect: zerodds_idl_compose::ComposeOptions::default().dialect,
        },
    );
    if !composed.diagnostics.is_empty() {
        // Rendered through `Display`, not `Debug`: this string is what a
        // refusal row in `tests/interop-results/ledger.tsv` records as the
        // reason, and the ledger holds the tool's own sentence. A struct
        // dump puts field names and a `Span` into that column, where the
        // clause reference belongs — and the column is truncated, so the
        // dump loses the clause and keeps the punctuation.
        return Err(ProbeError::NoLayout {
            detail: composed
                .diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        });
    }
    Ok(composed.ast)
}

/// The bytes as lowercase hex, which is how the fixtures record them.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Shorthand for the common case: one IDL source, one type, one value.
///
/// # Errors
/// As [`encode`], plus the frontend's own diagnostics.
pub fn probe(
    idl: &str,
    type_name: &str,
    values: &Value,
    encoding: Encoding,
    endianness: Endianness,
) -> Result<Vec<u8>, ProbeError> {
    let ast = model_from_idl(idl, "probe.idl")?;
    encode(
        &ast,
        &ProbeRequest::new(type_name, encoding, endianness),
        values,
    )
}
