// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The transaction controller of OASIS AMQP 1.0 Part 4 on the client
//! connection of [`crate::broker`].
//!
//! * [`TxnController`] is the control link (§4.2, §4.3) as a state
//!   machine over the events of a [`BrokerReader`]: it attaches to the
//!   coordinator, sends `declare` and `discharge` unsettled, and reads
//!   their outcome: `declared` with the txn-id, `accepted`, or the
//!   transaction error, carried by `rejected` or by the detach of the
//!   link.
//! * [`TxnClient`] is one connection with one session and one control
//!   link, driven by the calling thread: declare, commit, roll back, send
//!   in a transaction (§4.4.1), retire in one (§4.4.2), acquire in one
//!   (§4.4.3).
//!
//! The transactional work itself needs no more than the client connection:
//! [`BrokerWriter::transfer_with_state`],
//! [`BrokerWriter::disposition_with_state`] and
//! [`BrokerWriter::flow_in_txn`] with the states [`posted`] and
//! [`retired`].

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::time::{Duration, Instant};

use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::broker::{
    BrokerReader, BrokerWriter, ConnectParams, INITIAL_DELIVERY_COUNT, Inbound, LinkSpec,
    RemoteOpen, connect, sender_credit,
};
use crate::terminus::{AmqpError, DeliveryState, Outcome};
use crate::transactions::{
    Coordinator, Declare, Discharge, TransactionError, TransactionalState, TxnCapability, TxnId,
    declare_message, discharge_message,
};

/// The state of a transfer posted to `txn` (§4.4.1) and of a message
/// acquired in it (§4.4.3): a `transactional-state` without an outcome.
#[must_use]
pub fn posted(txn: &TxnId) -> DeliveryState {
    DeliveryState::Transactional(TransactionalState::of(txn))
}

/// The state of a disposition that associates `outcome` with `txn`
/// (§4.4.2).
#[must_use]
pub fn retired(txn: &TxnId, outcome: Outcome) -> DeliveryState {
    DeliveryState::Transactional(TransactionalState {
        txn_id: txn.clone(),
        outcome: Some(outcome),
    })
}

/// Why a declare or a discharge did not succeed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxnError {
    /// `amqp:transaction:unknown-id` (§4.5.8): "The specified txn-id does
    /// not exist."
    UnknownId(String),
    /// `amqp:transaction:rollback`: "The transaction was rolled back for
    /// an unspecified reason."
    Rollback(String),
    /// `amqp:transaction:timeout`: "The work represented by this
    /// transaction took too long."
    Timeout(String),
    /// The coordinator refused with another error condition.
    Refused(AmqpError),
    /// The coordinator answered with a state that is no answer to the
    /// request.
    Protocol(String),
    /// The control link is gone, with the error of its detach if any;
    /// every transaction declared over it is rolled back (§4.1).
    Closed(Option<AmqpError>),
    /// The request could not be sent or its answer not read: an I/O
    /// error, or no answer within the time-out. Whether the coordinator
    /// performed it is not known.
    Io(String),
    /// The request needs a capability the coordinator did not state, and
    /// was not sent (§4.5.1: "it is the responsibility of the transaction
    /// controller to verify that the capabilities of the controller meet
    /// its requirements"); for example a second declare on the session
    /// while a transaction is open, without `amqp:multi-txns-per-ssn`
    /// (§4.5.7).
    Unsupported(String),
}

impl TxnError {
    /// The error an `amqp:error` of the coordinator stands for.
    #[must_use]
    pub fn from_error(error: &AmqpError) -> Self {
        let text = error.description.clone().unwrap_or_default();
        match TransactionError::from_condition(&error.condition) {
            Some(TransactionError::UnknownId) => Self::UnknownId(text),
            Some(TransactionError::Rollback) => Self::Rollback(text),
            Some(TransactionError::Timeout) => Self::Timeout(text),
            None => Self::Refused(error.clone()),
        }
    }
}

impl core::fmt::Display for TxnError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownId(t) => write!(f, "amqp:transaction:unknown-id: {t}"),
            Self::Rollback(t) => write!(f, "amqp:transaction:rollback: {t}"),
            Self::Timeout(t) => write!(f, "amqp:transaction:timeout: {t}"),
            Self::Refused(e) => write!(f, "the coordinator refused: {e}"),
            Self::Protocol(t) => write!(f, "the coordinator's answer: {t}"),
            Self::Closed(Some(e)) => write!(f, "the control link was detached: {e}"),
            Self::Closed(None) => f.write_str("the control link is gone"),
            Self::Io(t) => write!(f, "the control link: {t}"),
            Self::Unsupported(t) => write!(f, "the coordinator lacks a capability: {t}"),
        }
    }
}

impl std::error::Error for TxnError {}

impl From<TxnError> for io::Error {
    fn from(e: TxnError) -> Self {
        Self::other(e)
    }
}

/// What the control link reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxnEvent {
    /// The coordinator attached, with the capabilities it has (§4.5.1).
    Attached(Coordinator),
    /// A declare was answered: the txn-id, or why not.
    Declared {
        /// The delivery of the declare.
        delivery_id: u32,
        /// The identifier of the transaction.
        result: Result<TxnId, TxnError>,
    },
    /// A discharge was answered: `Ok` when the coordinator completed it
    /// as asked.
    Discharged {
        /// The delivery of the discharge.
        delivery_id: u32,
        /// The transaction.
        txn: TxnId,
        /// The discharge asked for the rollback.
        fail: bool,
        /// Completed, or why not. After an error the transaction is
        /// rolled back (§4.4.2: "a resource-initiated rollback (the
        /// discharge message being rejected, or the link to the
        /// coordinator being detached with an error)").
        result: Result<(), TxnError>,
    },
    /// The control link is gone; every transaction declared over it is
    /// rolled back (§4.1).
    Closed(Option<AmqpError>),
}

enum Request {
    Declare,
    Discharge { txn: TxnId, fail: bool },
}

/// The control link of a transaction controller (§4.1 to §4.3).
pub struct TxnController {
    channel: u16,
    handle: u32,
    name: String,
    remote_handle: Option<u32>,
    coordinator: Option<Coordinator>,
    /// Deliveries the coordinator's credit still admits.
    credit: u32,
    /// Deliveries sent on the link.
    delivery_count: u32,
    pending: BTreeMap<u32, Request>,
    /// The transactions declared over the link and not yet discharged.
    open: Vec<TxnId>,
    next_tag: u64,
    /// The link is gone, with the error it went with.
    closed: Option<Option<AmqpError>>,
}

impl TxnController {
    /// A control link named `name` with the client's `handle` on session
    /// `channel`; [`TxnController::attach`] establishes it.
    #[must_use]
    pub fn new(channel: u16, handle: u32, name: &str) -> Self {
        Self {
            channel,
            handle,
            name: name.to_string(),
            remote_handle: None,
            coordinator: None,
            credit: 0,
            delivery_count: 0,
            pending: BTreeMap::new(),
            open: Vec::new(),
            next_tag: 0,
            closed: None,
        }
    }

    /// The attach of the control link (§4.2): the target is a coordinator
    /// with the `desired` capabilities, the source supports `rejected`,
    /// and the link sends unsettled (a declare "MUST NOT be sent
    /// settled").
    #[must_use]
    pub fn link_spec(&self, desired: &[TxnCapability]) -> LinkSpec {
        LinkSpec {
            name: self.name.clone(),
            handle: self.handle,
            sender: true,
            address: String::new(),
            snd_settle_mode: 0,
            rcv_settle_mode: 0,
            durable: 0,
            properties: Vec::new(),
            desired_capabilities: Vec::new(),
            coordinator: Some(desired.to_vec()),
        }
    }

    /// Attaches the control link.
    ///
    /// # Errors
    /// I/O errors.
    pub fn attach(&self, writer: &mut BrokerWriter, desired: &[TxnCapability]) -> io::Result<()> {
        writer.attach(self.channel, &self.link_spec(desired))
    }

    /// The coordinator as it attached; `None` before that. §4.5.1: "it is
    /// the responsibility of the transaction controller to verify that
    /// the capabilities of the controller meet its requirements".
    #[must_use]
    pub fn coordinator(&self) -> Option<&Coordinator> {
        self.coordinator.as_ref()
    }

    /// Requests the coordinator's credit admits now.
    #[must_use]
    pub fn credit(&self) -> u32 {
        self.credit
    }

    /// `Some` once the control link is gone, with the error it went
    /// with if any.
    #[must_use]
    pub fn closed(&self) -> Option<&Option<AmqpError>> {
        self.closed.as_ref()
    }

    /// The transactions declared over the link that are not discharged:
    /// declared, and neither discharged nor rolled back with the end of
    /// the link (§4.1).
    #[must_use]
    pub fn open_transactions(&self) -> &[TxnId] {
        &self.open
    }

