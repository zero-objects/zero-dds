// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The composite values a link carries, as Rust values: the error of
//! OASIS AMQP 1.0 Part 2 §2.8.14, the delivery states of Part 3 §3.4 (and,
//! from [`crate::transactions`], the two of Part 4 §4.5) and the source
//! and target of Part 3 §3.5.3 and §3.5.4, each with its codec over
//! [`AmqpExtValue`].
//!
//! `no_std` + `alloc`. The client ([`crate::broker`]) and the connection
//! endpoint ([`crate::peer`]) share these types.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::transactions::{Declared, TransactionalState, TxnId, desc as txn_desc};

/// Numeric descriptors of the described types of this module.
pub mod desc {
    /// `amqp:error:list` (Part 2 §2.8.14).
    pub const ERROR: u64 = 0x1d;
    /// `amqp:received:list` (Part 3 §3.4.1).
    pub const RECEIVED: u64 = 0x23;
    /// `amqp:accepted:list` (§3.4.2).
    pub const ACCEPTED: u64 = 0x24;
    /// `amqp:rejected:list` (§3.4.3).
    pub const REJECTED: u64 = 0x25;
    /// `amqp:released:list` (§3.4.4).
    pub const RELEASED: u64 = 0x26;
    /// `amqp:modified:list` (§3.4.5).
    pub const MODIFIED: u64 = 0x27;
    /// `amqp:source:list` (§3.5.3).
    pub const SOURCE: u64 = 0x28;
    /// `amqp:target:list` (§3.5.4).
    pub const TARGET: u64 = 0x29;
}

/// The symbolic descriptors of the four outcomes, as `source.outcomes`
/// names them (Part 3 §3.5.3).
pub const OUTCOME_SYMBOLS: [&str; 4] = [
    "amqp:accepted:list",
    "amqp:rejected:list",
    "amqp:released:list",
    "amqp:modified:list",
];

// ============================================================================
// Value helpers.
// ============================================================================

/// The fields of a composite: the items of its list, none for another
/// value.
pub(crate) fn fields(v: &AmqpExtValue) -> &[AmqpExtValue] {
    match v {
        AmqpExtValue::List(items) => items,
        _ => &[],
    }
}

/// Field `i` of a composite; a null or a missing trailing field is `None`.
pub(crate) fn field(v: &AmqpExtValue, i: usize) -> Option<&AmqpExtValue> {
    fields(v)
        .get(i)
        .filter(|x| !matches!(x, AmqpExtValue::Null))
}

pub(crate) fn as_u32(v: Option<&AmqpExtValue>) -> Option<u32> {
    match v? {
        AmqpExtValue::Uint(x) => Some(*x),
        AmqpExtValue::Ushort(x) => Some(u32::from(*x)),
        AmqpExtValue::Ubyte(x) => Some(u32::from(*x)),
        AmqpExtValue::Ulong(x) => u32::try_from(*x).ok(),
        _ => None,
    }
}

pub(crate) fn as_u64(v: Option<&AmqpExtValue>) -> Option<u64> {
    match v? {
        AmqpExtValue::Ulong(x) => Some(*x),
        other => as_u32(Some(other)).map(u64::from),
    }
}

pub(crate) fn as_bool(v: Option<&AmqpExtValue>) -> Option<bool> {
    match v? {
        AmqpExtValue::Boolean(b) => Some(*b),
        _ => None,
    }
}

pub(crate) fn as_text(v: Option<&AmqpExtValue>) -> Option<String> {
    match v? {
        AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) => Some(s.clone()),
        _ => None,
    }
}

/// A `symbol` multiple (Part 1 §1.4): one symbol or an array of them.
pub(crate) fn symbols(v: Option<&AmqpExtValue>) -> Vec<String> {
    match v {
        Some(AmqpExtValue::Symbol(s)) => vec![s.clone()],
        Some(AmqpExtValue::Array(items)) | Some(AmqpExtValue::List(items)) => {
            items.iter().filter_map(|x| as_text(Some(x))).collect()
        }
        _ => Vec::new(),
    }
}

/// A `symbol` multiple (Part 1 §1.4): an array of symbols, or null for
/// none (an empty array has no element constructor to encode).
pub(crate) fn symbol_array<S: AsRef<str>>(names: &[S]) -> AmqpExtValue {
    if names.is_empty() {
        return AmqpExtValue::Null;
    }
    AmqpExtValue::Array(
        names
            .iter()
            .map(|n| AmqpExtValue::Symbol(n.as_ref().to_string()))
            .collect(),
    )
}

