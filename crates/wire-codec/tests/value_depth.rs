// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A recursive type has finite values; a cyclic value has none.
//!
//! The planner admits `@mutable struct Node { long value; @optional
//! @external Node next; }` because every *value* of it terminates
//! itself — `next` can be absent, and an absence ends the walk where a
//! zero-length sequence would. That is a statement about the type. It
//! says nothing about one particular value, and `a.next = a` is the
//! object graph XTypes 1.3 §7.3.1.2.1.4 calls unserializable.
//!
//! Neither walk can see the cycle: the encoder is handed one member at
//! a time by a `Source` and the decoder one header at a time by a
//! stream, and in both directions the shape is known only one step
//! ahead. So both count, and both refuse by name past
//! [`MAX_VALUE_DEPTH`]. A truncated prefix reported as success is the
//! failure this file exists to keep out.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, Extensibility, MemberSpec, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_float128::Float128;
use zerodds_wire_codec::encode::ProbeError;
use zerodds_wire_codec::source::Source;
use zerodds_wire_codec::value::ProbeRequest;
use zerodds_wire_codec::{MAX_VALUE_DEPTH, decode, encode};

/// `@mutable struct Node { long value; @optional @external Node next;
/// }` — the corpus case `recursive-optional-external`, built without a
/// parser because this crate names none.
fn node_model() -> (Ast, Id<TypeNode>) {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let node = ast
        .add_type(
            root,
            "Node",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Node");
    let any = ast.any_of_type(node).expect("any");
    ast.facets_mut(any)
        .expect("facets")
        .xtypes_mut()
        .extensibility = Some(Extensibility::Mutable);

    ast.add_member(node, MemberSpec::new("value", Ref::Resolved(long)))
        .expect("value");
    let next = ast
        .add_member(node, MemberSpec::new("next", Ref::Resolved(node)))
        .expect("next");
    let any = ast.any_of_member(next).expect("any");
    let facets = ast.facets_mut(any).expect("facets");
    facets.xtypes_mut().is_optional = true;
    facets.xtypes_mut().is_external = true;

    (ast, node)
}

fn request() -> ProbeRequest {
    ProbeRequest::new("Node", Encoding::PlCdr1, Endianness::Big)
}

// -- values that are not files -------------------------------------------

/// A `Node` value held as a program holds one, rather than as a file
/// holds one.
///
/// JSON cannot express what these tests need: a file is a tree, and the
/// value the depth guard exists for is a graph. `serde_json` also stops
/// its own parser at 128 levels, so even the legitimate deep case has
/// to be built rather than read.
enum Sample {
    /// The `long` leaf.
    Number,
    /// A `Node`, with or without a `next` below it.
    Node(Option<Box<Self>>),
    /// A `Node` that is its own `next`: `a.next = a`.
    Cyclic,
}

/// The leaf every `value` member is answered with, as a `static` so
/// that [`Source::field`] can hand out a reference to it.
static LEAF: Sample = Sample::Number;

/// A chain of `levels` `Node`s: the last one has no `next`.
fn chain(levels: usize) -> Sample {
    let mut node = Sample::Node(None);
    for _ in 1..levels {
        node = Sample::Node(Some(Box::new(node)));
    }
    node
}

impl Source for Sample {
    fn kind(&self) -> &'static str {
        match self {
            Self::Number => "a number",
            Self::Node(_) | Self::Cyclic => "an object",
        }
    }
    fn is_null(&self) -> bool {
        false
    }
    fn is_object(&self) -> bool {
        !matches!(self, Self::Number)
    }
    fn as_bool(&self) -> Option<bool> {
        None
    }
    fn as_text(&self) -> Option<&str> {
        None
    }
    fn as_float(&self) -> Option<f64> {
        None
    }
    fn as_items(&self) -> Option<&[Self]> {
        None
    }
    fn field(&self, name: &str) -> Option<&Self> {
        match (self, name) {
            (Self::Node(_) | Self::Cyclic, "value") => Some(&LEAF),
            (Self::Node(next), "next") => next.as_deref(),
            (Self::Cyclic, "next") => Some(self),
            _ => None,
        }
    }
    fn entries(&self) -> Vec<(&str, &Self)> {
        match self {
            Self::Number => Vec::new(),
            Self::Node(None) => vec![("value", &LEAF)],
            Self::Node(Some(next)) => vec![("value", &LEAF), ("next", next.as_ref())],
            Self::Cyclic => vec![("value", &LEAF), ("next", self)],
        }
    }
    fn as_integer(&self, path: &str) -> Result<i128, ProbeError> {
        match self {
            Self::Number => Ok(1),
            _ => Err(ProbeError::Mismatch {
                path: String::from(path),
                expected: String::from("a number"),
                found: String::from("a Node"),
            }),
        }
    }
    fn as_float128(&self, path: &str) -> Result<Float128, ProbeError> {
        Err(ProbeError::Mismatch {
            path: String::from(path),
            expected: String::from("a long double"),
            found: String::from(self.kind()),
        })
    }
    fn render(&self) -> String {
        String::from("<a Node>")
    }
}