    /// Whether the coordinator admits one more declare on this session:
    /// with a transaction open or a declare unanswered, only when it
    /// stated `amqp:multi-txns-per-ssn` (§4.5.7: "Support multiple active
    /// transactions on a single session"; §4.5.1: the controller verifies
    /// the capabilities).
    ///
    /// # Errors
    /// Why the declare would need a capability the coordinator lacks.
    pub fn admits_declare(&self) -> Result<(), String> {
        let multi = self
            .coordinator
            .as_ref()
            .is_some_and(|c| c.has(TxnCapability::MultiTxnsPerSsn));
        let declaring = self.pending.values().any(|r| matches!(r, Request::Declare));
        if !multi && (declaring || !self.open.is_empty()) {
            return Err(format!(
                "a second transaction on the session ({} open, a declare {}), and the \
                 coordinator did not state amqp:multi-txns-per-ssn",
                self.open.len(),
                if declaring {
                    "unanswered"
                } else {
                    "not pending"
                }
            ));
        }
        Ok(())
    }

    /// `NotConnected` before the coordinator attached, `InvalidInput` when
    /// it lacks `amqp:multi-txns-per-ssn` for one more transaction.
    fn check_declare(&self) -> io::Result<()> {
        if self.coordinator.is_none() || self.closed.is_some() {
            // The request says why.
            return Ok(());
        }
        self.admits_declare()
            .map_err(|reason| io::Error::new(io::ErrorKind::InvalidInput, reason))
    }

    /// Keeps [`TxnController::open_transactions`] with what the link
    /// reports.
    fn note(&mut self, event: &TxnEvent) {
        match event {
            TxnEvent::Declared {
                result: Ok(txn), ..
            } => {
                if !self.open.contains(txn) {
                    self.open.push(txn.clone());
                }
            }
            // Committed, rolled back, or refused and rolled back by the
            // resource (§4.4.2): no longer open.
            TxnEvent::Discharged { txn, .. } => self.open.retain(|t| t != txn),
            // §4.1: every transaction of the link is rolled back.
            TxnEvent::Closed(_) => self.open.clear(),
            TxnEvent::Declared { result: Err(_), .. } | TxnEvent::Attached(_) => {}
        }
    }

    fn request(
        &mut self,
        writer: &mut BrokerWriter,
        message: &[u8],
        request: Request,
    ) -> io::Result<u32> {
        if self.closed.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "the control link is gone",
            ));
        }
        if self.coordinator.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "the coordinator has not attached",
            ));
        }
        if self.credit == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "the coordinator granted no credit",
            ));
        }
        self.next_tag += 1;
        let tag = self.next_tag.to_be_bytes();
        // §4.2: "This message MUST NOT be sent settled".
        let delivery_id = writer.transfer(self.channel, self.handle, &tag, message, false)?;
        self.credit -= 1;
        self.delivery_count = self.delivery_count.wrapping_add(1);
        self.pending.insert(delivery_id, request);
        Ok(delivery_id)
    }

    /// Sends a declare for a local transaction (§4.2); the answer is
    /// [`TxnEvent::Declared`] for the returned delivery.
    ///
    /// # Errors
    /// `NotConnected` before the coordinator attached or after the link
    /// went; `InvalidInput` for a second transaction on the session when
    /// the coordinator did not state `amqp:multi-txns-per-ssn`
    /// ([`TxnController::admits_declare`]); `WouldBlock` without credit,
    /// or when the session window is full; I/O errors.
    pub fn declare(&mut self, writer: &mut BrokerWriter) -> io::Result<u32> {
        self.check_declare()?;
        let message = declare_message(&Declare::default())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))?;
        self.request(writer, &message, Request::Declare)
    }

    /// Sends a declare that associates the transaction with the global
    /// transaction `global_id` (§4.5.2).
    ///
    /// # Errors
    /// `InvalidInput` when the coordinator did not state
    /// `amqp:distributed-transactions` ("This field MUST NOT be set if the
    /// coordinator does not have the distributed-transactions
    /// capability"); else as [`TxnController::declare`].
    pub fn declare_global(
        &mut self,
        writer: &mut BrokerWriter,
        global_id: AmqpExtValue,
    ) -> io::Result<u32> {
        if !self
            .coordinator
            .as_ref()
            .is_some_and(|c| c.has(TxnCapability::DistributedTransactions))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the coordinator did not state amqp:distributed-transactions",
            ));
        }
        self.check_declare()?;
        let message = declare_message(&Declare {
            global_id: Some(global_id),
        })
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))?;
        self.request(writer, &message, Request::Declare)
    }

    /// Sends the discharge of `txn` (§4.3): a commit, or with `fail` a
    /// rollback; the answer is [`TxnEvent::Discharged`].
    ///
    /// # Errors
    /// As [`TxnController::declare`].
    pub fn discharge(
        &mut self,
        writer: &mut BrokerWriter,
        txn: &TxnId,
        fail: bool,
    ) -> io::Result<u32> {
        let message = discharge_message(&Discharge {
            txn_id: txn.clone(),
            fail,
        })
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))?;
        self.request(
            writer,
            &message,
            Request::Discharge {
                txn: txn.clone(),
                fail,
            },
        )
    }

    fn answered(
        delivery_id: u32,
        request: Request,
        result: Result<Option<TxnId>, TxnError>,
    ) -> TxnEvent {
        match request {
            Request::Declare => TxnEvent::Declared {
                delivery_id,
                result: result.and_then(|txn| {
                    txn.ok_or_else(|| {
                        TxnError::Protocol("a declare answered without declared".to_string())
                    })
                }),
            },
            Request::Discharge { txn, fail } => TxnEvent::Discharged {
                delivery_id,
                txn,
                fail,
                result: result.map(|_| ()),
            },
        }
    }

    /// The link is gone: every request that waits fails with `error`.
    fn close(&mut self, error: Option<&AmqpError>, events: &mut Vec<TxnEvent>) {
        if self.closed.is_some() {
            return;
        }
        self.closed = Some(error.cloned());
        self.credit = 0;
        // §4.2, §4.3: without the outcome rejected "the transactional
        // resource MUST detach the link to the coordinator, with the
        // detach performative carrying the transaction-error".
        let failure = match error {
            Some(e) => TxnError::from_error(e),
            None => TxnError::Closed(None),
        };
        for (delivery_id, request) in core::mem::take(&mut self.pending) {
            events.push(Self::answered(delivery_id, request, Err(failure.clone())));
        }
        events.push(TxnEvent::Closed(error.cloned()));
    }

    /// Takes an event of the connection; returns what it means for the
    /// control link. Events of other links and sessions yield nothing.
    ///
    /// # Errors
    /// I/O errors of what the controller sends in answer: the settlement
    /// of a request the coordinator answered without settling, the detach
    /// that answers the coordinator's.
    pub fn on_event(
        &mut self,
        writer: &mut BrokerWriter,
        event: &Inbound,
    ) -> io::Result<Vec<TxnEvent>> {
        let mut events = Vec::new();
        match event {
            Inbound::Attach {
                channel,
                name,
                remote_handle,
                has_node,
                coordinator,
                ..
            } if *channel == self.channel && *name == self.name => {
                self.remote_handle = Some(*remote_handle);
                match coordinator {
                    Some(c) => {
                        self.coordinator = Some(c.clone());
                        events.push(TxnEvent::Attached(c.clone()));
                    }
                    // A refused attach: the detach with the reason
                    // follows.
                    None if !has_node => {}
                    None => {
                        // The partner attached a node that is no
                        // coordinator: no transactions here.
                        let error = AmqpError::new(
                            "amqp:not-implemented",
                            "the partner attached no coordinator",
                        );
                        writer.detach(self.channel, self.handle, Some(&error))?;
                        self.close(Some(&error), &mut events);
                    }
                }
            }
            Inbound::Flow {
                channel,
                remote_handle: Some(remote),
                delivery_count,
                link_credit,
                echo,
                drain,
                ..
            } if *channel == self.channel && Some(*remote) == self.remote_handle => {
                // Part 2 §2.6.7: link-credit_snd := delivery-count_rcv +
                // link-credit_rcv - delivery-count_snd.
                if let Some(link_credit) = link_credit {
                    self.credit = sender_credit(
                        *delivery_count,
                        *link_credit,
                        self.delivery_count,
                        INITIAL_DELIVERY_COUNT,
                    );
                }
                // A request is sent when it is made or not at all, so no
                // message waits for credit: asked to drain, the link
                // advances its delivery-count by the credit, which
                // consumes it, and sends its flow state (§2.6.7); asked
                // for its state (`echo`, §2.7.4), it sends it as it is.
                let drained = *drain && self.credit > 0;
                if drained {
                    self.delivery_count = self.delivery_count.wrapping_add(self.credit);
                    self.credit = 0;
                }
                if drained || *echo {
                    writer.sender_flow(
                        self.channel,
                        self.handle,
                        self.delivery_count,
                        self.credit,
                        0,
                        *drain,
                    )?;
                }
            }
            Inbound::Disposition {
                channel,
                role_receiver: true,
                first,
                last,
                settled,
                state,
                ..
            } if *channel == self.channel => {
                let ids: Vec<u32> = self
                    .pending
                    .keys()
                    .copied()
                    .filter(|id| {
                        id.wrapping_sub(*first) < 0x8000_0000
                            && last.wrapping_sub(*id) < 0x8000_0000
                    })
                    .collect();
                for id in ids {
                    let result = match state {
                        Some(DeliveryState::Declared(txn)) => Ok(Some(txn.clone())),
                        Some(DeliveryState::Outcome(Outcome::Accepted)) => Ok(None),
                        Some(DeliveryState::Outcome(Outcome::Rejected(Some(error)))) => {
                            Err(TxnError::from_error(error))
                        }
                        Some(DeliveryState::Outcome(other)) => Err(TxnError::Protocol(format!(
                            "the outcome {other:?} carries no transaction error"
                        ))),
                        Some(DeliveryState::Transactional(_)) => Err(TxnError::Protocol(
                            "a transactional-state on the control link".to_string(),
                        )),
                        // Not terminal: the answer is still to come.
                        Some(DeliveryState::Received { .. }) | None if !settled => continue,
                        Some(DeliveryState::Received { .. }) | None => {
                            Err(TxnError::Protocol("settled without an outcome".to_string()))
                        }
                    };
                    let Some(request) = self.pending.remove(&id) else {
                        continue;
                    };
                    let result = match (&request, result) {
                        (Request::Declare, Ok(None)) => Err(TxnError::Protocol(
                            "a declare answered with accepted".to_string(),
                        )),
                        (Request::Discharge { .. }, Ok(Some(_))) => Err(TxnError::Protocol(
                            "a discharge answered with declared".to_string(),
                        )),
                        (_, result) => result,
                    };
                    if !settled {
                        // Part 2 §2.6.12: the receiver reached a terminal
                        // state and waits; the sender settles.
                        writer.settle(self.channel, id, id)?;
                    }
                    events.push(Self::answered(id, request, result));
                }
            }
            Inbound::Detach {
                channel,
                remote_handle,
                error,
                ..
            } if *channel == self.channel && Some(*remote_handle) == self.remote_handle => {
                if self.closed.is_none() {
                    // Part 2 §2.6.5: the detach is answered.
                    writer.detach(self.channel, self.handle, None)?;
                }
                self.close(error.as_ref(), &mut events);
            }
            Inbound::End { channel, error } if *channel == self.channel => {
                self.close(error.as_ref(), &mut events);
            }
            Inbound::Close(error) => self.close(error.as_ref(), &mut events),
            _ => {}
        }
        for event in &events {
            self.note(event);
        }
        Ok(events)
    }
}