pub(crate) fn described(descriptor: u64, list: Vec<AmqpExtValue>) -> AmqpExtValue {
    AmqpExtValue::Described {
        descriptor,
        value: Box::new(AmqpExtValue::List(list)),
    }
}

/// A composite's field list without the nulls at its end (Part 1 §1.4:
/// trailing fields that are absent are null).
pub(crate) fn trimmed(mut list: Vec<AmqpExtValue>) -> Vec<AmqpExtValue> {
    while matches!(list.last(), Some(AmqpExtValue::Null)) {
        list.pop();
    }
    list
}

/// A `fields`-like map with symbol keys (Part 2 §2.8.13, Part 3 §3.5.8,
/// §3.5.9) as pairs.
fn symbol_map(v: Option<&AmqpExtValue>) -> Result<Vec<(String, AmqpExtValue)>, String> {
    match v {
        None => Ok(Vec::new()),
        Some(AmqpExtValue::Map(pairs)) => pairs
            .iter()
            .map(|(k, v)| match k {
                AmqpExtValue::Symbol(s) => Ok((s.clone(), v.clone())),
                other => Err(alloc::format!("map key {other:?} is not a symbol")),
            })
            .collect(),
        Some(other) => Err(alloc::format!("{other:?} is not a map")),
    }
}

fn encode_symbol_map(pairs: &[(String, AmqpExtValue)]) -> AmqpExtValue {
    if pairs.is_empty() {
        return AmqpExtValue::Null;
    }
    AmqpExtValue::Map(
        pairs
            .iter()
            .map(|(k, v)| (AmqpExtValue::Symbol(k.clone()), v.clone()))
            .collect(),
    )
}

// ============================================================================
// Error (Part 2 §2.8.14).
// ============================================================================

/// An `amqp:error` (Part 2 §2.8.14): condition and description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmqpError {
    /// The condition symbol, e.g. `amqp:not-found`.
    pub condition: String,
    /// The description, if any.
    pub description: Option<String>,
}

impl AmqpError {
    /// An error with `condition` and a description.
    #[must_use]
    pub fn new(condition: &str, description: impl Into<String>) -> Self {
        Self {
            condition: condition.to_string(),
            description: Some(description.into()),
        }
    }
}

impl core::fmt::Display for AmqpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.description {
            Some(d) => write!(f, "{}: {d}", self.condition),
            None => f.write_str(&self.condition),
        }
    }
}

/// Reads an `error` field; `None` for null or another value.
#[must_use]
pub fn decode_error(v: Option<&AmqpExtValue>) -> Option<AmqpError> {
    match v? {
        AmqpExtValue::Described { descriptor, value } if *descriptor == desc::ERROR => {
            Some(AmqpError {
                condition: as_text(field(value, 0)).unwrap_or_default(),
                description: as_text(field(value, 1)),
            })
        }
        _ => None,
    }
}

/// The described list of an error. A condition that is no ASCII symbol
/// cannot be written (Part 1 §1.6.21); it becomes `amqp:internal-error`
/// with the condition in the description.
#[must_use]
pub fn encode_error(e: &AmqpError) -> AmqpExtValue {
    let (condition, description) = if e.condition.is_ascii() && !e.condition.is_empty() {
        (e.condition.clone(), e.description.clone())
    } else {
        (
            "amqp:internal-error".to_string(),
            Some(alloc::format!("{e}")),
        )
    };
    described(
        desc::ERROR,
        trimmed(vec![
            AmqpExtValue::Symbol(condition),
            description.map_or(AmqpExtValue::Null, AmqpExtValue::Str),
        ]),
    )
}

// ============================================================================
// Delivery states (Part 3 §3.4).
// ============================================================================

/// The outcome of a delivery (Part 3 §3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// `accepted`.
    Accepted,
    /// `rejected`, with the error.
    Rejected(Option<AmqpError>),
    /// `released`: not processed, may be sent again.
    Released,
    /// `modified`.
    Modified {
        /// The delivery failed and counts towards redelivery.
        delivery_failed: bool,
        /// Must not be delivered to this link again.
        undeliverable_here: bool,
    },
    /// A non-terminal `received` state or none.
    None,
}

