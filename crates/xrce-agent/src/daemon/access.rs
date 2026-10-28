// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The AccessController of DDS-XRCE 1.0 §7.4.
//!
//! "The AccessController is responsible for making decisions regarding the
//! resources and operations a particular XRCE ProxyClient is allowed to
//! perform. It contains rules that associate a Client with privileges,
//! which determine which DDS domain an application executing on behalf of a
//! client may join, which DDS Topics the application can read and write, and
//! so on."
//!
//! The controller answers three questions:
//!
//! - Is the `ClientKey` of a `CREATE_CLIENT` configured in the agent
//!   (§7.8.1 "It must be configured both on the Client and in the Agent")
//!   and so authorized to connect (§7.8.2.1)? [`AccessController::client_authorized`]
//! - Do the client's properties meet what the agent requires
//!   (§7.8.2.1 "Check the Client properties")?
//!   [`AccessController::properties_accepted`]
//! - May this client perform this operation on this object
//!   (§7.8.3.1 to §7.8.5.1: `STATUS_ERR_DENIED`)? [`AccessController::permits`]
//!
//! A controller without rules restricts no operation. As soon as one rule
//! exists, an operation that no rule matches is denied; the first rule that
//! matches decides.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_xrce::handshake::Property;
use zerodds_xrce::header::CLIENT_KEY_LEN;
use zerodds_xrce::object_kind::ObjectKind;

/// A `ClientKey` as the four octets of the `MessageHeader`.
pub type ClientKeyBytes = [u8; CLIENT_KEY_LEN];

/// The operations of the XRCE ProxyClient, §7.8.3 to §7.8.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// `create`, §7.8.3.1 (the CREATE submessage).
    Create,
    /// `update`, §7.8.3.2.
    Update,
    /// `get_info`, §7.8.3.3 (the GET_INFO submessage).
    GetInfo,
    /// `delete`, §7.8.3.4 (the DELETE submessage).
    Delete,
    /// `write`, §7.8.4.1 (the WRITE_DATA submessage).
    WriteData,
    /// `read`, §7.8.5.1 (the READ_DATA submessage).
    ReadData,
}

impl Operation {
    /// The name a configuration file uses.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::GetInfo => "get_info",
            Self::Delete => "delete",
            Self::WriteData => "write_data",
            Self::ReadData => "read_data",
        }
    }

    /// The operation a configuration file name stands for.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::Create,
            Self::Update,
            Self::GetInfo,
            Self::Delete,
            Self::WriteData,
            Self::ReadData,
        ]
        .into_iter()
        .find(|op| op.name() == name)
    }
}

/// What a rule decides for the operations it matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// The operation is permitted.
    Allow,
    /// The operation is answered with `STATUS_ERR_DENIED`.
    Deny,
}

/// What an operation is performed on: the object's kind and the DDS
/// resources behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    /// The kind of the object.
    pub kind: ObjectKind,
    /// The DDS domain the object lives in, where it has one (participants,
    /// and everything below them).
    pub domain: Option<u16>,
    /// The DDS topic name, where the object has one (topics, writers,
    /// readers).
    pub topic: Option<String>,
}

impl Subject {
    /// A subject without a DDS domain or topic (QoS profiles, types,
    /// applications, the client itself).
    #[must_use]
    pub fn of_kind(kind: ObjectKind) -> Self {
        Self {
            kind,
            domain: None,
            topic: None,
        }
    }
}

/// One rule: a client (or every client), the operations, object kinds,
/// domains and topics it covers, and its [`Effect`]. An empty list covers
/// everything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// The client the rule is for; `None` is every client.
    pub client: Option<ClientKeyBytes>,
    /// Allow or deny.
    pub effect: Effect,
    /// The operations covered; empty is all.
    pub operations: Vec<Operation>,
    /// The object kinds covered; empty is all.
    pub kinds: Vec<ObjectKind>,
    /// The DDS domains covered; empty is all. A non-empty list covers
    /// only objects that live in one of these domains.
    pub domains: Vec<u16>,
    /// The topic names covered; empty is all. A name ending in `*` covers
    /// every topic that begins with the text before it. A non-empty list
    /// covers only objects that have a topic.
    pub topics: Vec<String>,
}

