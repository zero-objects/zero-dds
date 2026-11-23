// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Claims-based security (spec §7.2, §10): the `put-token` exchange of the
//! AMQP CBS working draft on the node `$cbs`, as Azure Service Bus
//! authorizes the links of a connection. A session of its own with a
//! sender to `$cbs` and a receiver from `$cbs` whose target is the
//! `reply-to` of the requests. Per AMQP address of the daemon one shared
//! access signature (SAS) token with the audience
//! `amqp://<host>/<address>` is put before the topic links are attached;
//! the tokens are put again at four fifths of their lifetime, on the same
//! links, so the broker never sees one expire. An answer other than 200 or
//! 202 ends the connection with the broker's status.

use std::collections::BTreeMap;
use std::string::{String, ToString};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::vec::Vec;

use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::sections::MessageSection;

use super::log::{Field, emit};
use crate::broker::{
    BrokerWriter, INITIAL_DELIVERY_COUNT, Inbound, LinkSpec, Outcome, sender_credit,
};

/// The token type of a shared access signature (Azure Service Bus).
pub(super) const SAS_TOKEN_TYPE: &str = "servicebus.windows.net:sastoken";

/// The credit the daemon grants the broker for answers.
const ANSWER_CREDIT: u32 = 16;

/// RFC 3986 unreserved characters stay; every other octet is `%XX`
/// (`quote_plus` would write a space as `+`; a URI has none).
fn percent_encode(s: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if unreserved || (keep_slash && b == b'/') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The shared access signature of `resource_uri` (Azure Service Bus,
/// "Generate SAS token"): HMAC-SHA256 with the UTF-8 octets of `key` over
/// the URL-encoded URI, a line feed and the expiry in seconds since the
/// Unix epoch; the signature Base64 and URL-encoded.
#[must_use]
pub(super) fn sas_token(
    resource_uri: &str,
    key_name: &str,
    key: &str,
    expiry_unix_s: u64,
) -> String {
    let sr = percent_encode(resource_uri, false);
    let to_sign = format!("{sr}\n{expiry_unix_s}");
    // HMAC takes a key of any length; new_from_slice does not fail.
    let sig = <Hmac<Sha256> as Mac>::new_from_slice(key.as_bytes())
        .map(|mut m| {
            m.update(to_sign.as_bytes());
            base64::engine::general_purpose::STANDARD.encode(m.finalize().into_bytes())
        })
        .unwrap_or_default();
    format!(
        "SharedAccessSignature sr={sr}&sig={}&se={expiry_unix_s}&skn={key_name}",
        percent_encode(&sig, true)
    )
}

/// The `put-token` request: message-id, `to` `$cbs`, `reply-to`; the
/// application properties `operation`, `type`, `name` (the audience); the
/// token as an `amqp-value` string.
///
/// # Errors
/// An encoding error of the codec.
pub(super) fn put_token_message(
    message_id: u64,
    reply_to: &str,
    audience: &str,
    token_type: &str,
    token: &str,
) -> Result<Vec<u8>, String> {
    let sections = [
        MessageSection::Properties(AmqpExtValue::List(vec![
            AmqpExtValue::Ulong(message_id),
            AmqpExtValue::Null,
            AmqpExtValue::Str("$cbs".into()),
            AmqpExtValue::Null,
            AmqpExtValue::Str(reply_to.into()),
        ])),
        MessageSection::ApplicationProperties(AmqpExtValue::Map(vec![
            (
                AmqpExtValue::Str("operation".into()),
                AmqpExtValue::Str("put-token".into()),
            ),
            (
                AmqpExtValue::Str("type".into()),
                AmqpExtValue::Str(token_type.into()),
            ),
            (
                AmqpExtValue::Str("name".into()),
                AmqpExtValue::Str(audience.into()),
            ),
        ])),
        MessageSection::AmqpValue(AmqpExtValue::Str(token.into())),
    ];
    let mut out = Vec::new();
    for s in &sections {
        out.extend_from_slice(&s.encode().map_err(|e| format!("cbs request: {e:?}"))?);
    }
    Ok(out)
}

/// The answer to a request: its `correlation-id` (the request's
/// message-id), `status-code` and `status-description`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Answer {
    pub(super) correlation_id: Option<u64>,
    pub(super) status_code: Option<i64>,
    pub(super) description: String,
}

/// Reads an answer of `$cbs`.
///
/// # Errors
/// A message the codec cannot decode.
pub(super) fn parse_answer(message: &[u8]) -> Result<Answer, String> {
    let mut answer = Answer {
        correlation_id: None,
        status_code: None,
        description: String::new(),
    };
    let mut rest = message;
    while !rest.is_empty() {
        let (section, used) =
            MessageSection::decode(rest).map_err(|e| format!("cbs answer: {e:?}"))?;
        rest = &rest[used.max(1).min(rest.len())..];
        match section {
            MessageSection::Properties(AmqpExtValue::List(fields)) => {
                answer.correlation_id = fields
                    .get(5)
                    .and_then(integer)
                    .and_then(|v| u64::try_from(v).ok());
            }
            MessageSection::ApplicationProperties(AmqpExtValue::Map(entries)) => {
                for (k, v) in entries {
                    let key = match &k {
                        AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) => s.as_str(),
                        _ => continue,
                    };
                    match key {
                        "status-code" | "statusCode" => answer.status_code = integer(&v),
                        "status-description" | "statusDescription" => {
                            if let AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) = v {
                                answer.description = s;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(answer)
}

fn integer(v: &AmqpExtValue) -> Option<i64> {
    match v {
        AmqpExtValue::Int(n) => Some(i64::from(*n)),
        AmqpExtValue::Long(n) => Some(*n),
        AmqpExtValue::Uint(n) => Some(i64::from(*n)),
        AmqpExtValue::Ulong(n) => i64::try_from(*n).ok(),
        AmqpExtValue::Ushort(n) => Some(i64::from(*n)),
        AmqpExtValue::Short(n) => Some(i64::from(*n)),
        AmqpExtValue::Ubyte(n) => Some(i64::from(*n)),
        AmqpExtValue::Byte(n) => Some(i64::from(*n)),
        _ => None,
    }
}

fn unix_now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The audience of a token for the AMQP address `address` on `host`.
#[must_use]
pub(super) fn audience(host: &str, address: &str) -> String {
    format!("amqp://{host}/{}", address.trim_start_matches('/'))
}

/// The CBS session of one connection.
pub(super) struct Cbs {
    channel: u16,
    sender: LinkSpec,
    receiver: LinkSpec,
    reply_to: String,
    key_name: String,
    key: String,
    ttl: Duration,
    audiences: Vec<String>,
    remote_sender: Option<u32>,
    remote_receiver: Option<u32>,
    /// What the broker lets the daemon send on the sender (Part 2 §2.6.7).
    credit: u32,
    delivery_count: u32,
    /// The broker's drain flag for the sender (Part 2 §2.6.7).
    drain: bool,
    /// Answers received since the last grant.
    answers_since_grant: u32,
    received_count: u32,
    /// Requests in flight: message-id → audience.
    pending: BTreeMap<u64, String>,
    /// Audiences whose token is still to be sent in this round.
    to_send: Vec<String>,
    next_id: u64,
    next_tag: u64,
    /// The topic links may be attached: every audience had its token.
    authorized: bool,
    renew_at: Option<Instant>,
    expiry: u64,
    /// How long a round may take, from its start to its last answer.
    round_bound: Duration,
    /// When the running round must have ended; `None` between rounds.
    round_deadline: Option<Instant>,
}

impl Cbs {
    /// The CBS session on `channel`, its links on handles `handle` and
    /// `handle + 1`, one token per entry of `audiences`; a round that has
    /// not ended `round_bound` after its start ends the connection.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        container_id: &str,
        channel: u16,
        handle: u32,
        key_name: &str,
        key: &str,
        ttl: Duration,
        audiences: Vec<String>,
        round_bound: Duration,
    ) -> Self {
        let link = |role: &str, h: u32, sender: bool| LinkSpec {
            name: format!("{container_id}/$cbs/{role}"),
            handle: h,
            sender,
            address: "$cbs".into(),
            snd_settle_mode: 1,
            rcv_settle_mode: 0,
            durable: 0,
            properties: vec![],
            desired_capabilities: vec![],
            coordinator: None,
        };
        Self {
            channel,
            sender: link("sender", handle, true),
            receiver: link("receiver", handle + 1, false),
            reply_to: format!("{container_id}/$cbs/reply"),
            key_name: key_name.to_string(),
            key: key.to_string(),
            ttl,
            audiences,
            remote_sender: None,
            remote_receiver: None,
            credit: 0,
            delivery_count: INITIAL_DELIVERY_COUNT,
            drain: false,
            answers_since_grant: 0,
            received_count: 0,
            pending: BTreeMap::new(),
            to_send: Vec::new(),
            next_id: 1,
            next_tag: 0,
            authorized: false,
            renew_at: None,
            expiry: 0,
            round_bound,
            round_deadline: None,
        }
    }

    /// Begins the session on a new connection; the state of the last one
    /// is dropped.
    ///
    /// # Errors
    /// The I/O error of the begin.
    pub(super) fn start(&mut self, writer: &mut BrokerWriter) -> Result<(), String> {
        self.remote_sender = None;
        self.remote_receiver = None;
        self.credit = 0;
        self.delivery_count = INITIAL_DELIVERY_COUNT;
        self.drain = false;
        self.answers_since_grant = 0;
        self.received_count = 0;
        self.pending.clear();
        self.authorized = false;
        self.renew_at = None;
        self.to_send = self.audiences.clone();
        self.round_deadline = Some(Instant::now() + self.round_bound);
        writer
            .begin(self.channel)
            .map_err(|e| format!("cbs begin: {e}"))
    }

    /// Detaches the two links at a shutdown (§9.2), before the session
    /// ends with the connection's close.
    pub(super) fn detach(&self, writer: &mut BrokerWriter) {
        if self.remote_sender.is_some() {
            let _ = writer.detach(self.channel, self.sender.handle, None);
        }
        if self.remote_receiver.is_some() {
            let _ = writer.detach(self.channel, self.receiver.handle, None);
        }
    }

    /// Whether `inbound` belongs to the CBS session.
    #[must_use]
    pub(super) fn owns(&self, inbound: &Inbound) -> bool {
        let channel = match inbound {
            Inbound::Begin { channel, .. }
            | Inbound::Attach { channel, .. }
            | Inbound::Flow { channel, .. }
            | Inbound::Transfer { channel, .. }
            | Inbound::Disposition { channel, .. }
            | Inbound::Detach { channel, .. }
            | Inbound::End { channel, .. } => *channel,
            Inbound::Close(_) | Inbound::Heartbeat => return false,
        };
        channel == self.channel
    }

    /// One frame of the CBS session. Returns `true` when the last token of
    /// the first round was accepted: the topic links may be attached now.
    ///
    /// # Errors
    /// A refused link or token, the broker's detach or end of the session,
    /// an I/O error.
    pub(super) fn on_inbound(
        &mut self,
        writer: &mut BrokerWriter,
        inbound: Inbound,
    ) -> Result<bool, String> {
        match inbound {
            Inbound::Begin {
                channel,
                incoming_window,
                ..
            } => {
                writer.session_begun(channel, incoming_window);
                writer
                    .attach(self.channel, &self.sender)
                    .and_then(|()| {
                        writer.attach_receiver_to(self.channel, &self.receiver, &self.reply_to)
                    })
                    .map_err(|e| format!("cbs attach: {e}"))?;
            }
            Inbound::Attach {
                name,
                remote_handle,
                has_node,
                ..
            } => {
                if !has_node {
                    return Err(format!("cbs: the broker refused the link {name} to $cbs"));
                }
                if name == self.sender.name {
                    self.remote_sender = Some(remote_handle);
                } else if name == self.receiver.name {
                    self.remote_receiver = Some(remote_handle);
                    writer
                        .flow(self.channel, self.receiver.handle, 0, ANSWER_CREDIT)
                        .map_err(|e| format!("cbs flow: {e}"))?;
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
                writer.apply_session_flow(channel, next_incoming_id, incoming_window);
                if remote_handle.is_none() {
                    if echo {
                        // Part 2 §2.7.4: the session's state alone.
                        writer
                            .session_flow(channel)
                            .map_err(|e| format!("cbs flow: {e}"))?;
                    }
                } else if remote_handle == self.remote_sender {
                    // Part 2 §2.6.7: delivery-count of the receiver plus its
                    // credit, less what the sender counted.
                    self.credit = sender_credit(
                        delivery_count,
                        link_credit.unwrap_or(0),
                        self.delivery_count,
                        INITIAL_DELIVERY_COUNT,
                    );
                    self.drain = drain;
                    self.send_due(writer)?;
                    self.answer_sender_flow(writer, echo)?;
                    return Ok(false);
                } else if remote_handle == self.remote_receiver {
                    self.on_answer_link_flow(writer, delivery_count, echo)?;
                }
            }
            Inbound::Transfer {
                remote_handle,
                delivery_id,
                settled,
                message,
                ..
            } => {
                if Some(remote_handle) != self.remote_receiver {
                    return Err(format!("cbs: a transfer on unknown handle {remote_handle}"));
                }
                if !settled {
                    writer
                        .disposition(self.channel, delivery_id, delivery_id, &Outcome::Accepted)
                        .map_err(|e| format!("cbs disposition: {e}"))?;
                }
                self.received_count = self.received_count.wrapping_add(1);
                self.answers_since_grant += 1;
                if self.answers_since_grant >= ANSWER_CREDIT / 2 {
                    writer
                        .flow(
                            self.channel,
                            self.receiver.handle,
                            self.received_count,
                            ANSWER_CREDIT,
                        )
                        .map_err(|e| format!("cbs flow: {e}"))?;
                    self.answers_since_grant = 0;
                }
                let first_round_done = self.on_answer(&parse_answer(&message)?)?;
                if first_round_done {
                    self.send_due(writer)?;
                    return Ok(true);
                }
            }
            Inbound::Disposition {
                outcome: Outcome::Rejected(e),
                ..
            } => {
                return Err(format!(
                    "cbs: the broker rejected a put-token: {}",
                    e.map_or_else(String::new, |e| e.condition)
                ));
            }
            Inbound::Disposition { .. } => {}
            Inbound::Detach { error, .. } | Inbound::End { error, .. } => {
                return Err(format!(
                    "cbs: the broker ended the $cbs link: {}",
                    error.map_or_else(|| "no condition".to_string(), |e| e.condition)
                ));
            }
            Inbound::Close(_) | Inbound::Heartbeat => {}
        }
        self.send_due(writer)?;
        Ok(false)
    }

    /// An answer: its request is done; returns `true` when it was the last
    /// of the first round.
    fn on_answer(&mut self, a: &Answer) -> Result<bool, String> {
        let Some(id) = a.correlation_id else {
            return Err("cbs: an answer without correlation-id".into());
        };
        let Some(audience) = self.pending.remove(&id) else {
            return Err(format!("cbs: an answer to no request ({id})"));
        };
        match a.status_code {
            Some(200 | 202) => {}
            other => {
                return Err(format!(
                    "cbs put-token for {audience} refused: {} {}",
                    other.map_or_else(|| "no status-code".to_string(), |c| c.to_string()),
                    a.description
                ));
            }
        }
        if self.pending.is_empty() && self.to_send.is_empty() {
            let first = !self.authorized;
            self.authorized = true;
            self.round_deadline = None;
            self.renew_at = Some(Instant::now() + self.ttl * 4 / 5);
            emit(
                if first {
                    "cbs_authorized"
                } else {
                    "cbs_token_renewed"
                },
                &[
                    ("tokens", Field::Number(self.audiences.len() as u64)),
                    ("expires", Field::Number(self.expiry)),
                ],
            );
            return Ok(first);
        }
        Ok(false)
    }

    /// Starts a new round when four fifths of the tokens' lifetime have
    /// passed; ends the connection when the running round, the first or a
    /// renewal, has not ended within its bound (§7.2).
    ///
    /// # Errors
    /// A round past its bound, an I/O error.
    pub(super) fn renew_if_due(
        &mut self,
        writer: &mut BrokerWriter,
        now: Instant,
    ) -> Result<(), String> {
        if self.round_deadline.is_some_and(|at| now >= at) {
            return Err(format!(
                "cbs: put-token round not answered within {} ms ({} of {} tokens answered)",
                self.round_bound.as_millis(),
                self.audiences
                    .len()
                    .saturating_sub(self.pending.len() + self.to_send.len()),
                self.audiences.len()
            ));
        }
        if self.renew_at.is_some_and(|at| now >= at) && self.pending.is_empty() {
            self.renew_at = None;
            self.to_send = self.audiences.clone();
            self.round_deadline = Some(now + self.round_bound);
            self.send_due(writer)?;
        }
        Ok(())
    }

    /// The flow state of the put-token sender the broker asked for (Part
    /// 2 §2.6.7, §2.7.4): with `drain` and no token waiting, the
    /// delivery-count is advanced by the credit left, which consumes it;
    /// with `echo` the state is sent as it is; one flow answers both.
    /// `available` is the number of tokens waiting for credit.
    fn answer_sender_flow(&mut self, writer: &mut BrokerWriter, echo: bool) -> Result<(), String> {
        let available = u32::try_from(self.to_send.len()).unwrap_or(u32::MAX);
        let drained = self.drain && self.credit > 0 && available == 0;
        if drained {
            self.delivery_count = self.delivery_count.wrapping_add(self.credit);
            self.credit = 0;
        }
        if drained || echo {
            writer
                .sender_flow(
                    self.channel,
                    self.sender.handle,
                    self.delivery_count,
                    self.credit,
                    available,
                    self.drain,
                )
                .map_err(|e| format!("cbs flow: {e}"))?;
        }
        Ok(())
    }

    /// The broker's flow state as the sender of the answer link (Part 2
    /// §2.6.7, §2.7.4): its delivery-count is the one the daemon counts
    /// from; a count that moved without answers (a drained link) consumed
    /// credit, which is granted again once half is used, as after answers.
    /// With `echo` the broker asks for the link's state: the flow that
    /// grants credit carries it, else a flow with the credit still granted.
    fn on_answer_link_flow(
        &mut self,
        writer: &mut BrokerWriter,
        delivery_count: Option<u32>,
        echo: bool,
    ) -> Result<(), String> {
        if let Some(count) = delivery_count {
            let moved = count.wrapping_sub(self.received_count);
            if moved < 0x8000_0000 {
                self.answers_since_grant = self
                    .answers_since_grant
                    .saturating_add(moved)
                    .min(ANSWER_CREDIT);
                self.received_count = count;
            }
        }
        if self.answers_since_grant >= ANSWER_CREDIT / 2 {
            writer
                .flow(
                    self.channel,
                    self.receiver.handle,
                    self.received_count,
                    ANSWER_CREDIT,
                )
                .map_err(|e| format!("cbs flow: {e}"))?;
            self.answers_since_grant = 0;
        } else if echo {
            writer
                .flow(
                    self.channel,
                    self.receiver.handle,
                    self.received_count,
                    ANSWER_CREDIT - self.answers_since_grant,
                )
                .map_err(|e| format!("cbs flow: {e}"))?;
        }
        Ok(())
    }

    /// Sends the tokens of this round while the broker grants credit.
    fn send_due(&mut self, writer: &mut BrokerWriter) -> Result<(), String> {
        if self.remote_sender.is_none() || self.remote_receiver.is_none() {
            return Ok(());
        }
        if !self.to_send.is_empty() && self.pending.is_empty() {
            self.expiry = unix_now_s() + self.ttl.as_secs();
        }
        while self.credit > 0 && !self.to_send.is_empty() {
            let audience = self.to_send.remove(0);
            let token = sas_token(&audience, &self.key_name, &self.key, self.expiry);
            let id = self.next_id;
            self.next_id += 1;
            let message = put_token_message(id, &self.reply_to, &audience, SAS_TOKEN_TYPE, &token)?;
            let tag = self.next_tag.to_be_bytes();
            self.next_tag += 1;
            writer
                .transfer(self.channel, self.sender.handle, &tag, &message, true)
                .map_err(|e| format!("cbs put-token: {e}"))?;
            self.credit -= 1;
            self.delivery_count = self.delivery_count.wrapping_add(1);
            self.pending.insert(id, audience);
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_amqp_bridge::performatives::descriptor;

    /// §7.2: the token matches the one Python's `hmac`, `hashlib`, `base64`
    /// and `urllib.parse` give for the algorithm Microsoft documents
    /// ("Generate SAS token", Python example: `quote_plus` of the URI, the
    /// key's UTF-8 octets, `quote` of the Base64 signature).
    #[test]
    fn amqpbridge_10_the_sas_token_matches_the_documented_algorithm() {
        assert_eq!(
            sas_token(
                "amqp://zerodds-test.servicebus.windows.net/plant/telemetry",
                "RootManageSharedAccessKey",
                "rG5mT0x+Q2v8uJ1kW4yZ9aB7cD3eF6hI0jK2lM5nO8p=",
                1_767_225_600,
            ),
            "SharedAccessSignature sr=amqp%3A%2F%2Fzerodds-test.servicebus.windows.net%2Fplant%2Ftelemetry&sig=tQkzhTLSjU%2B1HlaAEUhgWRi7PNZN8yR%2BjeTM8WY1sEQ%3D&se=1767225600&skn=RootManageSharedAccessKey"
        );
    }

    /// The request carries what the CBS draft names: `operation`
    /// `put-token`, the token `type`, the audience as `name`, the
    /// `reply-to`, the token as an `amqp-value` string.
    #[test]
    fn amqpbridge_10_the_put_token_request_names_operation_type_audience_and_reply_to() {
        let m =
            put_token_message(7, "cid/$cbs/reply", "amqp://h/q", SAS_TOKEN_TYPE, "tok").unwrap();
        let mut rest = &m[..];
        let mut seen = Vec::new();
        while !rest.is_empty() {
            let (s, n) = MessageSection::decode(rest).unwrap();
            rest = &rest[n..];
            seen.push(s);
        }
        let MessageSection::Properties(AmqpExtValue::List(p)) = &seen[0] else {
            panic!("{seen:?}")
        };
        assert_eq!(p[0], AmqpExtValue::Ulong(7));
        assert_eq!(p[2], AmqpExtValue::Str("$cbs".into()));
        assert_eq!(p[4], AmqpExtValue::Str("cid/$cbs/reply".into()));
        let MessageSection::ApplicationProperties(AmqpExtValue::Map(a)) = &seen[1] else {
            panic!("{seen:?}")
        };
        let get = |k: &str| {
            a.iter()
                .find(|(key, _)| key == &AmqpExtValue::Str(k.into()))
                .map(|(_, v)| v.clone())
        };
        assert_eq!(
            get("operation"),
            Some(AmqpExtValue::Str("put-token".into()))
        );
        assert_eq!(get("type"), Some(AmqpExtValue::Str(SAS_TOKEN_TYPE.into())));
        assert_eq!(get("name"), Some(AmqpExtValue::Str("amqp://h/q".into())));
        assert_eq!(
            seen[2],
            MessageSection::AmqpValue(AmqpExtValue::Str("tok".into()))
        );
    }

    /// The answer's correlation-id, `status-code` (an int, as the broker
    /// sends it) and `status-description`.
    #[test]
    fn amqpbridge_10_an_answer_gives_its_correlation_id_and_status() {
        let mut m = MessageSection::Properties(AmqpExtValue::List(vec![
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Ulong(9),
        ]))
        .encode()
        .unwrap();
        m.extend_from_slice(
            &MessageSection::ApplicationProperties(AmqpExtValue::Map(vec![
                (
                    AmqpExtValue::Str("status-code".into()),
                    AmqpExtValue::Int(401),
                ),
                (
                    AmqpExtValue::Str("status-description".into()),
                    AmqpExtValue::Str("InvalidSignature".into()),
                ),
            ]))
            .encode()
            .unwrap(),
        );
        assert_eq!(
            parse_answer(&m).unwrap(),
            Answer {
                correlation_id: Some(9),
                status_code: Some(401),
                description: "InvalidSignature".into(),
            }
        );
    }

    /// The audience of an address names the host and the address without
    /// its leading slash.
    #[test]
    fn amqpbridge_10_the_audience_of_an_address() {
        assert_eq!(
            audience("ns.example", "plant/telemetry"),
            "amqp://ns.example/plant/telemetry"
        );
        assert_eq!(
            audience("ns.example", "/queues/q"),
            "amqp://ns.example/queues/q"
        );
    }

    /// Unreserved characters stay, the rest is `%XX`; the signature keeps
    /// `/` as Python's `quote` does.
    #[test]
    fn amqpbridge_10_percent_encoding_as_quote_plus_and_quote() {
        assert_eq!(percent_encode("a b/c:~", false), "a%20b%2Fc%3A~");
        assert_eq!(percent_encode("ab+/=", true), "ab%2B/%3D");
    }

    fn uint(v: &AmqpExtValue) -> u32 {
        match v {
            AmqpExtValue::Uint(n) => *n,
            other => panic!("not a uint: {other:?}"),
        }
    }

    /// A CBS session on channel 5 (sender handle 10, receiver handle 11)
    /// whose two links the broker attached with its handles 50 and 51, no
    /// token to send; the session begun on `writer`.
    fn attached_cbs(writer: &mut BrokerWriter) -> Cbs {
        let mut c = Cbs::new(
            "cid",
            5,
            10,
            "key-name",
            "key",
            Duration::from_secs(3600),
            vec![audience("ns.example", "q")],
            Duration::from_secs(10),
        );
        c.remote_sender = Some(50);
        c.remote_receiver = Some(51);
        writer.begin(5).unwrap();
        c
    }

    fn broker_flow(
        remote_handle: Option<u32>,
        delivery_count: u32,
        link_credit: u32,
        drain: bool,
        echo: bool,
    ) -> Inbound {
        Inbound::Flow {
            channel: 5,
            next_incoming_id: Some(0),
            incoming_window: 100,
            remote_handle,
            delivery_count: remote_handle.map(|_| delivery_count),
            link_credit: remote_handle.map(|_| link_credit),
            echo,
            drain,
        }
    }

    /// AMQP 1.0 Part 2 §2.6.7, §2.7.4: the put-token sender asked to drain
    /// with no token to send consumes its credit and sends its flow state;
    /// a flow with `echo` is answered with the state as it is.
    #[test]
    fn amqp10_2_6_7_the_cbs_sender_drains_and_answers_an_echo() {
        let (mut w, mut written) = crate::broker::tests::bare_writer_and_written();
        let mut c = attached_cbs(&mut w);
        written.next_on();
        c.on_inbound(&mut w, broker_flow(Some(50), 0, 4, true, false))
            .unwrap();
        let (channel, code, f) = written.try_next_on().expect("the drained flow");
        assert_eq!((channel, code), (5, descriptor::FLOW));
        assert_eq!(
            (uint(&f[4]), uint(&f[5]), uint(&f[6]), uint(&f[7])),
            (10, 4, 0, 0),
            "handle, delivery-count, link-credit, available"
        );
        assert_eq!(f[8], AmqpExtValue::Boolean(true), "drain");
        assert_eq!((c.credit, c.delivery_count), (0, 4));
        c.on_inbound(&mut w, broker_flow(Some(50), 4, 2, false, true))
            .unwrap();
        let (_, code, f) = written.try_next_on().expect("the echoed flow");
        assert_eq!(code, descriptor::FLOW);
        assert_eq!((uint(&f[5]), uint(&f[6])), (4, 2));
        assert_eq!(f[8], AmqpExtValue::Boolean(false), "drain");
        assert!(written.nothing_more());
    }

    /// AMQP 1.0 Part 2 §2.6.7, §2.7.4: the answer receiver answers an echo
    /// with its state, takes the broker's delivery-count and grants its
    /// credit again once half of it is consumed; a session echo is
    /// answered with the session's state.
    #[test]
    fn amqp10_2_6_7_the_cbs_receiver_answers_an_echo_and_follows_the_brokers_count() {
        let (mut w, mut written) = crate::broker::tests::bare_writer_and_written();
        let mut c = attached_cbs(&mut w);
        written.next_on();
        c.on_inbound(&mut w, broker_flow(Some(51), 0, ANSWER_CREDIT, false, true))
            .unwrap();
        let (channel, code, f) = written.try_next_on().expect("the echoed flow");
        assert_eq!((channel, code), (5, descriptor::FLOW));
        assert_eq!(
            (uint(&f[4]), uint(&f[5]), uint(&f[6])),
            (11, 0, ANSWER_CREDIT),
            "handle, delivery-count, link-credit"
        );
        c.on_inbound(&mut w, broker_flow(Some(51), ANSWER_CREDIT, 0, true, false))
            .unwrap();
        let (_, code, f) = written.try_next_on().expect("the credit granted again");
        assert_eq!(code, descriptor::FLOW);
        assert_eq!(
            (uint(&f[4]), uint(&f[5]), uint(&f[6])),
            (11, ANSWER_CREDIT, ANSWER_CREDIT)
        );
        c.on_inbound(&mut w, broker_flow(None, 0, 0, false, true))
            .unwrap();
        let (_, code, f) = written.try_next_on().expect("the session's flow");
        assert_eq!(code, descriptor::FLOW);
        assert_eq!(f.len(), 4, "session fields only: {f:?}");
        assert!(written.nothing_more());
    }
}