/// Reads a delivery state as an outcome; `received`, null and any other
/// value are [`Outcome::None`].
#[must_use]
pub fn decode_outcome(v: Option<&AmqpExtValue>) -> Outcome {
    match v {
        Some(AmqpExtValue::Described { descriptor, value }) => match *descriptor {
            desc::ACCEPTED => Outcome::Accepted,
            desc::REJECTED => Outcome::Rejected(decode_error(field(value, 0))),
            desc::RELEASED => Outcome::Released,
            desc::MODIFIED => Outcome::Modified {
                delivery_failed: as_bool(field(value, 0)).unwrap_or(false),
                undeliverable_here: as_bool(field(value, 1)).unwrap_or(false),
            },
            _ => Outcome::None,
        },
        _ => Outcome::None,
    }
}

/// The `message-annotations` field of a `modified` outcome (Part 3 §3.4.5):
/// the entries the sender combines with the message's own annotations
/// before it delivers the message again. Empty for any other state, and
/// for a `modified` without the field.
///
/// # Errors
/// A field that is neither null nor a map (Part 3 §3.4.5 types it
/// `fields`, a map with symbol keys).
pub fn modified_annotations(
    v: Option<&AmqpExtValue>,
) -> Result<Vec<(AmqpExtValue, AmqpExtValue)>, String> {
    let Some(AmqpExtValue::Described { descriptor, value }) = v else {
        return Ok(Vec::new());
    };
    if *descriptor != desc::MODIFIED {
        return Ok(Vec::new());
    }
    match field(value, 2) {
        None | Some(AmqpExtValue::Null) => Ok(Vec::new()),
        Some(AmqpExtValue::Map(entries)) => {
            if let Some((k, _)) = entries
                .iter()
                .find(|(k, _)| !matches!(k, AmqpExtValue::Symbol(_)))
            {
                return Err(format!(
                    "modified.message-annotations: the key {k:?} is not a symbol"
                ));
            }
            Ok(entries.clone())
        }
        Some(other) => Err(format!(
            "modified.message-annotations is {other:?}, not a map"
        )),
    }
}

/// The described list of an outcome; null for [`Outcome::None`].
#[must_use]
pub fn encode_outcome(o: &Outcome) -> AmqpExtValue {
    match o {
        Outcome::Accepted => described(desc::ACCEPTED, vec![]),
        Outcome::Rejected(e) => described(
            desc::REJECTED,
            vec![e.as_ref().map_or(AmqpExtValue::Null, encode_error)],
        ),
        Outcome::Released => described(desc::RELEASED, vec![]),
        Outcome::Modified {
            delivery_failed,
            undeliverable_here,
        } => described(
            desc::MODIFIED,
            vec![
                AmqpExtValue::Boolean(*delivery_failed),
                AmqpExtValue::Boolean(*undeliverable_here),
            ],
        ),
        Outcome::None => AmqpExtValue::Null,
    }
}

/// A delivery state: the non-terminal `received` (Part 3 §3.4.1), a
/// terminal outcome (§3.4.2 to §3.4.5), or one of the two states of Part 4
/// (§4.5.5 `declared`, §4.5.6 `transactional-state`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryState {
    /// `received`: how much of the message the receiver holds.
    Received {
        /// The section of the message the offset is in, counted from 0.
        section_number: u32,
        /// The first octet of that section not yet received.
        section_offset: u64,
    },
    /// A terminal outcome (never [`Outcome::None`]).
    Outcome(Outcome),
    /// `declared` (Part 4 §4.5.5): the outcome of a declare, with the
    /// identifier of the transaction.
    Declared(TxnId),
    /// `transactional-state` (Part 4 §4.5.6): the delivery belongs to a
    /// transaction; its outcome applies if the transaction commits.
    Transactional(TransactionalState),
}

impl DeliveryState {
    /// The state for an outcome; `None` for [`Outcome::None`].
    #[must_use]
    pub fn of(outcome: Outcome) -> Option<Self> {
        match outcome {
            Outcome::None => None,
            terminal => Some(Self::Outcome(terminal)),
        }
    }

    /// The outcome of Part 3 this state is: [`Outcome::None`] for
    /// `received` and for the two states of Part 4.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        match self {
            Self::Outcome(o) => o.clone(),
            Self::Received { .. } | Self::Declared(_) | Self::Transactional(_) => Outcome::None,
        }
    }
}