// ============================================================================
// One connection driven by the calling thread.
// ============================================================================

/// A message the broker transferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// The broker's delivery-id.
    pub delivery_id: u32,
    /// The encoded message sections.
    pub message: Vec<u8>,
    /// The broker settled it on sending.
    pub settled: bool,
    /// `transfer.state`: a `transactional-state` that names the
    /// transaction for a message acquired in one (§4.4.3).
    pub state: Option<DeliveryState>,
}

struct ClientLink {
    name: String,
    sender: bool,
    remote_handle: Option<u32>,
    /// The broker attached: with a node, or refusing.
    attached: Option<bool>,
    /// The broker detached, with its error.
    detached: Option<Option<AmqpError>>,
    /// This client sent its detach.
    closing: bool,
    /// Sender: deliveries the broker's credit admits. Receiver: the
    /// credit granted and not used.
    credit: u32,
    /// Sender: deliveries sent. Receiver: the broker's delivery-count.
    delivery_count: u32,
    queue: VecDeque<Received>,
    /// Receiver: the transaction the broker acquires in for this link.
    acquiring: Option<TxnId>,
    /// Receiver: the broker's last flow stated no credit (a drain was
    /// answered).
    drained: bool,
    /// Sender: the broker's drain flag (Part 2 §2.6.7).
    drain: bool,
    /// Sender: a message waits for credit ([`TxnClient::send`]); it is
    /// available, and the link does not drain before it went out.
    sending: bool,
}

/// The session all links of a [`TxnClient`] are on.
const CHANNEL: u16 = 0;
/// The handle of its control link.
const CONTROL: u32 = 0;
/// How long one read of the socket waits before the client looks at its
/// deadline.
const SLICE: Duration = Duration::from_millis(50);

/// One connection with one session and a control link, driven by the
/// calling thread: a transaction controller for a program that is not
/// event-driven.
pub struct TxnClient {
    writer: BrokerWriter,
    reader: BrokerReader,
    controller: TxnController,
    remote: RemoteOpen,
    links: BTreeMap<u32, ClientLink>,
    next_handle: u32,
    next_tag: u64,
    /// The broker's answers to deliveries this client sent: settled and
    /// state, by delivery-id.
    answers: BTreeMap<u32, (bool, Option<DeliveryState>)>,
    /// Deliveries of the broker this client retired without settling:
    /// `Some` once the broker settled, with the state it gave.
    settlements: BTreeMap<u32, Option<Option<DeliveryState>>>,
    events: VecDeque<TxnEvent>,
    begun: bool,
    ended: Option<String>,
    timeout: Duration,
    last_heartbeat: Instant,
}