impl Rule {
    fn matches(&self, client: ClientKeyBytes, op: Operation, subject: &Subject) -> bool {
        if self.client.is_some_and(|c| c != client) {
            return false;
        }
        if !self.operations.is_empty() && !self.operations.contains(&op) {
            return false;
        }
        if !self.kinds.is_empty() && !self.kinds.contains(&subject.kind) {
            return false;
        }
        if !self.domains.is_empty() && !subject.domain.is_some_and(|d| self.domains.contains(&d)) {
            return false;
        }
        if !self.topics.is_empty() {
            let Some(topic) = subject.topic.as_deref() else {
                return false;
            };
            if !self.topics.iter().any(|p| topic_matches(p, topic)) {
                return false;
            }
        }
        true
    }
}

fn topic_matches(pattern: &str, topic: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => topic.starts_with(prefix),
        None => pattern == topic,
    }
}

/// The AccessController, §7.4.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessController {
    /// The ClientKeys configured in the agent; empty accepts every key.
    pub authorized_clients: Vec<ClientKeyBytes>,
    /// Properties every `CREATE_CLIENT` has to carry with exactly this
    /// name and value; empty requires none.
    pub required_properties: Vec<(String, String)>,
    /// The rules, in the order they are evaluated.
    pub rules: Vec<Rule>,
}

impl AccessController {
    /// Whether the client identified by `key` is authorized to connect
    /// (§7.8.2.1 "Check that the Client identified by the client_key is
    /// authorized to connect to the XRCE Agent").
    #[must_use]
    pub fn client_authorized(&self, key: ClientKeyBytes) -> bool {
        self.authorized_clients.is_empty() || self.authorized_clients.contains(&key)
    }

    /// Whether the properties of a `CREATE_CLIENT` meet what the agent
    /// requires (§7.8.2.1 "Check the Client properties, if present"). Other
    /// properties are vendor-specific and do not matter to the agent.
    #[must_use]
    pub fn properties_accepted(&self, properties: Option<&[Property]>) -> bool {
        self.required_properties.iter().all(|(name, value)| {
            properties
                .unwrap_or(&[])
                .iter()
                .any(|p| &p.name == name && &p.value == value)
        })
    }

    /// Whether the client may perform `op` on `subject`.
    #[must_use]
    pub fn permits(&self, client: ClientKeyBytes, op: Operation, subject: &Subject) -> bool {
        if self.rules.is_empty() {
            return true;
        }
        self.rules
            .iter()
            .find(|r| r.matches(client, op, subject))
            .is_some_and(|r| r.effect == Effect::Allow)
    }
}