/// Reads a delivery state; `Ok(None)` for null and for a value that is no
/// delivery state of Part 3 or Part 4.
///
/// # Errors
/// A `declared` or a `transactional-state` without a txn-id, with a
/// txn-id that is none (Part 4 §4.5.4), or with an outcome that is none.
pub fn decode_state(v: Option<&AmqpExtValue>) -> Result<Option<DeliveryState>, String> {
    decode_state_up_to(v, crate::transactions::TxnId::MAX_LEN)
}

/// [`decode_state`] with txn-ids of at most `max_txn_id` octets in a
/// `declared` or a `transactional-state`
/// ([`crate::transactions::TxnId::decode_up_to`]).
///
/// # Errors
/// As [`decode_state`].
pub fn decode_state_up_to(
    v: Option<&AmqpExtValue>,
    max_txn_id: usize,
) -> Result<Option<DeliveryState>, String> {
    let Some(value) = v else {
        return Ok(None);
    };
    match value {
        AmqpExtValue::Described { descriptor, value } if *descriptor == desc::RECEIVED => {
            Ok(Some(DeliveryState::Received {
                section_number: as_u32(field(value, 0)).unwrap_or(0),
                section_offset: as_u64(field(value, 1)).unwrap_or(0),
            }))
        }
        AmqpExtValue::Described { descriptor, value } if *descriptor == txn_desc::DECLARED => {
            Declared::decode_list_up_to(value, max_txn_id)
                .map(|d| Some(DeliveryState::Declared(d.txn_id)))
        }
        AmqpExtValue::Described { descriptor, value }
            if *descriptor == txn_desc::TRANSACTIONAL_STATE =>
        {
            TransactionalState::decode_list_up_to(value, max_txn_id)
                .map(|t| Some(DeliveryState::Transactional(t)))
        }
        other => Ok(DeliveryState::of(decode_outcome(Some(other)))),
    }
}

/// The described list of a delivery state.
#[must_use]
pub fn encode_state(state: &DeliveryState) -> AmqpExtValue {
    match state {
        DeliveryState::Received {
            section_number,
            section_offset,
        } => described(
            desc::RECEIVED,
            vec![
                AmqpExtValue::Uint(*section_number),
                AmqpExtValue::Ulong(*section_offset),
            ],
        ),
        DeliveryState::Outcome(o) => encode_outcome(o),
        DeliveryState::Declared(txn_id) => Declared {
            txn_id: txn_id.clone(),
        }
        .encode(),
        DeliveryState::Transactional(state) => state.encode(),
    }
}

// ============================================================================
// Source and target (Part 3 §3.5).
// ============================================================================

/// `terminus-expiry-policy` (Part 3 §3.5.6): when the expiry timer of a
/// terminus starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExpiryPolicy {
    /// `link-detach`.
    LinkDetach,
    /// `session-end`, the default.
    #[default]
    SessionEnd,
    /// `connection-close`.
    ConnectionClose,
    /// `never`.
    Never,
}

impl ExpiryPolicy {
    /// The symbol of the policy.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::LinkDetach => "link-detach",
            Self::SessionEnd => "session-end",
            Self::ConnectionClose => "connection-close",
            Self::Never => "never",
        }
    }

    /// The policy `symbol` names.
    #[must_use]
    pub fn from_symbol(symbol: &str) -> Option<Self> {
        [
            Self::LinkDetach,
            Self::SessionEnd,
            Self::ConnectionClose,
            Self::Never,
        ]
        .into_iter()
        .find(|p| p.symbol() == symbol)
    }
}

/// What source and target share (Part 3 §3.5.3, §3.5.4: fields 0 to 5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Terminus {
    /// `address`: the node. `None` for a dynamic node not yet created and
    /// for a minimal terminus.
    pub address: Option<String>,
    /// `durable` (§3.5.5): 0 none, 1 configuration, 2 unsettled-state.
    pub durable: u32,
    /// `expiry-policy` (§3.5.6).
    pub expiry_policy: ExpiryPolicy,
    /// `timeout`: seconds an expiring terminus is retained.
    pub timeout: u32,
    /// `dynamic`: a request to create a node, or the statement that one
    /// was created.
    pub dynamic: bool,
    /// `dynamic-node-properties` (§3.5.9), e.g. `lifetime-policy`.
    pub dynamic_node_properties: Vec<(String, AmqpExtValue)>,
    /// `capabilities`.
    pub capabilities: Vec<String>,
}