impl TxnClient {
    /// Connects, begins a session and attaches the control link with the
    /// `desired` capabilities; returns once the coordinator attached with
    /// every one of them and granted credit. Every later call waits at
    /// most `timeout` for the broker.
    ///
    /// # Errors
    /// The errors of [`connect`]; the error the broker refused the
    /// control link with; `TimedOut` when it does not answer;
    /// `Unsupported` when the coordinator lacks a desired capability
    /// (§4.5.1: "it is the responsibility of the transaction controller
    /// to verify that the capabilities of the controller meet its
    /// requirements"), and the connection is closed then.
    pub fn connect(
        params: &ConnectParams,
        desired: &[TxnCapability],
        timeout: Duration,
    ) -> io::Result<Self> {
        let (mut writer, reader, remote) = connect(params).map_err(|e| e.error)?;
        reader.set_read_timeout(Some(SLICE))?;
        writer.begin(CHANNEL)?;
        let controller = TxnController::new(CHANNEL, CONTROL, "txn-ctl");
        let mut client = Self {
            writer,
            reader,
            controller,
            remote,
            links: BTreeMap::new(),
            next_handle: CONTROL + 1,
            next_tag: 0,
            answers: BTreeMap::new(),
            settlements: BTreeMap::new(),
            events: VecDeque::new(),
            begun: false,
            ended: None,
            timeout,
            last_heartbeat: Instant::now(),
        };
        client.wait("the broker's begin", |c| c.begun.then_some(()))?;
        client.controller.attach(&mut client.writer, desired)?;
        client.wait("the coordinator's attach and credit", |c| {
            if let Some(TxnEvent::Closed(error)) =
                c.events.iter().find(|e| matches!(e, TxnEvent::Closed(_)))
            {
                return Some(Err(TxnError::Closed(error.clone())));
            }
            (c.controller.coordinator().is_some() && c.controller.credit() > 0).then_some(Ok(()))
        })??;
        let lacking = client.coordinator().lacking(desired);
        if !lacking.is_empty() {
            let names: Vec<&str> = lacking.iter().map(|c| c.symbol()).collect();
            client.close();
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "the coordinator lacks the desired capabilities {}",
                    names.join(", ")
                ),
            ));
        }
        Ok(client)
    }

    /// The coordinator's capabilities (§4.5.1, §4.5.7).
    #[must_use]
    pub fn coordinator(&self) -> Coordinator {
        self.controller.coordinator().cloned().unwrap_or_default()
    }

    /// What the broker's `open` said.
    #[must_use]
    pub fn remote_open(&self) -> &RemoteOpen {
        &self.remote
    }

    /// Takes txn-ids of up to `octets` octets from the coordinator in
    /// place of the 32 of §4.5.4 ([`BrokerReader::set_txn_id_limit`]):
    /// Apache ActiveMQ Artemis declares transactions with 36 octets, the
    /// text of a UUID. Without it, such a `declared` fails the declare
    /// with a protocol error.
    pub fn set_txn_id_limit(&mut self, octets: usize) {
        self.reader.set_txn_id_limit(octets);
    }

    fn gone(&self) -> io::Error {
        io::Error::new(
            io::ErrorKind::ConnectionAborted,
            self.ended
                .clone()
                .unwrap_or_else(|| "the connection ended".to_string()),
        )
    }

    /// Reads events until `done` yields, at most for the time-out.
    fn wait<T>(&mut self, what: &str, done: impl Fn(&Self) -> Option<T>) -> io::Result<T> {
        let deadline = Instant::now() + self.timeout;
        self.wait_until(deadline, done)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!("no answer within {:?}: {what}", self.timeout),
            )
        })
    }

    /// Reads events until `done` yields or `deadline` passes.
    fn wait_until<T>(
        &mut self,
        deadline: Instant,
        done: impl Fn(&Self) -> Option<T>,
    ) -> io::Result<Option<T>> {
        loop {
            if let Some(value) = done(self) {
                return Ok(Some(value));
            }
            if self.ended.is_some() {
                return Err(self.gone());
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            match self.reader.next_event() {
                Ok(event) => self.dispatch(&event)?,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    self.keep_alive()?;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Part 2 §2.4.5: a frame within the broker's idle time-out.
    fn keep_alive(&mut self) -> io::Result<()> {
        if let Some(idle) = self.remote.idle_time_out_ms.filter(|t| *t > 0)
            && self.last_heartbeat.elapsed() >= Duration::from_millis(u64::from(idle) / 4)
        {
            self.writer.heartbeat()?;
            self.last_heartbeat = Instant::now();
        }
        Ok(())
    }

    fn dispatch(&mut self, event: &Inbound) -> io::Result<()> {
        for txn_event in self.controller.on_event(&mut self.writer, event)? {
            self.events.push_back(txn_event);
        }
        match event {
            Inbound::Begin {
                channel,
                incoming_window,
                ..
            } => {
                self.writer.session_begun(*channel, *incoming_window);
                self.begun = true;
            }
            Inbound::Attach {
                name,
                remote_handle,
                has_node,
                initial_delivery_count,
                ..
            } => {
                if let Some(link) = self.links.values_mut().find(|l| l.name == *name) {
                    link.remote_handle = Some(*remote_handle);
                    link.attached = Some(*has_node);
                    if !link.sender {
                        link.delivery_count = initial_delivery_count.unwrap_or(0);
                    }
                }
            }
            Inbound::Flow {
                channel,
                next_incoming_id,
                incoming_window,
                remote_handle,
                delivery_count,
                link_credit,
                echo,
                drain,
            } => {
                self.writer
                    .apply_session_flow(*channel, *next_incoming_id, *incoming_window);
                let Some(remote) = remote_handle else {
                    // Part 2 §2.7.4: "If set to true, and the handle field
                    // is not set, then the sender only requires session
                    // endpoint state to be echoed".
                    if *echo {
                        self.writer.session_flow(*channel)?;
                    }
                    return Ok(());
                };
                let Some((handle, link)) = self
                    .links
                    .iter_mut()
                    .find(|(_, l)| l.remote_handle == Some(*remote))
                else {
                    return Ok(());
                };
                if link.sender {
                    if let Some(link_credit) = link_credit {
                        link.credit = sender_credit(
                            *delivery_count,
                            *link_credit,
                            link.delivery_count,
                            INITIAL_DELIVERY_COUNT,
                        );
                    }
                    link.drain = *drain;
                    // Part 2 §2.6.7: asked to drain with no message waiting,
                    // the link consumes its credit; §2.7.4: asked for its
                    // state, it sends it; one flow answers both. A message
                    // that waits for credit drains the link once it went
                    // out (`transfer`).
                    let drained = link.drain && link.credit > 0 && !link.sending;
                    if drained {
                        link.delivery_count = link.delivery_count.wrapping_add(link.credit);
                        link.credit = 0;
                    }
                    if drained || *echo {
                        self.writer.sender_flow(
                            CHANNEL,
                            *handle,
                            link.delivery_count,
                            link.credit,
                            u32::from(link.sending),
                            link.drain,
                        )?;
                    }
                } else {
                    // The sender's flow state: after a drain its
                    // delivery-count moved and the credit is zero.
                    if let Some(count) = delivery_count {
                        link.delivery_count = *count;
                    }
                    if let Some(link_credit) = link_credit {
                        link.credit = *link_credit;
                        link.drained = *link_credit == 0;
                    }
                    // §2.7.4: the receiver's state, the delivery-count it
                    // counts and the credit it holds granted.
                    if *echo {
                        self.writer
                            .flow(CHANNEL, *handle, link.delivery_count, link.credit)?;
                    }
                }
            }
            Inbound::Transfer {
                remote_handle,
                delivery_id,
                settled,
                message,
                state,
                ..
            } => {
                if let Some(link) = self
                    .links
                    .values_mut()
                    .find(|l| l.remote_handle == Some(*remote_handle))
                {
                    link.delivery_count = link.delivery_count.wrapping_add(1);
                    link.credit = link.credit.saturating_sub(1);
                    link.queue.push_back(Received {
                        delivery_id: *delivery_id,
                        message: message.clone(),
                        settled: *settled,
                        state: state.clone(),
                    });
                }
            }
            Inbound::Disposition {
                role_receiver,
                first,
                last,
                settled,
                state,
                ..
            } => {
                let covers = |id: u32| {
                    id.wrapping_sub(*first) < 0x8000_0000 && last.wrapping_sub(id) < 0x8000_0000
                };
                if *role_receiver {
                    for (id, answer) in &mut self.answers {
                        if covers(*id) {
                            *answer = (*settled, state.clone());
                        }
                    }
                } else if *settled {
                    for (id, settlement) in &mut self.settlements {
                        if covers(*id) {
                            *settlement = Some(state.clone());
                        }
                    }
                }
            }
            Inbound::Detach {
                remote_handle,
                error,
                ..
            } => {
                let found = self
                    .links
                    .iter_mut()
                    .find(|(_, l)| l.remote_handle == Some(*remote_handle));
                if let Some((handle, link)) = found
                    && link.detached.is_none()
                {
                    link.detached = Some(error.clone());
                    if !link.closing {
                        // Part 2 §2.6.5: the detach is answered.
                        self.writer.detach(CHANNEL, *handle, None)?;
                    }
                }
            }
            Inbound::End { error, .. } => {
                self.ended = Some(match error {
                    Some(e) => format!("the broker ended the session: {e}"),
                    None => "the broker ended the session".to_string(),
                });
            }
            Inbound::Close(error) => {
                self.ended = Some(match error {
                    Some(e) => format!("the broker closed the connection: {e}"),
                    None => "the broker closed the connection".to_string(),
                });
            }
            Inbound::Heartbeat => {}
        }
        Ok(())
    }

    /// Waits for the answer of the control link to the request
    /// `delivery_id`.
    fn answer_of(&mut self, delivery_id: u32) -> io::Result<TxnEvent> {
        let found = self.wait("the coordinator's answer", |c| {
            c.events.iter().position(|e| match e {
                TxnEvent::Declared { delivery_id: d, .. }
                | TxnEvent::Discharged { delivery_id: d, .. } => *d == delivery_id,
                TxnEvent::Attached(_) | TxnEvent::Closed(_) => false,
            })
        })?;
        self.events
            .remove(found)
            .ok_or_else(|| io::Error::other("the coordinator's answer is gone"))
    }

    /// Declares a transaction (§4.2) and returns its identifier.
    ///
    /// # Errors
    /// The transaction error of the coordinator; [`TxnError::Unsupported`]
    /// without sending, for a second transaction while one is open when
    /// the coordinator did not state `amqp:multi-txns-per-ssn` (§4.5.7);
    /// I/O errors and the time-out as [`TxnError::Io`].
    pub fn declare(&mut self) -> Result<TxnId, TxnError> {
        if let Some(error) = self.controller.closed() {
            return Err(TxnError::Closed(error.clone()));
        }
        self.controller
            .admits_declare()
            .map_err(TxnError::Unsupported)?;
        let io = |e: io::Error| TxnError::Io(e.to_string());
        let delivery_id = self.controller.declare(&mut self.writer).map_err(io)?;
        match self.answer_of(delivery_id).map_err(io)? {
            TxnEvent::Declared { result, .. } => result,
            other => Err(TxnError::Protocol(format!(
                "{other:?} in answer to a declare"
            ))),
        }
    }

    fn discharge(&mut self, txn: &TxnId, fail: bool) -> Result<(), TxnError> {
        if let Some(error) = self.controller.closed() {
            return Err(TxnError::Closed(error.clone()));
        }
        let io = |e: io::Error| TxnError::Io(e.to_string());
        let delivery_id = self
            .controller
            .discharge(&mut self.writer, txn, fail)
            .map_err(io)?;
        match self.answer_of(delivery_id).map_err(io)? {
            TxnEvent::Discharged { result, .. } => result,
            other => Err(TxnError::Protocol(format!(
                "{other:?} in answer to a discharge"
            ))),
        }
    }

    /// Commits `txn` (§4.3: a discharge without `fail`).
    ///
    /// # Errors
    /// The transaction error; the transaction is rolled back then.
    pub fn commit(&mut self, txn: &TxnId) -> Result<(), TxnError> {
        self.discharge(txn, false)
    }

    /// Rolls `txn` back (§4.3: a discharge with `fail`).
    ///
    /// # Errors
    /// The transaction error.
    pub fn rollback(&mut self, txn: &TxnId) -> Result<(), TxnError> {
        self.discharge(txn, true)
    }

    fn attach(&mut self, address: &str, sender: bool) -> io::Result<u32> {
        let handle = self.next_handle;
        self.next_handle += 1;
        let name = format!("{}-{handle}-{address}", if sender { "snd" } else { "rcv" });
        self.links.insert(
            handle,
            ClientLink {
                name: name.clone(),
                sender,
                remote_handle: None,
                attached: None,
                detached: None,
                closing: false,
                credit: 0,
                delivery_count: 0,
                queue: VecDeque::new(),
                acquiring: None,
                drained: false,
                drain: false,
                sending: false,
            },
        );
        self.writer.attach(
            CHANNEL,
            &LinkSpec {
                name,
                handle,
                sender,
                address: address.to_string(),
                // Mixed: a delivery is sent settled or not (§4.4.4 knows
                // both).
                snd_settle_mode: 2,
                rcv_settle_mode: 0,
                durable: 0,
                properties: Vec::new(),
                desired_capabilities: Vec::new(),
                coordinator: None,
            },
        )?;
        let refused = self.wait("the broker's attach", |c| {
            let link = c.links.get(&handle)?;
            match (link.attached?, &link.detached) {
                (_, Some(error)) => Some(Some(error.clone())),
                // A sender waits for the broker's credit as well.
                (true, None) if !sender || link.credit > 0 => Some(None),
                (_, None) => None,
            }
        })?;
        match refused {
            None => Ok(handle),
            Some(error) => Err(io::Error::other(match error {
                Some(e) => format!("the broker refused the link to {address}: {e}"),
                None => format!("the broker detached the link to {address}"),
            })),
        }
    }

    /// Attaches a link this client sends on, to the node `address`;
    /// returns once the broker granted credit.
    ///
    /// # Errors
    /// The error the broker refused the link with; I/O errors; the
    /// time-out.
    pub fn sender(&mut self, address: &str) -> io::Result<u32> {
        self.attach(address, true)
    }

    /// Attaches a link this client receives on, from the node `address`.
    ///
    /// # Errors
    /// As [`TxnClient::sender`].
    pub fn receiver(&mut self, address: &str) -> io::Result<u32> {
        self.attach(address, false)
    }

    fn link(&mut self, handle: u32, sender: bool) -> io::Result<&mut ClientLink> {
        let link = self
            .links
            .get_mut(&handle)
            .filter(|l| l.sender == sender)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("no such link {handle}"),
                )
            })?;
        if let Some(error) = &link.detached {
            return Err(io::Error::other(match error {
                Some(e) => format!("the broker detached link {}: {e}", link.name),
                None => format!("the broker detached link {}", link.name),
            }));
        }
        Ok(link)
    }

    /// Sends `message` on sending link `link` once the broker grants
    /// credit. While it waits, the message is available (Part 2 §2.6.7):
    /// a drain the broker asks for meanwhile is answered once it went out,
    /// with the credit left.
    fn transfer(
        &mut self,
        link: u32,
        message: &[u8],
        txn: Option<&TxnId>,
        settled: bool,
    ) -> io::Result<u32> {
        if let Some(l) = self.links.get_mut(&link) {
            l.sending = true;
        }
        let sent = self.transfer_with_credit(link, message, txn, settled);
        if let Some(l) = self.links.get_mut(&link) {
            l.sending = false;
            if l.drain && l.credit > 0 && l.detached.is_none() && !l.closing {
                l.delivery_count = l.delivery_count.wrapping_add(l.credit);
                l.credit = 0;
                self.writer
                    .sender_flow(CHANNEL, link, l.delivery_count, 0, 0, true)?;
            }
        }
        sent
    }

    fn transfer_with_credit(
        &mut self,
        link: u32,
        message: &[u8],
        txn: Option<&TxnId>,
        settled: bool,
    ) -> io::Result<u32> {
        // Wait for credit: the broker restores it as it takes messages.
        self.wait("credit of the broker", |c| {
            c.links
                .get(&link)
                .is_none_or(|l| l.credit > 0 || l.detached.is_some())
                .then_some(())
        })?;
        self.link(link, true)?;
        self.next_tag += 1;
        let tag = self.next_tag.to_be_bytes();
        let state = txn.map(posted);
        let delivery_id = self.writer.transfer_with_state(
            CHANNEL,
            link,
            &tag,
            message,
            settled,
            state.as_ref(),
        )?;
        let l = self.link(link, true)?;
        l.credit -= 1;
        l.delivery_count = l.delivery_count.wrapping_add(1);
        Ok(delivery_id)
    }

    /// Sends `message` unsettled on the sending link `link`, in `txn` if
    /// one is given (§4.4.1), and returns the broker's answer: for a
    /// posted message a `transactional-state` of the same transaction
    /// with the outcome that applies with the commit.
    ///
    /// # Errors
    /// I/O errors; a link the broker detached, with its error; the
    /// time-out.
    pub fn send(
        &mut self,
        link: u32,
        message: &[u8],
        txn: Option<&TxnId>,
    ) -> io::Result<Option<DeliveryState>> {
        let delivery_id = self.transfer(link, message, txn, false)?;
        self.answers.insert(delivery_id, (false, None));
        let answered = self.wait("the broker's disposition", |c| {
            if let Some(link) = c.links.get(&link)
                && let Some(error) = &link.detached
            {
                return Some(Err(error.clone()));
            }
            c.answers
                .get(&delivery_id)
                .filter(|(settled, state)| *settled || state.is_some())
                .cloned()
                .map(Ok)
        });
        self.answers.remove(&delivery_id);
        match answered? {
            Ok((settled, state)) => {
                if !settled {
                    // §4.4.4: "The controller SHOULD settle any
                    // outstanding unsettled deliveries in a timely
                    // fashion".
                    self.writer.settle(CHANNEL, delivery_id, delivery_id)?;
                }
                Ok(state)
            }
            Err(error) => Err(io::Error::other(match error {
                Some(e) => format!("the broker detached the link: {e}"),
                None => "the broker detached the link".to_string(),
            })),
        }
    }

    /// Sends `message` settled, in `txn` if one is given (§4.4.4
    /// "Delivery Sent Settled By Controller"): no answer follows; if the
    /// resource cannot take it, the transaction does not commit.
    ///
    /// # Errors
    /// I/O errors; a link the broker detached; the time-out.
    pub fn send_settled(
        &mut self,
        link: u32,
        message: &[u8],
        txn: Option<&TxnId>,
    ) -> io::Result<()> {
        self.transfer(link, message, txn, true).map(|_| ())
    }

    /// The next message of the receiving link `link`, waiting at most
    /// `wait`: the client grants one credit, with the transaction to
    /// acquire in if `txn` is given (§4.4.3). When none arrives, the link
    /// is drained so that no credit stays with the broker (§4.4.3: "the
    /// controller SHOULD ensure there is no outstanding credit at the
    /// sender before it discharges the transaction").
    ///
    /// # Errors
    /// I/O errors; a link the broker detached, with its error.
    pub fn receive(
        &mut self,
        link: u32,
        txn: Option<&TxnId>,
        wait: Duration,
    ) -> io::Result<Option<Received>> {
        let l = self.link(link, false)?;
        if let Some(message) = l.queue.pop_front() {
            return Ok(Some(message));
        }
        // The properties of the flow determine the transaction of the
        // link: sent when it changes.
        let change = (l.acquiring.as_ref() != txn).then_some(txn);
        l.acquiring = txn.cloned();
        l.credit = 1;
        l.drained = false;
        let delivery_count = l.delivery_count;
        self.writer
            .flow_in_txn(CHANNEL, link, delivery_count, 1, false, change)?;
        let deadline = Instant::now() + wait;
        let arrived = self.wait_until(deadline, |c| {
            let l = c.links.get(&link)?;
            (!l.queue.is_empty() || l.detached.is_some()).then_some(())
        })?;
        if arrived.is_none() {
            // Drain: the broker uses the credit or gives it back.
            let l = self.link(link, false)?;
            let (delivery_count, credit) = (l.delivery_count, l.credit);
            self.writer
                .flow_in_txn(CHANNEL, link, delivery_count, credit, true, None)?;
            self.wait("the broker's answer to the drain", |c| {
                let l = c.links.get(&link)?;
                (l.drained || !l.queue.is_empty() || l.detached.is_some()).then_some(())
            })?;
        }
        Ok(self.link(link, false)?.queue.pop_front())
    }

    /// States the outcome of the broker's delivery `delivery_id`: in
    /// `txn` if one is given (§4.4.2), where it applies with the commit;
    /// `settle` settles the delivery with it.
    ///
    /// # Errors
    /// I/O errors.
    pub fn retire(
        &mut self,
        delivery_id: u32,
        outcome: Outcome,
        txn: Option<&TxnId>,
        settle: bool,
    ) -> io::Result<()> {
        let state = match txn {
            Some(txn) => retired(txn, outcome),
            None => DeliveryState::Outcome(outcome),
        };
        if !settle {
            self.settlements.insert(delivery_id, None);
        }
        // else: settled by this client, the broker says no more.
        self.writer
            .disposition_with_state(CHANNEL, delivery_id, delivery_id, settle, &state)
    }

    /// Waits at most `wait` for the broker to settle its delivery
    /// `delivery_id`, which this client retired without settling; returns
    /// the state the broker settled it with (`Some(None)`: settled
    /// without a state). `Ok(None)`: not settled in that time.
    ///
    /// # Errors
    /// I/O errors.
    pub fn settled(
        &mut self,
        delivery_id: u32,
        wait: Duration,
    ) -> io::Result<Option<Option<DeliveryState>>> {
        let deadline = Instant::now() + wait;
        let settled = self.wait_until(deadline, |c| {
            c.settlements.get(&delivery_id).cloned().flatten()
        })?;
        if settled.is_some() {
            self.settlements.remove(&delivery_id);
        }
        Ok(settled)
    }

    /// Detaches the link and waits for the broker's detach.
    ///
    /// # Errors
    /// I/O errors; the time-out.
    pub fn detach(&mut self, link: u32) -> io::Result<()> {
        let Some(l) = self.links.get_mut(&link) else {
            return Ok(());
        };
        if l.detached.is_none() {
            // Marked first: the broker's detach is the answer then.
            l.closing = true;
            self.writer.detach(CHANNEL, link, None)?;
            self.wait("the broker's detach", |c| {
                c.links
                    .get(&link)
                    .is_none_or(|l| l.detached.is_some())
                    .then_some(())
            })?;
        }
        self.links.remove(&link);
        Ok(())
    }

    /// Ends the session and closes the connection.
    pub fn close(self) {
        self.writer.close(None);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// §4.5.8: the error of a refused declare or discharge is one of the
    /// three transaction errors by its condition; another condition is
    /// kept as it came.
    #[test]
    fn amqp10_4_5_8_an_error_of_the_coordinator_is_read_by_its_condition() {
        assert_eq!(
            TxnError::from_error(&AmqpError::new("amqp:transaction:unknown-id", "no 07")),
            TxnError::UnknownId("no 07".into())
        );
        assert_eq!(
            TxnError::from_error(&AmqpError::new("amqp:transaction:rollback", "node full")),
            TxnError::Rollback("node full".into())
        );
        assert_eq!(
            TxnError::from_error(&AmqpError::new("amqp:transaction:timeout", "300 s")),
            TxnError::Timeout("300 s".into())
        );
        let other = AmqpError::new("amqp:resource-limit-exceeded", "64 transactions");
        assert_eq!(
            TxnError::from_error(&other),
            TxnError::Refused(other.clone())
        );
        assert!(
            TxnError::Refused(other)
                .to_string()
                .contains("amqp:resource-limit-exceeded")
        );
    }

    /// §4.4.1, §4.4.2: the state of a posted transfer names the
    /// transaction and no outcome; the state of a retirement names both.
    #[test]
    fn amqp10_4_4_the_states_of_transactional_work_name_the_transaction() {
        let txn = TxnId::new(&[1, 2]).unwrap();
        assert_eq!(
            posted(&txn),
            DeliveryState::Transactional(TransactionalState {
                txn_id: txn.clone(),
                outcome: None
            })
        );
        assert_eq!(
            retired(&txn, Outcome::Accepted),
            DeliveryState::Transactional(TransactionalState {
                txn_id: txn,
                outcome: Some(Outcome::Accepted)
            })
        );
    }

    /// §4.2: the control link sends unsettled, its target is a
    /// coordinator with the desired capabilities, and it is a link the
    /// client sends on.
    #[test]
    fn amqp10_4_2_the_control_link_sends_unsettled_to_a_coordinator() {
        let controller = TxnController::new(3, 7, "ctl");
        let spec = controller.link_spec(&[TxnCapability::LocalTransactions]);
        assert_eq!(spec.name, "ctl");
        assert_eq!(spec.handle, 7);
        assert!(spec.sender);
        assert_eq!(spec.snd_settle_mode, 0, "unsettled");
        assert_eq!(
            spec.coordinator,
            Some(vec![TxnCapability::LocalTransactions])
        );
        assert!(controller.coordinator().is_none());
        assert_eq!(controller.credit(), 0);
        assert!(controller.closed().is_none());
    }

    use crate::broker::tests::{Written, writer_and_written};
    use crate::transactions::{CoordinatorRequest, coordinator_request};
    use zerodds_amqp_bridge::performatives::descriptor;

    /// The session of the writer of [`writer_and_written`].
    const SESSION: u16 = 3;
    /// The coordinator's handle of the control link.
    const REMOTE: u32 = 40;

    fn coordinator() -> Coordinator {
        Coordinator {
            capabilities: vec![
                TxnCapability::LocalTransactions,
                TxnCapability::MultiTxnsPerSsn,
            ],
        }
    }

    fn attach_event(coordinator: Option<Coordinator>, has_node: bool) -> Inbound {
        Inbound::Attach {
            channel: SESSION,
            name: "ctl".into(),
            remote_handle: REMOTE,
            has_node,
            initial_delivery_count: None,
            coordinator,
        }
    }

    fn credit_event(delivery_count: u32, link_credit: u32) -> Inbound {
        Inbound::Flow {
            channel: SESSION,
            next_incoming_id: Some(0),
            incoming_window: 100,
            remote_handle: Some(REMOTE),
            delivery_count: Some(delivery_count),
            link_credit: Some(link_credit),
            echo: false,
            drain: false,
        }
    }

    fn answer_event(delivery_id: u32, settled: bool, state: Option<DeliveryState>) -> Inbound {
        Inbound::Disposition {
            channel: SESSION,
            role_receiver: true,
            first: delivery_id,
            last: delivery_id,
            settled,
            outcome: state.as_ref().map_or(Outcome::None, DeliveryState::outcome),
            state,
            annotations: Vec::new(),
        }
    }

    /// A controller whose coordinator attached and granted `credit`.
    fn attached(credit: u32) -> (TxnController, BrokerWriter, Written) {
        let (mut writer, written) = writer_and_written();
        let mut controller = TxnController::new(SESSION, 7, "ctl");
        let events = controller
            .on_event(&mut writer, &attach_event(Some(coordinator()), true))
            .unwrap();
        assert_eq!(events, vec![TxnEvent::Attached(coordinator())]);
        assert!(
            controller
                .on_event(&mut writer, &credit_event(0, credit))
                .unwrap()
                .is_empty()
        );
        assert_eq!(controller.credit(), credit);
        (controller, writer, written)
    }

    fn txn(octets: &[u8]) -> TxnId {
        TxnId::new(octets).unwrap()
    }

    /// §4.2, Figure 4.1: a declare is a message with the declare in one
    /// amqp-value section, sent unsettled within the coordinator's
    /// credit; the disposition with declared yields the txn-id. Before
    /// the coordinator attached and without credit nothing is sent.
    #[test]
    fn amqp10_4_2_a_declare_is_sent_unsettled_and_answered_with_the_txn_id() {
        let (mut writer, mut written) = writer_and_written();
        let mut controller = TxnController::new(SESSION, 7, "ctl");
        assert_eq!(
            controller.declare(&mut writer).unwrap_err().kind(),
            io::ErrorKind::NotConnected,
            "the coordinator has not attached"
        );
        controller
            .on_event(&mut writer, &attach_event(Some(coordinator()), true))
            .unwrap();
        assert_eq!(controller.coordinator(), Some(&coordinator()));
        assert_eq!(
            controller.declare(&mut writer).unwrap_err().kind(),
            io::ErrorKind::WouldBlock,
            "no credit yet"
        );
        controller
            .on_event(&mut writer, &credit_event(0, 2))
            .unwrap();
        let delivery = controller.declare(&mut writer).unwrap();
        assert_eq!(controller.credit(), 1);
        let (code, fields, payload) = written.next();
        assert_eq!(code, descriptor::TRANSFER);
        assert_eq!(fields[0], AmqpExtValue::Uint(7), "the control link");
        assert_eq!(fields[4], AmqpExtValue::Boolean(false), "not settled");
        assert_eq!(
            coordinator_request(&payload),
            Ok(CoordinatorRequest::Declare(Declare::default()))
        );
        // A disposition for another delivery, and one that is not
        // terminal, answer nothing.
        assert!(
            controller
                .on_event(
                    &mut writer,
                    &answer_event(delivery + 5, true, Some(DeliveryState::Declared(txn(&[1]))))
                )
                .unwrap()
                .is_empty()
        );
        assert!(
            controller
                .on_event(&mut writer, &answer_event(delivery, false, None))
                .unwrap()
                .is_empty()
        );
        let events = controller
            .on_event(
                &mut writer,
                &answer_event(delivery, true, Some(DeliveryState::Declared(txn(&[1])))),
            )
            .unwrap();
        assert_eq!(
            events,
            vec![TxnEvent::Declared {
                delivery_id: delivery,
                result: Ok(txn(&[1]))
            }]
        );
        // Answered once.
        assert!(
            controller
                .on_event(
                    &mut writer,
                    &answer_event(delivery, true, Some(DeliveryState::Declared(txn(&[1]))))
                )
                .unwrap()
                .is_empty()
        );
        // §4.5.2: no global-id without the capability.
        assert_eq!(
            controller
                .declare_global(&mut writer, AmqpExtValue::Binary(vec![1]))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }

    /// §4.5.1, §4.5.7: the controller verifies the coordinator's
    /// capabilities. Without amqp:multi-txns-per-ssn no second declare is
    /// sent while a declare is unanswered or a transaction is open; once
    /// the transaction is discharged, or the link is gone, one is again.
    #[test]
    fn amqp10_4_5_7_without_multi_txns_per_ssn_one_transaction_at_a_time() {
        let (mut writer, mut written) = writer_and_written();
        let mut controller = TxnController::new(SESSION, 7, "ctl");
        let local = Coordinator {
            capabilities: vec![TxnCapability::LocalTransactions],
        };
        controller
            .on_event(&mut writer, &attach_event(Some(local), true))
            .unwrap();
        controller
            .on_event(&mut writer, &credit_event(0, 10))
            .unwrap();
        let first = controller.declare(&mut writer).unwrap();
        let refused = |c: &mut TxnController, w: &mut BrokerWriter| {
            assert_eq!(
                c.declare(w).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(
                c.declare_global(w, AmqpExtValue::Binary(vec![1]))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
            assert!(
                c.admits_declare()
                    .unwrap_err()
                    .contains("amqp:multi-txns-per-ssn")
            );
        };
        // A declare unanswered.
        refused(&mut controller, &mut writer);
        controller
            .on_event(
                &mut writer,
                &answer_event(first, true, Some(DeliveryState::Declared(txn(&[1])))),
            )
            .unwrap();
        assert_eq!(controller.open_transactions(), [txn(&[1])]);
        // A transaction open.
        refused(&mut controller, &mut writer);
        let discharge = controller
            .discharge(&mut writer, &txn(&[1]), false)
            .unwrap();
        assert_eq!(discharge, first.wrapping_add(1), "nothing was sent between");
        controller
            .on_event(
                &mut writer,
                &answer_event(
                    discharge,
                    true,
                    Some(DeliveryState::Outcome(Outcome::Accepted)),
                ),
            )
            .unwrap();
        assert!(controller.open_transactions().is_empty());
        assert_eq!(controller.admits_declare(), Ok(()));
        let second = controller.declare(&mut writer).unwrap();
        assert_eq!(second, discharge.wrapping_add(1));
        for expected in [
            CoordinatorRequest::Declare(Declare::default()),
            CoordinatorRequest::Discharge(Discharge {
                txn_id: txn(&[1]),
                fail: false,
            }),
            CoordinatorRequest::Declare(Declare::default()),
        ] {
            let (code, _, payload) = written.next();
            assert_eq!(code, descriptor::TRANSFER);
            assert_eq!(coordinator_request(&payload), Ok(expected));
        }
        // A refused discharge ends the transaction as well (§4.4.2), and
        // the end of the link ends all (§4.1).
        controller
            .on_event(
                &mut writer,
                &answer_event(second, true, Some(DeliveryState::Declared(txn(&[2])))),
            )
            .unwrap();
        let rejected = controller
            .discharge(&mut writer, &txn(&[2]), false)
            .unwrap();
        controller
            .on_event(
                &mut writer,
                &answer_event(
                    rejected,
                    true,
                    Some(DeliveryState::Outcome(Outcome::Rejected(Some(
                        AmqpError::new("amqp:transaction:rollback", "node full"),
                    )))),
                ),
            )
            .unwrap();
        assert!(controller.open_transactions().is_empty());
        let third = controller.declare(&mut writer).unwrap();
        controller
            .on_event(
                &mut writer,
                &answer_event(third, true, Some(DeliveryState::Declared(txn(&[3])))),
            )
            .unwrap();
        assert_eq!(controller.open_transactions(), [txn(&[3])]);
        controller
            .on_event(
                &mut writer,
                &Inbound::Detach {
                    channel: SESSION,
                    remote_handle: REMOTE,
                    closed: true,
                    error: None,
                },
            )
            .unwrap();
        assert!(controller.open_transactions().is_empty());
    }

    /// §4.5.7: with amqp:multi-txns-per-ssn several transactions are open
    /// at a time.
    #[test]
    fn amqp10_4_5_7_with_multi_txns_per_ssn_several_transactions_are_open() {
        let (mut controller, mut writer, _written) = attached(5);
        let one = controller.declare(&mut writer).unwrap();
        let two = controller.declare(&mut writer).unwrap();
        for (delivery, id) in [(one, 1), (two, 2)] {
            controller
                .on_event(
                    &mut writer,
                    &answer_event(delivery, true, Some(DeliveryState::Declared(txn(&[id])))),
                )
                .unwrap();
        }
        assert_eq!(controller.open_transactions(), [txn(&[1]), txn(&[2])]);
        assert_eq!(controller.admits_declare(), Ok(()));
        controller.declare(&mut writer).unwrap();
    }

    /// §4.3, Figure 4.2: a discharge names the transaction and the flag
    /// fail; accepted completes it. An answer the coordinator did not
    /// settle is settled by the controller (Part 2 §2.6.12).
    #[test]
    fn amqp10_4_3_a_discharge_names_the_transaction_and_is_answered_with_accepted() {
        let (mut controller, mut writer, mut written) = attached(5);
        for (fail, settled) in [(false, true), (true, false)] {
            let id = txn(&[7, u8::from(fail)]);
            let delivery = controller.discharge(&mut writer, &id, fail).unwrap();
            let (_, _, payload) = written.next();
            assert_eq!(
                coordinator_request(&payload),
                Ok(CoordinatorRequest::Discharge(Discharge {
                    txn_id: id.clone(),
                    fail
                }))
            );
            let events = controller
                .on_event(
                    &mut writer,
                    &answer_event(
                        delivery,
                        settled,
                        Some(DeliveryState::Outcome(Outcome::Accepted)),
                    ),
                )
                .unwrap();
            assert_eq!(
                events,
                vec![TxnEvent::Discharged {
                    delivery_id: delivery,
                    txn: id,
                    fail,
                    result: Ok(())
                }]
            );
            if !settled {
                let (code, fields, _) = written.next();
                assert_eq!(code, descriptor::DISPOSITION);
                assert_eq!(fields[0], AmqpExtValue::Boolean(false), "role sender");
                assert_eq!(fields[1], AmqpExtValue::Uint(delivery));
                assert_eq!(fields[3], AmqpExtValue::Boolean(true), "settled");
            }
        }
    }

    /// §4.2, §4.3: "If the source for the link to the coordinator
    /// supports the rejected outcome, then the message MUST be rejected
    /// with this outcome carrying the transaction-error": each of the
    /// three errors of §4.5.8 and another condition reach the caller; a
    /// state that is no answer to the request is a protocol error.
    #[test]
    fn amqp10_4_3_a_rejected_request_carries_the_transaction_error() {
        let (mut controller, mut writer, _written) = attached(20);
        let rejected = |condition: &str| {
            DeliveryState::Outcome(Outcome::Rejected(Some(AmqpError::new(condition, "why"))))
        };
        for (condition, expected) in [
            (
                "amqp:transaction:unknown-id",
                TxnError::UnknownId("why".into()),
            ),
            (
                "amqp:transaction:rollback",
                TxnError::Rollback("why".into()),
            ),
            ("amqp:transaction:timeout", TxnError::Timeout("why".into())),
            (
                "amqp:not-implemented",
                TxnError::Refused(AmqpError::new("amqp:not-implemented", "why")),
            ),
        ] {
            let id = txn(&[3]);
            let delivery = controller.discharge(&mut writer, &id, false).unwrap();
            let events = controller
                .on_event(
                    &mut writer,
                    &answer_event(delivery, true, Some(rejected(condition))),
                )
                .unwrap();
            assert_eq!(
                events,
                vec![TxnEvent::Discharged {
                    delivery_id: delivery,
                    txn: id,
                    fail: false,
                    result: Err(expected)
                }]
            );
        }
        let delivery = controller.declare(&mut writer).unwrap();
        let events = controller
            .on_event(
                &mut writer,
                &answer_event(
                    delivery,
                    true,
                    Some(rejected("amqp:resource-limit-exceeded")),
                ),
            )
            .unwrap();
        assert!(matches!(
            &events[0],
            TxnEvent::Declared {
                result: Err(TxnError::Refused(e)),
                ..
            } if e.condition == "amqp:resource-limit-exceeded"
        ));
        // Answers that are none.
        for state in [
            Some(DeliveryState::Outcome(Outcome::Accepted)),
            Some(DeliveryState::Outcome(Outcome::Released)),
            Some(posted(&txn(&[1]))),
            None,
        ] {
            let delivery = controller.declare(&mut writer).unwrap();
            let events = controller
                .on_event(&mut writer, &answer_event(delivery, true, state.clone()))
                .unwrap();
            assert!(
                matches!(
                    &events[0],
                    TxnEvent::Declared {
                        result: Err(TxnError::Protocol(_)),
                        ..
                    }
                ),
                "{state:?}: {events:?}"
            );
        }
        let delivery = controller.discharge(&mut writer, &txn(&[1]), true).unwrap();
        let events = controller
            .on_event(
                &mut writer,
                &answer_event(delivery, true, Some(DeliveryState::Declared(txn(&[2])))),
            )
            .unwrap();
        assert!(matches!(
            &events[0],
            TxnEvent::Discharged {
                result: Err(TxnError::Protocol(_)),
                ..
            }
        ));
    }

    /// §4.2, §4.3: "If the source does not support the rejected outcome,
    /// the transactional resource MUST detach the link to the
    /// coordinator, with the detach performative carrying the
    /// transaction-error"; §4.4.1, §4.4.2: the resource terminates the
    /// control link with amqp:transaction:rollback. A request that waits
    /// fails with the error of the detach, the detach is answered, and
    /// the link is gone: nothing more is sent on it (§4.1: its
    /// transactions are rolled back).
    #[test]
    fn amqp10_4_2_a_detach_of_the_control_link_carries_the_error_of_the_request() {
        let (mut controller, mut writer, mut written) = attached(5);
        let declare = controller.declare(&mut writer).unwrap();
        let id = txn(&[4]);
        let discharge = controller.discharge(&mut writer, &id, false).unwrap();
        written.next();
        written.next();
        let error = AmqpError::new("amqp:transaction:rollback", "a delivery posted in part");
        let events = controller
            .on_event(
                &mut writer,
                &Inbound::Detach {
                    channel: SESSION,
                    remote_handle: REMOTE,
                    closed: true,
                    error: Some(error.clone()),
                },
            )
            .unwrap();
        let failure = TxnError::Rollback("a delivery posted in part".into());
        assert_eq!(
            events,
            vec![
                TxnEvent::Declared {
                    delivery_id: declare,
                    result: Err(failure.clone())
                },
                TxnEvent::Discharged {
                    delivery_id: discharge,
                    txn: id,
                    fail: false,
                    result: Err(failure)
                },
                TxnEvent::Closed(Some(error.clone())),
            ]
        );
        let (code, fields, _) = written.next();
        assert_eq!(code, descriptor::DETACH, "the detach is answered");
        assert_eq!(fields[0], AmqpExtValue::Uint(7));
        assert_eq!(controller.closed(), Some(&Some(error)));
        assert_eq!(
            controller.declare(&mut writer).unwrap_err().kind(),
            io::ErrorKind::NotConnected
        );
        // A detach of another link is not the control link's.
        let (mut controller, mut writer, _written) = attached(5);
        assert!(
            controller
                .on_event(
                    &mut writer,
                    &Inbound::Detach {
                        channel: SESSION,
                        remote_handle: REMOTE + 1,
                        closed: true,
                        error: None,
                    },
                )
                .unwrap()
                .is_empty()
        );
        assert!(controller.closed().is_none());
        // The end of the session and the close of the connection end it
        // too; a request that waits fails as closed.
        let delivery = controller.declare(&mut writer).unwrap();
        let events = controller
            .on_event(
                &mut writer,
                &Inbound::End {
                    channel: SESSION,
                    error: None,
                },
            )
            .unwrap();
        assert_eq!(
            events,
            vec![
                TxnEvent::Declared {
                    delivery_id: delivery,
                    result: Err(TxnError::Closed(None))
                },
                TxnEvent::Closed(None)
            ]
        );
    }

    /// §4.2: "The container acting as the transactional resource defines
    /// a special target that functions as a transaction coordinator": a
    /// partner that answers the attach with a node that is no coordinator
    /// offers no transactions; the controller detaches the link with
    /// amqp:not-implemented. A refused attach (no node) waits for the
    /// detach that carries the reason.
    #[test]
    fn amqp10_4_2_a_partner_without_a_coordinator_is_no_transactional_resource() {
        let (mut writer, mut written) = writer_and_written();
        let mut controller = TxnController::new(SESSION, 7, "ctl");
        let events = controller
            .on_event(&mut writer, &attach_event(None, true))
            .unwrap();
        assert!(
            matches!(&events[..], [TxnEvent::Closed(Some(e))] if e.condition == "amqp:not-implemented"),
            "{events:?}"
        );
        let (code, fields, _) = written.next();
        assert_eq!(code, descriptor::DETACH);
        assert_eq!(fields[1], AmqpExtValue::Boolean(true), "closed");
        assert!(controller.closed().is_some());
        // A refused attach.
        let mut controller = TxnController::new(SESSION, 7, "ctl");
        assert!(
            controller
                .on_event(&mut writer, &attach_event(None, false))
                .unwrap()
                .is_empty()
        );
        let refusal = AmqpError::new("amqp:not-implemented", "no transactions");
        let events = controller
            .on_event(
                &mut writer,
                &Inbound::Detach {
                    channel: SESSION,
                    remote_handle: REMOTE,
                    closed: true,
                    error: Some(refusal.clone()),
                },
            )
            .unwrap();
        assert_eq!(events, vec![TxnEvent::Closed(Some(refusal))]);
        // An attach of another link is not the control link's.
        let mut controller = TxnController::new(SESSION, 7, "ctl");
        let other = Inbound::Attach {
            channel: SESSION,
            name: "another".into(),
            remote_handle: 9,
            has_node: true,
            initial_delivery_count: None,
            coordinator: None,
        };
        assert!(controller.on_event(&mut writer, &other).unwrap().is_empty());
        assert!(controller.closed().is_none());
    }

    fn flow_event(delivery_count: u32, link_credit: u32, drain: bool, echo: bool) -> Inbound {
        Inbound::Flow {
            channel: SESSION,
            next_incoming_id: Some(0),
            incoming_window: 100,
            remote_handle: Some(REMOTE),
            delivery_count: Some(delivery_count),
            link_credit: Some(link_credit),
            echo,
            drain,
        }
    }

    fn uint(v: &AmqpExtValue) -> u32 {
        match v {
            AmqpExtValue::Uint(n) => *n,
            other => panic!("not a uint: {other:?}"),
        }
    }

    /// Part 2 §2.6.7, §2.7.4: the control link sends a request only when
    /// one is made, so no message waits for credit: asked to drain, it
    /// advances its delivery-count by the credit, which consumes it, and
    /// sends its flow state; a flow with `echo` is answered with the state
    /// as it is; a credit behind the requests sent is none.
    #[test]
    fn amqp10_2_6_7_the_control_link_drains_and_answers_an_echo() {
        let (mut controller, mut writer, mut written) = attached(3);
        assert!(
            controller
                .on_event(&mut writer, &flow_event(0, 3, false, true))
                .unwrap()
                .is_empty()
        );
        let (channel, code, f) = written.try_next_on().expect("the echoed flow");
        assert_eq!((channel, code), (SESSION, descriptor::FLOW));
        assert_eq!(
            (uint(&f[4]), uint(&f[5]), uint(&f[6]), uint(&f[7])),
            (7, 0, 3, 0),
            "handle, delivery-count, link-credit, available"
        );
        assert_eq!(f[8], AmqpExtValue::Boolean(false), "drain");
        controller
            .on_event(&mut writer, &flow_event(0, 3, true, false))
            .unwrap();
        let (_, code, f) = written.try_next_on().expect("the drained flow");
        assert_eq!(code, descriptor::FLOW);
        assert_eq!((uint(&f[5]), uint(&f[6])), (3, 0));
        assert_eq!(f[8], AmqpExtValue::Boolean(true), "drain");
        assert_eq!(controller.credit(), 0);
        assert_eq!(
            controller.declare(&mut writer).unwrap_err().kind(),
            io::ErrorKind::WouldBlock,
            "the drained credit is gone"
        );
        // A flow sent before the coordinator saw the drain: behind the
        // count, no credit.
        controller
            .on_event(&mut writer, &flow_event(0, 2, false, false))
            .unwrap();
        assert_eq!(controller.credit(), 0);
        assert!(written.try_next_on().is_none());
    }
}
