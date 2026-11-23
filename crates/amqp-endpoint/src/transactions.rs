// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The types of OASIS AMQP 1.0 Part 4 (Transactions) §4.5 as values.
//!
//! * §4.5.1 [`Coordinator`]: the target of the control link, descriptor
//!   `0x30`, with the capabilities of §4.5.7 ([`TxnCapability`]).
//! * §4.5.2 [`Declare`] and §4.5.3 [`Discharge`]: the bodies of the two
//!   messages a controller sends to the coordinator, each in one
//!   `amqp-value` section ([`declare_message`], [`discharge_message`],
//!   read with [`coordinator_request`]).
//! * §4.5.4 [`TxnId`]: up to 32 octets of binary.
//! * §4.5.5 [`Declared`] and §4.5.6 [`TransactionalState`]: the two
//!   delivery states; [`crate::terminus::DeliveryState`] carries them.
//! * §4.5.8 [`TransactionError`]: the three error conditions.
//!
//! `no_std` + `alloc`.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_amqp_bridge::types::TypeError;

use crate::terminus::{
    AmqpError, Outcome, decode_outcome, described, encode_outcome, field, fields, trimmed,
};

/// The numeric descriptors of Part 4 §4.5.
pub mod desc {
    /// `amqp:coordinator:list` (§4.5.1).
    pub const COORDINATOR: u64 = 0x30;
    /// `amqp:declare:list` (§4.5.2).
    pub const DECLARE: u64 = 0x31;
    /// `amqp:discharge:list` (§4.5.3).
    pub const DISCHARGE: u64 = 0x32;
    /// `amqp:declared:list` (§4.5.5).
    pub const DECLARED: u64 = 0x33;
    /// `amqp:transactional-state:list` (§4.5.6).
    pub const TRANSACTIONAL_STATE: u64 = 0x34;
}

/// The key of the transaction identifier in `flow.properties` (§4.4.3).
pub const FLOW_TXN_ID: &str = "txn-id";

// ============================================================================
// §4.5.4 transaction-id.
// ============================================================================

/// A transaction identifier (§4.5.4): "up to 32 octets of binary data".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TxnId(Vec<u8>);

impl TxnId {
    /// The longest identifier.
    pub const MAX_LEN: usize = 32;

    /// The identifier with these octets; `None` for more than 32.
    #[must_use]
    pub fn new(octets: &[u8]) -> Option<Self> {
        (octets.len() <= Self::MAX_LEN).then(|| Self(octets.to_vec()))
    }

    /// The octets.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Reads a field that `requires="txn-id"`.
    ///
    /// # Errors
    /// The value that is no binary of at most 32 octets.
    pub fn decode(v: Option<&AmqpExtValue>) -> Result<Self, String> {
        Self::decode_up_to(v, Self::MAX_LEN)
    }

    /// [`TxnId::decode`] with the bound `max` in place of the 32 octets of
    /// §4.5.4: a controller told so reads the identifiers of a resource
    /// that issues longer ones (Apache ActiveMQ Artemis issues 36 octets,
    /// the text of a UUID) and names the transaction with them.
    ///
    /// # Errors
    /// The value that is no binary of at most `max` octets.
    pub fn decode_up_to(v: Option<&AmqpExtValue>, max: usize) -> Result<Self, String> {
        match v {
            Some(AmqpExtValue::Binary(octets)) if octets.len() <= max => Ok(Self(octets.clone())),
            Some(AmqpExtValue::Binary(octets)) => Err(format!(
                "a txn-id of {} octets, more than {max}",
                octets.len()
            )),
            Some(other) => Err(format!("txn-id {other:?} is not a binary")),
            None => Err("no txn-id".to_string()),
        }
    }

    /// The binary value.
    #[must_use]
    pub fn encode(&self) -> AmqpExtValue {
        AmqpExtValue::Binary(self.0.clone())
    }
}

impl core::fmt::Display for TxnId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for octet in &self.0 {
            write!(f, "{octet:02x}")?;
        }
        Ok(())
    }
}

// ============================================================================
// §4.5.7 txn-capability, §4.5.1 coordinator.
// ============================================================================

/// A capability of a transaction coordinator (§4.5.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TxnCapability {
    /// `amqp:local-transactions`.
    LocalTransactions,
    /// `amqp:distributed-transactions`.
    DistributedTransactions,
    /// `amqp:promotable-transactions`.
    PromotableTransactions,
    /// `amqp:multi-txns-per-ssn`: several active transactions on one
    /// session.
    MultiTxnsPerSsn,
    /// `amqp:multi-ssns-per-txn`: a txn-id used across the sessions of one
    /// connection.
    MultiSsnsPerTxn,
}