/// A source (Part 3 §3.5.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Source {
    /// Address, durability, expiry, dynamic node, capabilities.
    pub terminus: Terminus,
    /// `distribution-mode` (§3.5.7): `move` or `copy`.
    pub distribution_mode: Option<String>,
    /// `filter` (§3.5.8): named predicates.
    pub filter: Vec<(String, AmqpExtValue)>,
    /// `default-outcome`: the outcome of a transfer settled without a
    /// terminal state.
    pub default_outcome: Option<Outcome>,
    /// `outcomes`: symbolic descriptors of the outcomes the link can
    /// carry.
    pub outcomes: Vec<String>,
}

/// A target (Part 3 §3.5.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Target {
    /// Address, durability, expiry, dynamic node, capabilities.
    pub terminus: Terminus,
}

/// Fields 0 to 5 of a source or target.
fn decode_terminus(value: &AmqpExtValue, what: &str) -> Result<Terminus, String> {
    let address = match field(value, 0) {
        None => None,
        Some(AmqpExtValue::Str(s)) => Some(s.clone()),
        Some(other) => {
            return Err(alloc::format!(
                "{what}.address {other:?} is not an address string"
            ));
        }
    };
    let durable = match field(value, 1) {
        None => 0,
        Some(v) => match as_u32(Some(v)) {
            Some(d @ 0..=2) => d,
            _ => return Err(alloc::format!("{what}.durable {v:?} is not 0, 1 or 2")),
        },
    };
    let expiry_policy = match field(value, 2) {
        None => ExpiryPolicy::default(),
        Some(v) => as_text(Some(v))
            .and_then(|s| ExpiryPolicy::from_symbol(&s))
            .ok_or_else(|| alloc::format!("{what}.expiry-policy {v:?} is no expiry policy"))?,
    };
    let timeout = match field(value, 3) {
        None => 0,
        Some(v) => as_u32(Some(v))
            .ok_or_else(|| alloc::format!("{what}.timeout {v:?} is not a number of seconds"))?,
    };
    let dynamic = match field(value, 4) {
        None => false,
        Some(v) => as_bool(Some(v))
            .ok_or_else(|| alloc::format!("{what}.dynamic {v:?} is not a boolean"))?,
    };
    let dynamic_node_properties = symbol_map(field(value, 5))
        .map_err(|e| alloc::format!("{what}.dynamic-node-properties: {e}"))?;
    if !dynamic && !dynamic_node_properties.is_empty() {
        return Err(alloc::format!(
            "{what}.dynamic-node-properties is set and {what}.dynamic is not"
        ));
    }
    Ok(Terminus {
        address,
        durable,
        expiry_policy,
        timeout,
        dynamic,
        dynamic_node_properties,
        capabilities: Vec::new(),
    })
}

fn encode_terminus(t: &Terminus) -> Vec<AmqpExtValue> {
    vec![
        t.address
            .clone()
            .map_or(AmqpExtValue::Null, AmqpExtValue::Str),
        if t.durable == 0 {
            AmqpExtValue::Null
        } else {
            AmqpExtValue::Uint(t.durable)
        },
        if t.expiry_policy == ExpiryPolicy::default() {
            AmqpExtValue::Null
        } else {
            AmqpExtValue::Symbol(t.expiry_policy.symbol().to_string())
        },
        if t.timeout == 0 {
            AmqpExtValue::Null
        } else {
            AmqpExtValue::Uint(t.timeout)
        },
        if t.dynamic {
            AmqpExtValue::Boolean(true)
        } else {
            AmqpExtValue::Null
        },
        encode_symbol_map(&t.dynamic_node_properties),
    ]
}

impl Source {
    /// A source at `address`, every other field at its default.
    #[must_use]
    pub fn at(address: &str) -> Self {
        Self {
            terminus: Terminus {
                address: Some(address.to_string()),
                ..Terminus::default()
            },
            ..Self::default()
        }
    }