// -- the encoder ---------------------------------------------------------

/// The encoder refuses `a.next = a` by name, rather than writing a
/// prefix of an infinite stream or running out of stack.
#[test]
fn a_cyclic_value_is_refused_by_name() {
    let (ast, _) = node_model();
    match encode(&ast, &request(), &Sample::Cyclic) {
        Err(ProbeError::ValueTooDeep { limit, .. }) => assert_eq!(limit, MAX_VALUE_DEPTH),
        other => panic!("a cyclic object graph was not refused: {other:?}"),
    }
}

/// And the refusal says what it is, in the words of the clause it
/// enforces: a caller reading only the message has to learn that the
/// value has a cycle in it, not that some number was exceeded.
#[test]
fn the_refusal_names_the_clause() {
    let (ast, _) = node_model();
    let message = match encode(&ast, &request(), &Sample::Cyclic) {
        Err(error) => format!("{error}"),
        Ok(bytes) => panic!("a cyclic object graph produced {} bytes", bytes.len()),
    };
    assert!(message.contains("cycle"), "{message}");
    assert!(message.contains("7.3.1.2.1.4"), "{message}");
}

/// A chain at the limit is written and a chain one level past it is
/// refused: the boundary is where the constant says it is.
#[test]
fn the_encoder_writes_up_to_the_limit_and_not_past_it() {
    let (ast, _) = node_model();
    let at_the_limit = encode(&ast, &request(), &chain(MAX_VALUE_DEPTH));
    assert!(at_the_limit.is_ok(), "{at_the_limit:?}");

    match encode(&ast, &request(), &chain(MAX_VALUE_DEPTH + 1)) {
        Err(ProbeError::ValueTooDeep { limit, .. }) => assert_eq!(limit, MAX_VALUE_DEPTH),
        other => panic!("a chain past the limit was not refused: {other:?}"),
    }
}

// -- the decoder ---------------------------------------------------------

/// A PL_CDR1 big-endian stream of `levels` nested `Node`s, built by
/// hand because the encoder will not produce one past the limit.
///
/// One level has the shape the Fast DDS recording of
/// `recursive-optional-external` has: `0000 0004` and the `long` for
/// `value`, `0001 <len>` for `next`, the level below, then the
/// terminator. The terminator here carries the must-understand bit,
/// which is `PidFlagPolicy::SentinelMustUnderstand` — what [`request`]
/// asks for; Fast DDS writes the bare `3f02`.
fn nested_stream(levels: usize) -> Vec<u8> {
    let value_member = [0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01];
    let sentinel = [0x7f, 0x02, 0x00, 0x00];

    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(&value_member);
    body.extend_from_slice(&sentinel);
    for _ in 1..levels {
        let mut outer: Vec<u8> = Vec::new();
        outer.extend_from_slice(&value_member);
        outer.extend_from_slice(&[0x00, 0x01]);
        outer.extend_from_slice(
            &u16::try_from(body.len())
                .expect("a length the short header holds")
                .to_be_bytes(),
        );
        outer.extend_from_slice(&body);
        outer.extend_from_slice(&sentinel);
        body = outer;
    }
    body
}

/// A stream that nests past the limit is refused, and the reader does
/// not follow it into the stack.
///
/// This is the direction that faces a hostile peer: nothing in a byte
/// stream bounds how deeply it nests a recursive type, and a level
/// costs twelve octets to write against a reader frame to read.
#[test]
fn a_stream_that_nests_past_the_limit_is_refused() {
    let (ast, _) = node_model();
    let bytes = nested_stream(MAX_VALUE_DEPTH + 1);
    match decode::<serde_json::Value>(&ast, &request(), &bytes) {
        Err(ProbeError::ValueTooDeep { limit, .. }) => assert_eq!(limit, MAX_VALUE_DEPTH),
        other => panic!("a stream nesting past the limit was not refused: {other:?}"),
    }
}

/// A stream at the limit reads, and reads to the bottom.
#[test]
fn a_stream_at_the_limit_reads_to_the_bottom() {
    let (ast, _) = node_model();
    let bytes = nested_stream(MAX_VALUE_DEPTH);
    let value: serde_json::Value = decode(&ast, &request(), &bytes).expect("a stream at the limit");
    assert_eq!(levels_of(&value), MAX_VALUE_DEPTH);
}

/// The two directions agree at the limit: what the encoder writes for
/// a chain that deep is what the decoder reads back.
#[test]
fn the_two_directions_agree_at_the_limit() {
    let (ast, _) = node_model();
    let bytes = encode(&ast, &request(), &chain(MAX_VALUE_DEPTH)).expect("a chain at the limit");
    let value: serde_json::Value = decode(&ast, &request(), &bytes).expect("the same bytes back");
    assert_eq!(levels_of(&value), MAX_VALUE_DEPTH);
}

/// How many `Node`s a decoded value holds, counted iteratively so the
/// count itself cannot be what runs out of stack.
fn levels_of(value: &serde_json::Value) -> usize {
    let mut levels = 1;
    let mut at = value;
    while let Some(next) = at.get("next") {
        levels += 1;
        at = next;
    }
    levels
}