impl TxnCapability {
    /// The five capabilities.
    pub const ALL: [Self; 5] = [
        Self::LocalTransactions,
        Self::DistributedTransactions,
        Self::PromotableTransactions,
        Self::MultiTxnsPerSsn,
        Self::MultiSsnsPerTxn,
    ];

    /// The symbol.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::LocalTransactions => "amqp:local-transactions",
            Self::DistributedTransactions => "amqp:distributed-transactions",
            Self::PromotableTransactions => "amqp:promotable-transactions",
            Self::MultiTxnsPerSsn => "amqp:multi-txns-per-ssn",
            Self::MultiSsnsPerTxn => "amqp:multi-ssns-per-txn",
        }
    }

    /// The capability a symbol names.
    #[must_use]
    pub fn from_symbol(symbol: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.symbol() == symbol)
    }
}

/// The target of a control link (§4.5.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Coordinator {
    /// From the controller: the capabilities it desires. From the
    /// resource: the ones the coordinator has.
    pub capabilities: Vec<TxnCapability>,
}

impl Coordinator {
    /// Reads the `target` field of an attach: `Ok(None)` for a value that
    /// is no coordinator.
    ///
    /// # Errors
    /// A coordinator whose `capabilities` is no symbol, no array of
    /// symbols, or holds a symbol that is no txn-capability.
    pub fn decode(v: Option<&AmqpExtValue>) -> Result<Option<Self>, String> {
        let Some(AmqpExtValue::Described { descriptor, value }) = v else {
            return Ok(None);
        };
        if *descriptor != desc::COORDINATOR {
            return Ok(None);
        }
        let names: Vec<&AmqpExtValue> = match field(value, 0) {
            None => Vec::new(),
            Some(AmqpExtValue::Array(items)) => items.iter().collect(),
            Some(one) => vec![one],
        };
        let mut capabilities = Vec::with_capacity(names.len());
        for name in names {
            let AmqpExtValue::Symbol(symbol) = name else {
                return Err(format!("coordinator.capabilities {name:?} is not a symbol"));
            };
            let capability = TxnCapability::from_symbol(symbol)
                .ok_or_else(|| format!("coordinator.capabilities: no txn-capability {symbol}"))?;
            if !capabilities.contains(&capability) {
                capabilities.push(capability);
            }
        }
        Ok(Some(Self { capabilities }))
    }

    /// The described list `0x30`.
    #[must_use]
    pub fn encode(&self) -> AmqpExtValue {
        let capabilities = match self.capabilities.as_slice() {
            [] => AmqpExtValue::Null,
            many => AmqpExtValue::Array(
                many.iter()
                    .map(|c| AmqpExtValue::Symbol(c.symbol().to_string()))
                    .collect(),
            ),
        };
        described(desc::COORDINATOR, trimmed(vec![capabilities]))
    }

    /// Whether the coordinator has `capability`.
    #[must_use]
    pub fn has(&self, capability: TxnCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// The capabilities of `desired` the coordinator does not have
    /// (§4.5.1: "it is the responsibility of the transaction controller to
    /// verify that the capabilities of the controller meet its
    /// requirements").
    #[must_use]
    pub fn lacking(&self, desired: &[TxnCapability]) -> Vec<TxnCapability> {
        let mut lacking = Vec::new();
        for capability in desired {
            if !self.has(*capability) && !lacking.contains(capability) {
                lacking.push(*capability);
            }
        }
        lacking
    }
}

// ============================================================================
// §4.5.2 declare, §4.5.3 discharge.
// ============================================================================

/// The body of the message that declares a transaction (§4.5.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Declare {
    /// The global transaction the txn-id is to be associated with; not
    /// set for a local transaction.
    pub global_id: Option<AmqpExtValue>,
}

impl Declare {
    /// The described list `0x31`.
    #[must_use]
    pub fn encode(&self) -> AmqpExtValue {
        described(
            desc::DECLARE,
            trimmed(vec![self.global_id.clone().unwrap_or(AmqpExtValue::Null)]),
        )
    }
}

/// The body of the message that discharges a transaction (§4.5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discharge {
    /// The transaction.
    pub txn_id: TxnId,
    /// The work of the transaction failed: roll it back. Not set on the
    /// wire is false.
    pub fail: bool,
}

impl Discharge {
    /// The described list `0x32`.
    #[must_use]
    pub fn encode(&self) -> AmqpExtValue {
        described(
            desc::DISCHARGE,
            vec![self.txn_id.encode(), AmqpExtValue::Boolean(self.fail)],
        )
    }
}

/// What a message to the coordinator asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum CoordinatorRequest {
    /// §4.2.
    Declare(Declare),
    /// §4.3.
    Discharge(Discharge),
}