    /// Reads the `source` field of an attach: `Ok(None)` for null (no
    /// source), the source for a described list `0x28`.
    ///
    /// # Errors
    /// The field that does not hold what §3.5.3 gives it.
    pub fn decode(v: Option<&AmqpExtValue>) -> Result<Option<Self>, String> {
        let Some(v) = v else { return Ok(None) };
        let AmqpExtValue::Described { descriptor, value } = v else {
            return Err(alloc::format!("{v:?} is not a source"));
        };
        if *descriptor != desc::SOURCE {
            return Err(alloc::format!(
                "descriptor {descriptor:#x} is not the one of a source"
            ));
        }
        let mut terminus = decode_terminus(value, "source")?;
        terminus.capabilities = symbols(field(value, 10));
        let default_outcome = match field(value, 8) {
            None => None,
            Some(o) => match decode_outcome(Some(o)) {
                Outcome::None => {
                    return Err(alloc::format!("source.default-outcome {o:?} is no outcome"));
                }
                outcome => Some(outcome),
            },
        };
        Ok(Some(Self {
            terminus,
            distribution_mode: as_text(field(value, 6)),
            filter: symbol_map(field(value, 7))
                .map_err(|e| alloc::format!("source.filter: {e}"))?,
            default_outcome,
            outcomes: symbols(field(value, 9)),
        }))
    }

    /// The described list `0x28`.
    #[must_use]
    pub fn encode(&self) -> AmqpExtValue {
        let mut list = encode_terminus(&self.terminus);
        list.push(
            self.distribution_mode
                .clone()
                .map_or(AmqpExtValue::Null, AmqpExtValue::Symbol),
        );
        list.push(encode_symbol_map(&self.filter));
        list.push(
            self.default_outcome
                .as_ref()
                .map_or(AmqpExtValue::Null, encode_outcome),
        );
        list.push(symbol_array(&self.outcomes));
        list.push(symbol_array(&self.terminus.capabilities));
        described(desc::SOURCE, trimmed(list))
    }
}

impl Target {
    /// A target at `address`, every other field at its default.
    #[must_use]
    pub fn at(address: &str) -> Self {
        Self {
            terminus: Terminus {
                address: Some(address.to_string()),
                ..Terminus::default()
            },
        }
    }

    /// Reads the `target` field of an attach: `Ok(None)` for null (no
    /// target), the target for a described list `0x29`. A described value
    /// with another descriptor (Part 4: a coordinator) is an error here.
    ///
    /// # Errors
    /// The field that does not hold what §3.5.4 gives it.
    pub fn decode(v: Option<&AmqpExtValue>) -> Result<Option<Self>, String> {
        let Some(v) = v else { return Ok(None) };
        let AmqpExtValue::Described { descriptor, value } = v else {
            return Err(alloc::format!("{v:?} is not a target"));
        };
        if *descriptor != desc::TARGET {
            return Err(alloc::format!(
                "descriptor {descriptor:#x} is not the one of a target"
            ));
        }
        let mut terminus = decode_terminus(value, "target")?;
        terminus.capabilities = symbols(field(value, 6));
        Ok(Some(Self { terminus }))
    }