/// Reads a `ClientKey` written as its four octets in hexadecimal, in wire
/// order (`0xAABBCCDD` is the octets `AA BB CC DD`). The key is not
/// interpreted (§7.8.1 "The ClientKey shall not be interpreted").
#[must_use]
pub fn parse_client_key(text: &str) -> Option<ClientKeyBytes> {
    let digits = text.trim();
    let digits = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
        .unwrap_or(digits);
    if digits.len() != CLIENT_KEY_LEN * 2 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut key = [0u8; CLIENT_KEY_LEN];
    for (i, octet) in key.iter_mut().enumerate() {
        *octet = u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(key)
}

/// The object kind a configuration file name stands for (`participant`,
/// `topic`, `publisher`, `subscriber`, `datawriter`, `datareader`, `type`,
/// `qosprofile`, `application`).
#[must_use]
pub fn kind_from_name(name: &str) -> Option<ObjectKind> {
    Some(match name {
        "participant" => ObjectKind::Participant,
        "topic" => ObjectKind::Topic,
        "publisher" => ObjectKind::Publisher,
        "subscriber" => ObjectKind::Subscriber,
        "datawriter" => ObjectKind::DataWriter,
        "datareader" => ObjectKind::DataReader,
        "type" => ObjectKind::Type,
        "qosprofile" => ObjectKind::QosProfile,
        "application" => ObjectKind::Application,
        _ => return None,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    const A: ClientKeyBytes = [0xAA, 0xBB, 0xCC, 0xDD];
    const B: ClientKeyBytes = [0x01, 0x02, 0x03, 0x04];

    fn writer_of(topic: &str, domain: u16) -> Subject {
        Subject {
            kind: ObjectKind::DataWriter,
            domain: Some(domain),
            topic: Some(topic.to_string()),
        }
    }

    fn rule(effect: Effect) -> Rule {
        Rule {
            client: None,
            effect,
            operations: vec![],
            kinds: vec![],
            domains: vec![],
            topics: vec![],
        }
    }

    #[test]
    fn a_controller_without_rules_restricts_nothing() {
        let c = AccessController::default();
        assert!(c.permits(A, Operation::Delete, &Subject::of_kind(ObjectKind::Topic)));
        assert!(c.client_authorized(B));
        assert!(c.properties_accepted(None));
    }

    #[test]
    fn the_first_matching_rule_decides_and_no_match_denies() {
        let c = AccessController {
            rules: vec![
                Rule {
                    client: Some(A),
                    operations: vec![Operation::WriteData],
                    topics: vec!["Chatter".to_string()],
                    ..rule(Effect::Allow)
                },
                Rule {
                    client: Some(A),
                    operations: vec![Operation::WriteData],
                    ..rule(Effect::Deny)
                },
            ],
            ..AccessController::default()
        };
        assert!(c.permits(A, Operation::WriteData, &writer_of("Chatter", 0)));
        assert!(!c.permits(A, Operation::WriteData, &writer_of("Secret", 0)));
        // No rule names client B at all.
        assert!(!c.permits(B, Operation::WriteData, &writer_of("Chatter", 0)));
        // No rule covers READ_DATA.
        assert!(!c.permits(A, Operation::ReadData, &writer_of("Chatter", 0)));
    }

    #[test]
    fn domains_kinds_and_topic_prefixes_narrow_a_rule() {
        let c = AccessController {
            rules: vec![Rule {
                domains: vec![3],
                kinds: vec![ObjectKind::Participant, ObjectKind::DataReader],
                topics: vec![],
                ..rule(Effect::Allow)
            }],
            ..AccessController::default()
        };
        let participant = |d| Subject {
            kind: ObjectKind::Participant,
            domain: Some(d),
            topic: None,
        };
        assert!(c.permits(A, Operation::Create, &participant(3)));
        assert!(!c.permits(A, Operation::Create, &participant(4)));
        assert!(!c.permits(A, Operation::Create, &writer_of("T", 3)));
        // An object without a domain is outside a domain rule.
        assert!(!c.permits(
            A,
            Operation::Create,
            &Subject::of_kind(ObjectKind::Participant)
        ));

        let topics = AccessController {
            rules: vec![Rule {
                topics: vec!["sensor/*".to_string()],
                ..rule(Effect::Allow)
            }],
            ..AccessController::default()
        };
        assert!(topics.permits(A, Operation::ReadData, &writer_of("sensor/imu", 0)));
        assert!(!topics.permits(A, Operation::ReadData, &writer_of("actuator/x", 0)));
        assert!(!topics.permits(A, Operation::ReadData, &Subject::of_kind(ObjectKind::Type)));
    }

    #[test]
    fn only_configured_client_keys_are_authorized() {
        let c = AccessController {
            authorized_clients: vec![A],
            ..AccessController::default()
        };
        assert!(c.client_authorized(A));
        assert!(!c.client_authorized(B));
    }

    #[test]
    fn required_properties_must_all_be_present_with_their_value() {
        let c = AccessController {
            required_properties: vec![("auth".to_string(), "token".to_string())],
            ..AccessController::default()
        };
        let prop = |n: &str, v: &str| Property {
            name: n.to_string(),
            value: v.to_string(),
        };
        assert!(!c.properties_accepted(None));
        assert!(!c.properties_accepted(Some(&[prop("auth", "other")])));
        assert!(!c.properties_accepted(Some(&[prop("x", "token")])));
        assert!(c.properties_accepted(Some(&[prop("x", "y"), prop("auth", "token")])));
    }

    #[test]
    fn a_client_key_is_read_in_wire_order() {
        assert_eq!(parse_client_key("0xAABBCCDD"), Some(A));
        assert_eq!(parse_client_key("01020304"), Some(B));
        assert_eq!(parse_client_key("0xAABBCC"), None);
        assert_eq!(parse_client_key("0xAABBCCDDEE"), None);
        assert_eq!(parse_client_key("0xGGBBCCDD"), None);
    }

    #[test]
    fn operation_and_kind_names_round_trip() {
        for op in [
            Operation::Create,
            Operation::Update,
            Operation::GetInfo,
            Operation::Delete,
            Operation::WriteData,
            Operation::ReadData,
        ] {
            assert_eq!(Operation::from_name(op.name()), Some(op));
        }
        assert_eq!(Operation::from_name("write"), None);
        assert_eq!(kind_from_name("datawriter"), Some(ObjectKind::DataWriter));
        assert_eq!(kind_from_name("agent"), None);
    }
}