/// Why a message to the coordinator is no request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestError {
    /// The octets are no AMQP message.
    Undecodable(String),
    /// A field of the declare or the discharge does not hold what §4.5.2
    /// or §4.5.3 gives it.
    Malformed(String),
    /// The message has no body that is one `amqp-value` section with a
    /// declare or a discharge.
    NotARequest(String),
}

impl RequestError {
    /// The error condition of Part 2 §2.8.15 for it: `amqp:decode-error`
    /// ("Data could not be decoded"), `amqp:invalid-field` ("An invalid
    /// field was passed in a frame body, and the operation could not
    /// proceed") or `amqp:not-implemented` ("The peer tried to use
    /// functionality that is not implemented in its partner").
    #[must_use]
    pub const fn condition(&self) -> &'static str {
        match self {
            Self::Undecodable(_) => "amqp:decode-error",
            Self::Malformed(_) => "amqp:invalid-field",
            Self::NotARequest(_) => "amqp:not-implemented",
        }
    }
}

impl core::fmt::Display for RequestError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Undecodable(m) | Self::Malformed(m) | Self::NotARequest(m) => f.write_str(m),
        }
    }
}

/// The message that declares a transaction: one `amqp-value` section with
/// the declare (§4.2).
///
/// # Errors
/// A `global-id` that has no encoding.
pub fn declare_message(declare: &Declare) -> Result<Vec<u8>, TypeError> {
    MessageSection::AmqpValue(declare.encode()).encode()
}

/// The message that discharges a transaction: one `amqp-value` section
/// with the discharge (§4.3).
///
/// # Errors
/// None for a value of this type; the signature is the codec's.
pub fn discharge_message(discharge: &Discharge) -> Result<Vec<u8>, TypeError> {
    MessageSection::AmqpValue(discharge.encode()).encode()
}

/// Reads a message sent to the coordinator (§4.2, §4.3): its body is "the
/// declare type in a single amqp-value section" or the discharge likewise;
/// "other standard message sections such as the header section SHOULD be
/// ignored".
///
/// # Errors
/// See [`RequestError`].
pub fn coordinator_request(message: &[u8]) -> Result<CoordinatorRequest, RequestError> {
    let mut rest = message;
    let mut body = None;
    while !rest.is_empty() {
        let (section, used) = MessageSection::decode(rest)
            .map_err(|e| RequestError::Undecodable(format!("the message does not decode: {e}")))?;
        rest = &rest[used..];
        match section {
            MessageSection::AmqpValue(value) => {
                if body.is_some() {
                    return Err(RequestError::NotARequest(
                        "more than one amqp-value section".to_string(),
                    ));
                }
                body = Some(value);
            }
            MessageSection::Data(_) | MessageSection::AmqpSequence(_) => {
                return Err(RequestError::NotARequest(
                    "the body is no amqp-value section".to_string(),
                ));
            }
            MessageSection::Header(_)
            | MessageSection::DeliveryAnnotations(_)
            | MessageSection::MessageAnnotations(_)
            | MessageSection::Properties(_)
            | MessageSection::ApplicationProperties(_)
            | MessageSection::Footer(_) => {}
        }
    }
    let Some(body) = body else {
        return Err(RequestError::NotARequest(
            "the message has no body".to_string(),
        ));
    };
    let AmqpExtValue::Described { descriptor, value } = &body else {
        return Err(RequestError::NotARequest(
            "the amqp-value is neither a declare nor a discharge".to_string(),
        ));
    };
    match *descriptor {
        desc::DECLARE => Ok(CoordinatorRequest::Declare(Declare {
            global_id: field(value, 0).cloned(),
        })),
        desc::DISCHARGE => {
            let txn_id = TxnId::decode(field(value, 0))
                .map_err(|e| RequestError::Malformed(format!("discharge: {e}")))?;
            let fail = match fields(value).get(1) {
                None | Some(AmqpExtValue::Null) => false,
                Some(AmqpExtValue::Boolean(b)) => *b,
                Some(other) => {
                    return Err(RequestError::Malformed(format!(
                        "discharge.fail {other:?} is not a boolean"
                    )));
                }
            };
            Ok(CoordinatorRequest::Discharge(Discharge { txn_id, fail }))
        }
        other => Err(RequestError::NotARequest(format!(
            "the amqp-value has descriptor {other:#x}, neither a declare nor a discharge"
        ))),
    }
}

// ============================================================================
// §4.5.5 declared, §4.5.6 transactional-state.
// ============================================================================

/// The outcome of a declare: the allocated identifier (§4.5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The identifier of the transaction.
    pub txn_id: TxnId,
}