    /// The described list `0x29`.
    #[must_use]
    pub fn encode(&self) -> AmqpExtValue {
        let mut list = encode_terminus(&self.terminus);
        list.push(symbol_array(&self.terminus.capabilities));
        described(desc::TARGET, trimmed(list))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn octets(v: &AmqpExtValue) -> Vec<u8> {
        v.encode().expect("encode")
    }

    fn read(octets: &[u8]) -> AmqpExtValue {
        let (v, n) = AmqpExtValue::decode(octets).expect("decode");
        assert_eq!(n, octets.len());
        v
    }

    /// Part 3 §3.5.3: a source is the described list 0x28 with its eleven
    /// fields in order; absent trailing fields are left out.
    #[test]
    fn amqp10_3_5_3_a_source_is_0x28_and_its_fields_in_order() {
        // address "q" only: 0x00 0x53 0x28, list8 of one string.
        assert_eq!(
            octets(&Source::at("q").encode()),
            [0x00, 0x53, 0x28, 0xc0, 0x04, 0x01, 0xa1, 0x01, b'q']
        );
        let full = Source {
            terminus: Terminus {
                address: Some("q".into()),
                durable: 2,
                expiry_policy: ExpiryPolicy::Never,
                timeout: 30,
                dynamic: false,
                dynamic_node_properties: Vec::new(),
                capabilities: vec!["queue".into()],
            },
            distribution_mode: Some("move".into()),
            filter: vec![(
                "f".into(),
                AmqpExtValue::DescribedSymbol {
                    descriptor: "apache.org:selector-filter:string".into(),
                    value: Box::new(AmqpExtValue::Str("a = 1".into())),
                },
            )],
            default_outcome: Some(Outcome::Released),
            outcomes: OUTCOME_SYMBOLS.iter().map(|s| (*s).to_string()).collect(),
        };
        let value = full.encode();
        let AmqpExtValue::Described { descriptor, value } = &value else {
            panic!("a source is described");
        };
        assert_eq!(*descriptor, 0x28);
        let f = fields(value);
        assert_eq!(f.len(), 11);
        assert_eq!(f[0], AmqpExtValue::Str("q".into()));
        assert_eq!(f[1], AmqpExtValue::Uint(2));
        assert_eq!(f[2], AmqpExtValue::Symbol("never".into()));
        assert_eq!(f[3], AmqpExtValue::Uint(30));
        assert_eq!(f[4], AmqpExtValue::Null);
        assert_eq!(f[5], AmqpExtValue::Null);
        assert_eq!(f[6], AmqpExtValue::Symbol("move".into()));
        assert!(matches!(&f[7], AmqpExtValue::Map(m) if m.len() == 1));
        assert_eq!(f[8], described(desc::RELEASED, vec![]));
        assert!(matches!(&f[9], AmqpExtValue::Array(a) if a.len() == 4));
        assert_eq!(
            f[10],
            AmqpExtValue::Array(vec![AmqpExtValue::Symbol("queue".into())])
        );
        // Through the wire and back.
        let back = Source::decode(Some(&read(&octets(&full.encode()))))
            .expect("decode")
            .expect("a source");
        assert_eq!(back, full);
    }

    /// §3.5.3, §3.5.4: null is no terminus; a minimal terminus has every
    /// field unset; the defaults are durable none, expiry session-end,
    /// timeout 0, dynamic false.
    #[test]
    fn amqp10_3_5_3_an_unset_field_is_its_default() {
        assert_eq!(Source::decode(None), Ok(None));
        assert_eq!(Target::decode(None), Ok(None));
        let minimal = Source::decode(Some(&described(desc::SOURCE, vec![])))
            .expect("decode")
            .expect("a source");
        assert_eq!(minimal, Source::default());
        assert_eq!(minimal.terminus.address, None);
        assert_eq!(minimal.terminus.durable, 0);
        assert_eq!(minimal.terminus.expiry_policy, ExpiryPolicy::SessionEnd);
        assert_eq!(minimal.terminus.timeout, 0);
        assert!(!minimal.terminus.dynamic);
        assert_eq!(octets(&minimal.encode()), [0x00, 0x53, 0x28, 0x45]);
        assert_eq!(
            octets(&Target::default().encode()),
            [0x00, 0x53, 0x29, 0x45]
        );
    }

    /// §3.5.4: a target is the described list 0x29 with its seven fields;
    /// a dynamic target carries the properties of the node to create.
    #[test]
    fn amqp10_3_5_4_a_target_is_0x29_and_its_fields_in_order() {
        assert_eq!(
            octets(&Target::at("t").encode()),
            [0x00, 0x53, 0x29, 0xc0, 0x04, 0x01, 0xa1, 0x01, b't']
        );
        let dynamic = Target {
            terminus: Terminus {
                address: None,
                durable: 0,
                expiry_policy: ExpiryPolicy::LinkDetach,
                timeout: 0,
                dynamic: true,
                dynamic_node_properties: vec![("lifetime-policy".into(), described(0x2b, vec![]))],
                capabilities: vec!["temporary-queue".into()],
            },
        };
        let back = Target::decode(Some(&read(&octets(&dynamic.encode()))))
            .expect("decode")
            .expect("a target");
        assert_eq!(back, dynamic);
        let AmqpExtValue::Described { value, .. } = dynamic.encode() else {
            panic!("a target is described");
        };
        let f = fields(&value);
        assert_eq!(f.len(), 7);
        assert_eq!(f[2], AmqpExtValue::Symbol("link-detach".into()));
        assert_eq!(f[4], AmqpExtValue::Boolean(true));
    }

    /// §3.5.3, §3.5.4: a field with a value its type does not admit is
    /// refused by name, and so are node properties without `dynamic`.
    #[test]
    fn amqp10_3_5_3_a_field_of_the_wrong_type_is_refused_by_name() {
        let source = |list: Vec<AmqpExtValue>| Source::decode(Some(&described(desc::SOURCE, list)));
        let err = source(vec![AmqpExtValue::Uint(7)]).expect_err("address");
        assert!(err.contains("source.address"), "{err}");
        let err = source(vec![AmqpExtValue::Null, AmqpExtValue::Uint(3)]).expect_err("durable");
        assert!(err.contains("source.durable"), "{err}");
        let err = source(vec![
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Symbol("sometime".into()),
        ])
        .expect_err("expiry");
        assert!(err.contains("expiry-policy"), "{err}");
        let err = source(vec![
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Map(vec![(
                AmqpExtValue::Symbol("lifetime-policy".into()),
                described(0x2b, vec![]),
            )]),
        ])
        .expect_err("node properties without dynamic");
        assert!(err.contains("dynamic-node-properties"), "{err}");
        let mut with_default = vec![AmqpExtValue::Null; 9];
        with_default[8] = described(desc::RECEIVED, vec![]);
        let err = source(with_default).expect_err("default-outcome");
        assert!(err.contains("default-outcome"), "{err}");
        // A coordinator (Part 4, 0x30) is no target.
        let err = Target::decode(Some(&described(0x30, vec![]))).expect_err("coordinator");
        assert!(err.contains("0x30"), "{err}");
        assert!(Source::decode(Some(&AmqpExtValue::Str("q".into()))).is_err());
    }

    /// §3.5.6: the four expiry policies by their symbols.
    #[test]
    fn amqp10_3_5_6_the_expiry_policies_are_their_four_symbols() {
        for (policy, symbol) in [
            (ExpiryPolicy::LinkDetach, "link-detach"),
            (ExpiryPolicy::SessionEnd, "session-end"),
            (ExpiryPolicy::ConnectionClose, "connection-close"),
            (ExpiryPolicy::Never, "never"),
        ] {
            assert_eq!(policy.symbol(), symbol);
            assert_eq!(ExpiryPolicy::from_symbol(symbol), Some(policy));
        }
        assert_eq!(ExpiryPolicy::from_symbol("link-close"), None);
    }

    /// Part 3 §3.4.1: `received` is 0x23 with section-number and
    /// section-offset; the terminal states are the outcomes.
    #[test]
    fn amqp10_3_4_1_received_is_0x23_section_number_and_offset() {
        let state = DeliveryState::Received {
            section_number: 2,
            section_offset: 300,
        };
        // 0x00 0x53 0x23, list8: smalluint 2, ulong 300.
        assert_eq!(
            octets(&encode_state(&state)),
            [
                0x00, 0x53, 0x23, 0xc0, 0x0c, 0x02, 0x52, 0x02, 0x80, 0, 0, 0, 0, 0, 0, 0x01, 0x2c
            ]
        );
        assert_eq!(
            decode_state(Some(&read(&octets(&encode_state(&state))))).expect("a state"),
            Some(state)
        );
        assert_eq!(
            decode_state(Some(&described(desc::ACCEPTED, vec![]))),
            Ok(Some(DeliveryState::Outcome(Outcome::Accepted)))
        );
        assert_eq!(decode_state(None), Ok(None));
        assert_eq!(decode_state(Some(&AmqpExtValue::Uint(1))), Ok(None));
        // A received state is no outcome.
        assert_eq!(
            decode_outcome(Some(&described(desc::RECEIVED, vec![]))),
            Outcome::None
        );
    }

    /// Part 2 §2.8.14: an error is 0x1d with condition and description; a
    /// condition that is no symbol is written as `amqp:internal-error`.
    #[test]
    fn amqp10_2_8_14_an_error_is_0x1d_condition_and_description() {
        let e = AmqpError::new("amqp:not-found", "no node q");
        let v = encode_error(&e);
        assert_eq!(decode_error(Some(&read(&octets(&v)))), Some(e));
        let bare = AmqpError {
            condition: "amqp:link:stolen".into(),
            description: None,
        };
        assert_eq!(
            octets(&encode_error(&bare))[..6],
            [0x00, 0x53, 0x1d, 0xc0, 0x13, 0x01]
        );
        let odd = AmqpError::new("fehler:größe", "x");
        let back = decode_error(Some(&encode_error(&odd))).expect("an error");
        assert_eq!(back.condition, "amqp:internal-error");
        assert!(back.description.expect("description").contains("größe"));
        assert_eq!(decode_error(None), None);
    }
}