impl Declared {
    /// The described list `0x33`.
    #[must_use]
    pub fn encode(&self) -> AmqpExtValue {
        described(desc::DECLARED, vec![self.txn_id.encode()])
    }

    /// Reads the list of a `declared`.
    ///
    /// # Errors
    /// A missing or malformed `txn-id`.
    pub fn decode_list(list: &AmqpExtValue) -> Result<Self, String> {
        Self::decode_list_up_to(list, TxnId::MAX_LEN)
    }

    /// [`Declared::decode_list`] with a txn-id of at most `max` octets
    /// ([`TxnId::decode_up_to`]).
    ///
    /// # Errors
    /// A missing or malformed `txn-id`.
    pub fn decode_list_up_to(list: &AmqpExtValue, max: usize) -> Result<Self, String> {
        Ok(Self {
            txn_id: TxnId::decode_up_to(field(list, 0), max)
                .map_err(|e| format!("declared: {e}"))?,
        })
    }
}

/// The state of a transactional delivery (§4.5.6): the transaction, and
/// the outcome that applies if it commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionalState {
    /// The transaction.
    pub txn_id: TxnId,
    /// The provisional outcome; never [`Outcome::None`].
    pub outcome: Option<Outcome>,
}

impl TransactionalState {
    /// A state that names the transaction and no outcome: the state of a
    /// transfer posted to it (§4.4.1) or acquired in it (§4.4.3).
    #[must_use]
    pub fn of(txn_id: &TxnId) -> Self {
        Self {
            txn_id: txn_id.clone(),
            outcome: None,
        }
    }

    /// The described list `0x34`.
    #[must_use]
    pub fn encode(&self) -> AmqpExtValue {
        described(
            desc::TRANSACTIONAL_STATE,
            trimmed(vec![
                self.txn_id.encode(),
                self.outcome
                    .as_ref()
                    .map_or(AmqpExtValue::Null, encode_outcome),
            ]),
        )
    }

    /// Reads the list of a `transactional-state`.
    ///
    /// # Errors
    /// A missing or malformed `txn-id`; an `outcome` that is none of the
    /// outcomes of Part 3 §3.4.
    pub fn decode_list(list: &AmqpExtValue) -> Result<Self, String> {
        Self::decode_list_up_to(list, TxnId::MAX_LEN)
    }

    /// [`TransactionalState::decode_list`] with a txn-id of at most `max`
    /// octets ([`TxnId::decode_up_to`]).
    ///
    /// # Errors
    /// As [`TransactionalState::decode_list`].
    pub fn decode_list_up_to(list: &AmqpExtValue, max: usize) -> Result<Self, String> {
        let txn_id = TxnId::decode_up_to(field(list, 0), max)
            .map_err(|e| format!("transactional-state: {e}"))?;
        let outcome = match field(list, 1) {
            None => None,
            Some(value) => match decode_outcome(Some(value)) {
                Outcome::None => {
                    return Err(format!(
                        "transactional-state.outcome {value:?} is no outcome"
                    ));
                }
                outcome => Some(outcome),
            },
        };
        Ok(Self { txn_id, outcome })
    }
}

// ============================================================================
// §4.5.8 transaction-error.
// ============================================================================

/// The error conditions of a transaction (§4.5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransactionError {
    /// `amqp:transaction:unknown-id`: "The specified txn-id does not
    /// exist."
    UnknownId,
    /// `amqp:transaction:rollback`: "The transaction was rolled back for
    /// an unspecified reason."
    Rollback,
    /// `amqp:transaction:timeout`: "The work represented by this
    /// transaction took too long."
    Timeout,
}

impl TransactionError {
    /// The three conditions.
    pub const ALL: [Self; 3] = [Self::UnknownId, Self::Rollback, Self::Timeout];

    /// The condition symbol.
    #[must_use]
    pub const fn condition(self) -> &'static str {
        match self {
            Self::UnknownId => "amqp:transaction:unknown-id",
            Self::Rollback => "amqp:transaction:rollback",
            Self::Timeout => "amqp:transaction:timeout",
        }
    }

    /// The transaction error a condition symbol names.
    #[must_use]
    pub fn from_condition(condition: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.condition() == condition)
    }

    /// The `amqp:error` with this condition.
    #[must_use]
    pub fn error(self, description: impl Into<String>) -> AmqpError {
        AmqpError::new(self.condition(), description)
    }
}

/// The `txn-id` entry of `flow.properties` (§4.4.3): `Ok(None)` for a map
/// without the entry.
///
/// # Errors
/// An entry that is no txn-id.
pub fn flow_txn_id(properties: &AmqpExtValue) -> Result<Option<TxnId>, String> {
    let AmqpExtValue::Map(pairs) = properties else {
        return Err(format!("flow.properties {properties:?} is not a map"));
    };
    for (key, value) in pairs {
        if matches!(key, AmqpExtValue::Symbol(k) | AmqpExtValue::Str(k) if k == FLOW_TXN_ID) {
            return TxnId::decode(Some(value))
                .map(Some)
                .map_err(|e| format!("flow.properties: {e}"));
        }
    }
    Ok(None)
}

/// `flow.properties` with the `txn-id` entry (§4.4.3).
#[must_use]
pub fn flow_properties(txn_id: &TxnId) -> AmqpExtValue {
    AmqpExtValue::Map(vec![(
        AmqpExtValue::Symbol(FLOW_TXN_ID.to_string()),
        txn_id.encode(),
    )])
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::terminus::{DeliveryState, decode_state, encode_state};

    fn wire(v: &AmqpExtValue) -> Vec<u8> {
        v.encode().expect("encodes")
    }

    fn txn(octets: &[u8]) -> TxnId {
        TxnId::new(octets).expect("a txn-id")
    }

    /// §4.5.1: the coordinator is the described list 0x30 with the
    /// capabilities as one symbol or an array of symbols; the symbolic
    /// descriptor names the same type; a symbol outside §4.5.7 is refused.
    #[test]
    fn amqp10_4_5_1_a_coordinator_is_0x30_and_its_capabilities() {
        let none = Coordinator::default();
        // 0x00, smallulong 0x30, list0.
        assert_eq!(wire(&none.encode()), [0x00, 0x53, 0x30, 0x45]);
        let two = Coordinator {
            capabilities: vec![
                TxnCapability::LocalTransactions,
                TxnCapability::MultiTxnsPerSsn,
            ],
        };
        let encoded = two.encode();
        let AmqpExtValue::Described { descriptor, value } = &encoded else {
            panic!("described");
        };
        assert_eq!(*descriptor, 0x30);
        assert_eq!(
            fields(value),
            [AmqpExtValue::Array(vec![
                AmqpExtValue::Symbol("amqp:local-transactions".into()),
                AmqpExtValue::Symbol("amqp:multi-txns-per-ssn".into()),
            ])]
        );
        assert_eq!(Coordinator::decode(Some(&encoded)), Ok(Some(two.clone())));
        assert!(two.has(TxnCapability::MultiTxnsPerSsn));
        assert!(!two.has(TxnCapability::DistributedTransactions));
        // What the controller verifies: the desired capabilities the
        // coordinator lacks, each once.
        assert_eq!(
            two.lacking(&[
                TxnCapability::LocalTransactions,
                TxnCapability::DistributedTransactions,
                TxnCapability::MultiSsnsPerTxn,
                TxnCapability::DistributedTransactions,
            ]),
            [
                TxnCapability::DistributedTransactions,
                TxnCapability::MultiSsnsPerTxn
            ]
        );
        assert!(two.lacking(&[TxnCapability::MultiTxnsPerSsn]).is_empty());
        // `multiple="true"`: one symbol stands for an array of one.
        let single = described(
            desc::COORDINATOR,
            vec![AmqpExtValue::Symbol("amqp:local-transactions".into())],
        );
        assert_eq!(
            Coordinator::decode(Some(&single)),
            Ok(Some(Coordinator {
                capabilities: vec![TxnCapability::LocalTransactions]
            }))
        );
        // The symbolic descriptor (Part 1 §1.3.3): 0x00, sym8
        // "amqp:coordinator:list", list0.
        let mut symbolic = vec![0x00, 0xa3, 21];
        symbolic.extend_from_slice(b"amqp:coordinator:list");
        symbolic.push(0x45);
        let (decoded, used) = AmqpExtValue::decode(&symbolic).expect("decodes");
        assert_eq!(used, symbolic.len());
        assert_eq!(Coordinator::decode(Some(&decoded)), Ok(Some(none)));
        // No coordinator: a target, null.
        assert_eq!(
            Coordinator::decode(Some(&described(0x29, vec![]))),
            Ok(None)
        );
        assert_eq!(Coordinator::decode(None), Ok(None));
        // A capability that is none.
        let unknown = described(
            desc::COORDINATOR,
            vec![AmqpExtValue::Symbol("amqp:nested-transactions".into())],
        );
        let refused = Coordinator::decode(Some(&unknown)).expect_err("refused");
        assert!(refused.contains("amqp:nested-transactions"), "{refused}");
        let not_symbol = described(desc::COORDINATOR, vec![AmqpExtValue::Uint(1)]);
        assert!(Coordinator::decode(Some(&not_symbol)).is_err());
    }

    /// §4.5.2: declare is the described list 0x31 with an optional
    /// global-id, sent as one amqp-value section; sections around the body
    /// are ignored.
    #[test]
    fn amqp10_4_5_2_declare_is_0x31_with_an_optional_global_id() {
        let local = Declare::default();
        let message = declare_message(&local).expect("encodes");
        // amqp-value 0x77 around 0x00 0x53 0x31 list0.
        assert_eq!(message, [0x00, 0x53, 0x77, 0x00, 0x53, 0x31, 0x45]);
        assert_eq!(
            coordinator_request(&message),
            Ok(CoordinatorRequest::Declare(local))
        );
        let global = Declare {
            global_id: Some(AmqpExtValue::Binary(vec![1, 2, 3])),
        };
        let message = declare_message(&global).expect("encodes");
        assert_eq!(
            coordinator_request(&message),
            Ok(CoordinatorRequest::Declare(global))
        );
        // §4.2: "Other standard message sections such as the header
        // section SHOULD be ignored."
        let mut with_header =
            MessageSection::Header(AmqpExtValue::List(vec![AmqpExtValue::Boolean(true)]))
                .encode()
                .unwrap();
        with_header.extend(
            MessageSection::Properties(AmqpExtValue::List(vec![AmqpExtValue::Str("id".into())]))
                .encode()
                .unwrap(),
        );
        with_header.extend(declare_message(&Declare::default()).unwrap());
        with_header.extend(
            MessageSection::Footer(AmqpExtValue::Map(Vec::new()))
                .encode()
                .unwrap(),
        );
        assert_eq!(
            coordinator_request(&with_header),
            Ok(CoordinatorRequest::Declare(Declare::default()))
        );
    }

    /// §4.5.3: discharge is the described list 0x32 with the mandatory
    /// txn-id and the flag fail, false when not set.
    #[test]
    fn amqp10_4_5_3_discharge_is_0x32_txn_id_and_fail() {
        let commit = Discharge {
            txn_id: txn(&[0, 7]),
            fail: false,
        };
        let message = discharge_message(&commit).expect("encodes");
        assert_eq!(
            message,
            [
                0x00, 0x53, 0x77, 0x00, 0x53, 0x32, 0xc0, 0x06, 0x02, 0xa0, 0x02, 0x00, 0x07, 0x42
            ]
        );
        assert_eq!(
            coordinator_request(&message),
            Ok(CoordinatorRequest::Discharge(commit))
        );
        let rollback = Discharge {
            txn_id: txn(b"t"),
            fail: true,
        };
        assert_eq!(
            coordinator_request(&discharge_message(&rollback).unwrap()),
            Ok(CoordinatorRequest::Discharge(rollback))
        );
        // fail not set: false.
        let without_fail = MessageSection::AmqpValue(described(
            desc::DISCHARGE,
            vec![AmqpExtValue::Binary(vec![9])],
        ))
        .encode()
        .unwrap();
        assert_eq!(
            coordinator_request(&without_fail),
            Ok(CoordinatorRequest::Discharge(Discharge {
                txn_id: txn(&[9]),
                fail: false
            }))
        );
        // txn-id is mandatory.
        let without_id = MessageSection::AmqpValue(described(desc::DISCHARGE, vec![]))
            .encode()
            .unwrap();
        assert!(matches!(
            coordinator_request(&without_id),
            Err(RequestError::Malformed(_))
        ));
        let fail_not_boolean = MessageSection::AmqpValue(described(
            desc::DISCHARGE,
            vec![AmqpExtValue::Binary(vec![9]), AmqpExtValue::Uint(1)],
        ))
        .encode()
        .unwrap();
        assert!(matches!(
            coordinator_request(&fail_not_boolean),
            Err(RequestError::Malformed(_))
        ));
    }

    /// §4.2, §4.3: a message whose body is no single amqp-value section
    /// with a declare or a discharge is no request, and octets that are no
    /// message are undecodable; each has its error condition.
    #[test]
    fn amqp10_4_2_a_message_without_a_declare_or_discharge_is_no_request() {
        let data = MessageSection::Data(b"x".to_vec()).encode().unwrap();
        assert!(matches!(
            coordinator_request(&data),
            Err(RequestError::NotARequest(_))
        ));
        let text = MessageSection::AmqpValue(AmqpExtValue::Str("declare".into()))
            .encode()
            .unwrap();
        assert!(matches!(
            coordinator_request(&text),
            Err(RequestError::NotARequest(_))
        ));
        let accepted = MessageSection::AmqpValue(described(0x24, vec![]))
            .encode()
            .unwrap();
        assert!(matches!(
            coordinator_request(&accepted),
            Err(RequestError::NotARequest(_))
        ));
        let mut twice = declare_message(&Declare::default()).unwrap();
        twice.extend(declare_message(&Declare::default()).unwrap());
        assert!(matches!(
            coordinator_request(&twice),
            Err(RequestError::NotARequest(_))
        ));
        assert!(matches!(
            coordinator_request(&[]),
            Err(RequestError::NotARequest(_))
        ));
        let undecodable = coordinator_request(&[0x00, 0x53]).expect_err("no message");
        assert!(matches!(undecodable, RequestError::Undecodable(_)));
        assert_eq!(undecodable.condition(), "amqp:decode-error");
        assert_eq!(
            RequestError::Malformed(String::new()).condition(),
            "amqp:invalid-field"
        );
        assert_eq!(
            RequestError::NotARequest(String::new()).condition(),
            "amqp:not-implemented"
        );
    }

    /// §4.5.4: a controller told so reads longer txn-ids (Artemis sends
    /// 36 octets) in a `declared` and a `transactional-state`; the default
    /// readers stay at 32.
    #[test]
    fn amqp10_4_5_4_a_longer_txn_id_is_read_only_up_to_the_given_bound() {
        let long = AmqpExtValue::Binary(vec![b'u'; 36]);
        assert!(TxnId::decode(Some(&long)).is_err());
        let id = TxnId::decode_up_to(Some(&long), 36).expect("within 36");
        assert_eq!(id.as_bytes(), [b'u'; 36]);
        assert_eq!(id.encode(), long, "named with the same octets");
        assert_eq!(
            TxnId::decode_up_to(Some(&AmqpExtValue::Binary(vec![0; 37])), 36),
            Err("a txn-id of 37 octets, more than 36".to_string())
        );
        assert!(TxnId::decode_up_to(Some(&AmqpExtValue::Str("t".into())), 36).is_err());
        let list = AmqpExtValue::List(vec![long.clone()]);
        assert!(Declared::decode_list(&list).is_err());
        assert_eq!(
            Declared::decode_list_up_to(&list, 36).map(|d| d.txn_id),
            Ok(id.clone())
        );
        assert!(TransactionalState::decode_list(&list).is_err());
        assert_eq!(
            TransactionalState::decode_list_up_to(&list, 36),
            Ok(TransactionalState::of(&id))
        );
    }

    /// §4.5.4: a transaction-id is a binary of up to 32 octets.
    #[test]
    fn amqp10_4_5_4_a_txn_id_is_a_binary_of_at_most_32_octets() {
        assert_eq!(txn(&[]).as_bytes(), b"");
        assert_eq!(txn(&[7; 32]).as_bytes(), [7; 32]);
        assert_eq!(TxnId::new(&[7; 33]), None);
        assert_eq!(
            txn(&[0xab, 0x01]).encode(),
            AmqpExtValue::Binary(vec![0xab, 1])
        );
        assert_eq!(txn(&[0xab, 0x01]).to_string(), "ab01");
        assert_eq!(
            TxnId::decode(Some(&AmqpExtValue::Binary(vec![1; 32]))),
            Ok(txn(&[1; 32]))
        );
        assert!(TxnId::decode(Some(&AmqpExtValue::Binary(vec![1; 33]))).is_err());
        assert!(TxnId::decode(Some(&AmqpExtValue::Str("t".into()))).is_err());
        assert!(TxnId::decode(Some(&AmqpExtValue::Uint(0))).is_err());
        assert!(TxnId::decode(None).is_err());
    }

    /// §4.5.5: declared is the described list 0x33 with the mandatory
    /// txn-id, and a delivery state.
    #[test]
    fn amqp10_4_5_5_declared_is_0x33_and_the_txn_id() {
        let declared = Declared { txn_id: txn(&[5]) };
        assert_eq!(
            wire(&declared.encode()),
            [0x00, 0x53, 0x33, 0xc0, 0x04, 0x01, 0xa0, 0x01, 0x05]
        );
        let state = DeliveryState::Declared(txn(&[5]));
        assert_eq!(encode_state(&state), declared.encode());
        assert_eq!(decode_state(Some(&declared.encode())), Ok(Some(state)));
        // Without the txn-id, or with one that is none.
        assert!(decode_state(Some(&described(desc::DECLARED, vec![]))).is_err());
        assert!(
            decode_state(Some(&described(
                desc::DECLARED,
                vec![AmqpExtValue::Binary(vec![0; 40])]
            )))
            .is_err()
        );
    }

    /// §4.5.6: a transactional-state is the described list 0x34 with the
    /// mandatory txn-id and an optional outcome of Part 3 §3.4.
    #[test]
    fn amqp10_4_5_6_a_transactional_state_is_0x34_txn_id_and_outcome() {
        let posted = TransactionalState::of(&txn(&[1]));
        assert_eq!(
            wire(&posted.encode()),
            [0x00, 0x53, 0x34, 0xc0, 0x04, 0x01, 0xa0, 0x01, 0x01]
        );
        assert_eq!(
            decode_state(Some(&posted.encode())),
            Ok(Some(DeliveryState::Transactional(posted)))
        );
        for outcome in [
            Outcome::Accepted,
            Outcome::Released,
            Outcome::Rejected(Some(AmqpError::new("amqp:not-found", "no node"))),
            Outcome::Modified {
                delivery_failed: true,
                undeliverable_here: false,
            },
        ] {
            let state = TransactionalState {
                txn_id: txn(b"abc"),
                outcome: Some(outcome.clone()),
            };
            let encoded = state.encode();
            let AmqpExtValue::Described { descriptor, value } = &encoded else {
                panic!("described");
            };
            assert_eq!(*descriptor, 0x34);
            assert_eq!(fields(value).len(), 2);
            assert_eq!(fields(value)[1], encode_outcome(&outcome));
            assert_eq!(
                decode_state(Some(&encoded)),
                Ok(Some(DeliveryState::Transactional(state)))
            );
        }
        // txn-id is mandatory; the outcome is one of §3.4.
        assert!(decode_state(Some(&described(desc::TRANSACTIONAL_STATE, vec![]))).is_err());
        let received_inside = described(
            desc::TRANSACTIONAL_STATE,
            vec![
                AmqpExtValue::Binary(vec![1]),
                described(0x23, vec![AmqpExtValue::Uint(0), AmqpExtValue::Ulong(0)]),
            ],
        );
        assert!(decode_state(Some(&received_inside)).is_err());
    }

    /// §4.5.7: the five capabilities are their symbols.
    #[test]
    fn amqp10_4_5_7_the_txn_capabilities_are_their_five_symbols() {
        let symbols: Vec<&str> = TxnCapability::ALL.iter().map(|c| c.symbol()).collect();
        assert_eq!(
            symbols,
            [
                "amqp:local-transactions",
                "amqp:distributed-transactions",
                "amqp:promotable-transactions",
                "amqp:multi-txns-per-ssn",
                "amqp:multi-ssns-per-txn",
            ]
        );
        for capability in TxnCapability::ALL {
            assert_eq!(
                TxnCapability::from_symbol(capability.symbol()),
                Some(capability)
            );
        }
        assert_eq!(TxnCapability::from_symbol("amqp:local-transaction"), None);
    }

    /// §4.5.8: the three transaction errors are their symbols and are
    /// error conditions.
    #[test]
    fn amqp10_4_5_8_the_transaction_errors_are_their_three_symbols() {
        let symbols: Vec<&str> = TransactionError::ALL
            .iter()
            .map(|e| e.condition())
            .collect();
        assert_eq!(
            symbols,
            [
                "amqp:transaction:unknown-id",
                "amqp:transaction:rollback",
                "amqp:transaction:timeout",
            ]
        );
        for error in TransactionError::ALL {
            assert_eq!(
                TransactionError::from_condition(error.condition()),
                Some(error)
            );
        }
        assert_eq!(TransactionError::from_condition("amqp:not-found"), None);
        let error = TransactionError::UnknownId.error("no transaction 07");
        assert_eq!(error.condition, "amqp:transaction:unknown-id");
        assert_eq!(error.description.as_deref(), Some("no transaction 07"));
    }

    /// §4.4.3: the transaction of a flow is the entry `txn-id` of its
    /// properties map.
    #[test]
    fn amqp10_4_4_3_the_txn_id_of_a_flow_is_an_entry_of_its_properties() {
        let id = txn(&[3, 4]);
        let properties = flow_properties(&id);
        assert_eq!(
            properties,
            AmqpExtValue::Map(vec![(
                AmqpExtValue::Symbol("txn-id".into()),
                AmqpExtValue::Binary(vec![3, 4])
            )])
        );
        assert_eq!(flow_txn_id(&properties), Ok(Some(id)));
        let other = AmqpExtValue::Map(vec![(
            AmqpExtValue::Symbol("x".into()),
            AmqpExtValue::Uint(1),
        )]);
        assert_eq!(flow_txn_id(&other), Ok(None));
        assert_eq!(flow_txn_id(&AmqpExtValue::Map(Vec::new())), Ok(None));
        let not_binary = AmqpExtValue::Map(vec![(
            AmqpExtValue::Symbol("txn-id".into()),
            AmqpExtValue::Uint(1),
        )]);
        assert!(flow_txn_id(&not_binary).is_err());
        assert!(flow_txn_id(&AmqpExtValue::Uint(1)).is_err());
    }
}
