// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The nodes links attach to — OASIS AMQP 1.0 Part 3 §3.3 and §3.5.
//!
//! A node here is a queue by its address: a message a sending partner
//! transfers to it is held until a link whose source it is takes it
//! (distribution mode `move`, §3.5.2). The outcomes of §3.4 act on the
//! queue: `accepted` and `rejected` remove the message, `released` and
//! `modified` put it back; `modified` with `delivery-failed` counts the
//! attempt in the header (§3.2.1 `delivery-count`), with
//! `undeliverable-here` the message is not handed to that link again.
//! Nodes are shared by all connections of one listener.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Mutex, MutexGuard};

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_amqp_endpoint::NodeLimits;

/// A message held by a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// `message-format` of the transfer that brought it.
    pub format: u32,
    /// The encoded message (the payload of its transfer frames).
    pub payload: Vec<u8>,
    /// Links (by their key, see [`Nodes::take`]) that modified it with
    /// `undeliverable-here`.
    pub undeliverable: BTreeSet<String>,
}

impl Message {
    /// A message with `payload`.
    #[must_use]
    pub fn new(format: u32, payload: Vec<u8>) -> Self {
        Self {
            format,
            payload,
            undeliverable: BTreeSet::new(),
        }
    }

    /// Counts a failed delivery attempt: `header.delivery-count` plus one
    /// (Part 3 §3.2.1, §3.4.5 `delivery-failed`). A message without a
    /// header gets one. A payload that is no AMQP message (another
    /// `message-format`) is left as it is: there is no header to count in.
    pub fn count_failed_delivery(&mut self) {
        if self.format != 0 {
            return;
        }
        let (mut fields, rest) = match MessageSection::decode(&self.payload) {
            Ok((MessageSection::Header(AmqpExtValue::List(fields)), used)) => {
                (fields, self.payload[used..].to_vec())
            }
            Ok(_) => (Vec::new(), self.payload.clone()),
            Err(_) => return,
        };
        while fields.len() < 5 {
            fields.push(AmqpExtValue::Null);
        }
        let count = match &fields[4] {
            AmqpExtValue::Uint(n) => *n,
            _ => 0,
        };
        fields[4] = AmqpExtValue::Uint(count.saturating_add(1));
        if let Ok(mut header) = MessageSection::Header(AmqpExtValue::List(fields)).encode() {
            header.extend_from_slice(&rest);
            self.payload = header;
        }
    }
}

/// One queue and the octets of the payloads in it.
#[derive(Default)]
struct Queue {
    messages: VecDeque<Message>,
    octets: u64,
    /// Declared by the configuration or a test, not created by an attach.
    declared: bool,
}

#[derive(Default)]
struct Inner {
    queues: BTreeMap<String, Queue>,
    next_dynamic: u64,
}

impl Inner {
    /// Nodes an attach created.
    fn created(&self) -> usize {
        self.queues.values().filter(|q| !q.declared).count()
    }
}

/// What [`Nodes::resolve`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolved {
    /// The node exists, or was created now.
    Node,
    /// No node at the address and none is created for an attach.
    NotFound,
    /// No node at the address, and the listener holds as many created
    /// nodes as [`NodeLimits::max_nodes`] admits.
    Full,
}

/// What [`Nodes::put`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Put {
    /// The node holds the message.
    Stored,
    /// The node does not exist.
    NoNode,
    /// The node holds as many messages or octets as its bound admits.
    Full,
}

/// The nodes of a listener.
pub struct Nodes {
    inner: Mutex<Inner>,
    auto_create: bool,
    limits: NodeLimits,
}

impl core::fmt::Debug for Nodes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Nodes")
            .field("auto_create", &self.auto_create)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl Nodes {
    /// No nodes yet, with the default bounds. With `auto_create` an attach
    /// to an address without a node creates the node; without it such an
    /// attach is refused with `amqp:not-found`.
    #[must_use]
    pub fn new(auto_create: bool) -> Self {
        Self::with_limits(auto_create, NodeLimits::default())
    }

    /// No nodes yet, bounded by `limits`.
    #[must_use]
    pub fn with_limits(auto_create: bool, limits: NodeLimits) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            auto_create,
            limits,
        }
    }

    /// The bounds of these nodes.
    #[must_use]
    pub fn limits(&self) -> NodeLimits {
        self.limits
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A poisoned lock means a connection thread panicked while it held
        // the queues; the queues themselves are whole after every
        // operation, so the next thread goes on with them.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Creates the node at `address` if there is none. A declared node
    /// does not count against [`NodeLimits::max_nodes`].
    pub fn declare(&self, address: &str) {
        self.lock()
            .queues
            .entry(address.to_string())
            .or_default()
            .declared = true;
    }

    /// The node an attach to `address` gets.
    pub fn resolve(&self, address: &str) -> Resolved {
        let mut inner = self.lock();
        if inner.queues.contains_key(address) {
            return Resolved::Node;
        }
        if !self.auto_create {
            return Resolved::NotFound;
        }
        if inner.created() >= self.limits.max_nodes as usize {
            return Resolved::Full;
        }
        inner.queues.insert(address.to_string(), Queue::default());
        Resolved::Node
    }

    /// Creates a node for a dynamic terminus (Part 3 §3.5.3 `dynamic`) and
    /// returns its address, which names the link and the container that
    /// asked ("The generated name of the address SHOULD include the link
    /// name and the container-id of the remote container"). `None` when
    /// the listener holds as many created nodes as its bound admits.
    pub fn create_dynamic(&self, container: &str, link: &str) -> Option<String> {
        let mut inner = self.lock();
        if inner.created() >= self.limits.max_nodes as usize {
            return None;
        }
        inner.next_dynamic += 1;
        let address = format!("dynamic/{container}/{link}/{}", inner.next_dynamic);
        inner.queues.insert(address.clone(), Queue::default());
        Some(address)
    }

    /// Deletes the node and what it holds.
    pub fn delete(&self, address: &str) {
        self.lock().queues.remove(address);
    }

    /// Whether `address` has a node.
    #[must_use]
    pub fn exists(&self, address: &str) -> bool {
        self.lock().queues.contains_key(address)
    }

    /// Messages the node holds; `None` without a node.
    #[must_use]
    pub fn depth(&self, address: &str) -> Option<usize> {
        self.lock().queues.get(address).map(|q| q.messages.len())
    }

    /// Messages of the node that `link` may get: the ones it did not
    /// modify with `undeliverable-here`. 0 without a node.
    #[must_use]
    pub fn available(&self, address: &str, link: &str) -> usize {
        self.lock().queues.get(address).map_or(0, |queue| {
            queue
                .messages
                .iter()
                .filter(|m| !m.undeliverable.contains(link))
                .count()
        })
    }

    /// Appends a message, unless the node is gone or holds as many
    /// messages or octets as [`NodeLimits`] admits.
    pub fn put(&self, address: &str, message: Message) -> Put {
        let mut inner = self.lock();
        let Some(queue) = inner.queues.get_mut(address) else {
            return Put::NoNode;
        };
        let octets = message.payload.len() as u64;
        if queue.messages.len() >= self.limits.max_messages as usize
            || queue.octets.saturating_add(octets) > self.limits.max_octets
        {
            return Put::Full;
        }
        queue.octets += octets;
        queue.messages.push_back(message);
        Put::Stored
    }

    /// Appends the messages of a transaction to their nodes, all or none
    /// (OASIS AMQP 1.0 Part 4 §4.4.1: a posted message becomes available
    /// "after the transaction has been successfully discharged"; §4.4.4:
    /// a resource that can no longer take it "MUST NOT allow the
    /// successful discharge").
    ///
    /// # Errors
    /// The first node that is gone or that would hold more than its
    /// bounds; no node took a message then.
    pub fn put_all(&self, messages: Vec<(String, Message)>) -> Result<(), String> {
        let mut inner = self.lock();
        let mut added: BTreeMap<&str, (usize, u64)> = BTreeMap::new();
        for (address, message) in &messages {
            let Some(queue) = inner.queues.get(address) else {
                return Err(format!("node {address} is gone"));
            };
            let (count, octets) = added.entry(address.as_str()).or_default();
            *count += 1;
            *octets = octets.saturating_add(message.payload.len() as u64);
            if queue.messages.len() + *count > self.limits.max_messages as usize
                || queue.octets.saturating_add(*octets) > self.limits.max_octets
            {
                return Err(format!(
                    "node {address} would hold more than {} messages or {} octets, its bounds",
                    self.limits.max_messages, self.limits.max_octets
                ));
            }
        }
        for (address, message) in messages {
            if let Some(queue) = inner.queues.get_mut(&address) {
                queue.octets += message.payload.len() as u64;
                queue.messages.push_back(message);
            }
        }
        Ok(())
    }

    /// Puts a message back at the head (a released delivery); it is
    /// dropped when the node is gone. A message that comes back was taken
    /// from the node; it is taken in again whatever the node holds by now.
    pub fn put_back(&self, address: &str, message: Message) {
        if let Some(queue) = self.lock().queues.get_mut(address) {
            queue.octets = queue.octets.saturating_add(message.payload.len() as u64);
            queue.messages.push_front(message);
        }
    }

    /// Takes the first message `link` may get: one it did not modify with
    /// `undeliverable-here`.
    pub fn take(&self, address: &str, link: &str) -> Option<Message> {
        let mut inner = self.lock();
        let queue = inner.queues.get_mut(address)?;
        let index = queue
            .messages
            .iter()
            .position(|m| !m.undeliverable.contains(link))?;
        let message = queue.messages.remove(index)?;
        queue.octets = queue.octets.saturating_sub(message.payload.len() as u64);
        Some(message)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Part 3 §3.3, §3.5.2: a queue hands each message to one link, in
    /// the order it arrived; a released one goes back to the head.
    #[test]
    fn amqp10_3_3_a_queue_holds_messages_in_order_until_a_link_takes_them() {
        let nodes = Nodes::new(false);
        assert_eq!(
            nodes.resolve("q"),
            Resolved::NotFound,
            "no node and no auto-create"
        );
        assert_eq!(nodes.put("q", Message::new(0, b"x".to_vec())), Put::NoNode);
        nodes.declare("q");
        assert_eq!(nodes.resolve("q"), Resolved::Node);
        for body in [b"1", b"2", b"3"] {
            assert_eq!(nodes.put("q", Message::new(0, body.to_vec())), Put::Stored);
        }
        assert_eq!(nodes.depth("q"), Some(3));
        let first = nodes.take("q", "a").expect("first");
        assert_eq!(first.payload, b"1");
        assert_eq!(nodes.take("q", "b").expect("second").payload, b"2");
        nodes.put_back("q", first);
        assert_eq!(nodes.take("q", "b").expect("released").payload, b"1");
        assert_eq!(nodes.take("q", "a").expect("third").payload, b"3");
        assert!(nodes.take("q", "a").is_none());
        assert_eq!(
            Nodes::new(true).resolve("made"),
            Resolved::Node,
            "auto-create"
        );
    }

    /// Part 2 §2.8.15 resource-limit-exceeded: a node takes no more
    /// messages and no more octets than its bounds, and an attach creates
    /// no more nodes than theirs. What is taken out makes room again; a
    /// released message goes back whatever the node holds.
    #[test]
    fn amqp10_2_8_15_nodes_are_bounded_in_number_messages_and_octets() {
        let limits = NodeLimits {
            max_nodes: 2,
            max_messages: 3,
            max_octets: 10,
        };
        let nodes = Nodes::with_limits(true, limits);
        assert_eq!(nodes.limits(), limits);
        // A declared node is outside the bound of created ones.
        nodes.declare("declared");
        assert_eq!(nodes.resolve("a"), Resolved::Node);
        let dynamic = nodes.create_dynamic("c", "l").expect("the second node");
        assert_eq!(nodes.resolve("b"), Resolved::Full);
        assert_eq!(nodes.create_dynamic("c", "m"), None);
        assert_eq!(nodes.resolve("a"), Resolved::Node, "an existing node");
        assert_eq!(nodes.resolve("declared"), Resolved::Node);
        // A deleted node makes room.
        nodes.delete(&dynamic);
        assert_eq!(nodes.resolve("b"), Resolved::Node);
        // Three messages.
        for _ in 0..3 {
            assert_eq!(nodes.put("a", Message::new(0, b"x".to_vec())), Put::Stored);
        }
        assert_eq!(nodes.put("a", Message::new(0, b"x".to_vec())), Put::Full);
        assert_eq!(nodes.depth("a"), Some(3));
        let taken = nodes.take("a", "k").expect("one");
        assert_eq!(nodes.put("a", Message::new(0, b"y".to_vec())), Put::Stored);
        nodes.put_back("a", taken);
        assert_eq!(nodes.depth("a"), Some(4), "a released message goes back");
        // Ten octets.
        assert_eq!(nodes.put("b", Message::new(0, vec![0; 6])), Put::Stored);
        assert_eq!(nodes.put("b", Message::new(0, vec![0; 5])), Put::Full);
        assert_eq!(nodes.put("b", Message::new(0, vec![0; 4])), Put::Stored);
        assert_eq!(nodes.put("b", Message::new(0, vec![0; 1])), Put::Full);
        assert!(nodes.take("b", "k").is_some());
        assert_eq!(nodes.put("b", Message::new(0, vec![0; 6])), Put::Stored);
    }

    /// Part 4 §4.4.1, §4.4.4: the messages of a transaction reach their
    /// nodes together or not at all: a node that is gone or one that
    /// would hold more than its bounds refuses the whole set.
    #[test]
    fn amqp10_4_4_1_the_messages_of_a_transaction_reach_their_nodes_all_or_none() {
        let nodes = Nodes::with_limits(
            true,
            NodeLimits {
                max_nodes: 8,
                max_messages: 3,
                max_octets: 10,
            },
        );
        nodes.declare("a");
        nodes.declare("b");
        let m = |body: &[u8]| Message::new(0, body.to_vec());
        assert_eq!(
            nodes.put_all(vec![
                ("a".into(), m(b"1")),
                ("b".into(), m(b"2")),
                ("a".into(), m(b"3")),
            ]),
            Ok(())
        );
        assert_eq!(nodes.depth("a"), Some(2));
        assert_eq!(nodes.depth("b"), Some(1));
        assert_eq!(nodes.take("a", "k").expect("in order").payload, b"1");
        assert_eq!(nodes.take("a", "k").expect("in order").payload, b"3");
        // One message too many for a: b takes none either.
        let refused = nodes
            .put_all(vec![
                ("b".into(), m(b"x")),
                ("a".into(), m(b"1")),
                ("a".into(), m(b"2")),
                ("a".into(), m(b"3")),
                ("a".into(), m(b"4")),
            ])
            .expect_err("four messages for a node of three");
        assert!(refused.contains("node a"), "{refused}");
        assert_eq!(nodes.depth("a"), Some(0));
        assert_eq!(nodes.depth("b"), Some(1));
        // Octets.
        let refused = nodes
            .put_all(vec![("a".into(), m(&[0; 6])), ("a".into(), m(&[0; 5]))])
            .expect_err("eleven octets for a node of ten");
        assert!(refused.contains("octets"), "{refused}");
        assert_eq!(nodes.depth("a"), Some(0));
        // A node that is gone.
        let refused = nodes
            .put_all(vec![("a".into(), m(b"1")), ("gone".into(), m(b"2"))])
            .expect_err("no node gone");
        assert!(refused.contains("gone"), "{refused}");
        assert_eq!(nodes.depth("a"), Some(0));
        assert_eq!(nodes.put_all(Vec::new()), Ok(()));
    }

    /// §3.4.5 `undeliverable-here`: the message is not handed to the link
    /// that said so, and stays for the others.
    #[test]
    fn amqp10_3_4_5_an_undeliverable_here_message_skips_that_link() {
        let nodes = Nodes::new(true);
        nodes.declare("q");
        let mut m = Message::new(0, b"m".to_vec());
        m.undeliverable.insert("a".to_string());
        nodes.put_back("q", m);
        assert_eq!(nodes.put("q", Message::new(0, b"n".to_vec())), Put::Stored);
        assert_eq!(nodes.available("q", "a"), 1);
        assert_eq!(nodes.available("q", "b"), 2);
        assert_eq!(nodes.available("gone", "b"), 0);
        assert_eq!(nodes.take("q", "a").expect("the other one").payload, b"n");
        assert!(nodes.take("q", "a").is_none());
        assert_eq!(nodes.take("q", "b").expect("for b").payload, b"m");
    }

    /// §3.4.5 `delivery-failed` with §3.2.1: the header's delivery-count
    /// goes up by one; a message without a header gets one in front.
    #[test]
    fn amqp10_3_4_5_a_failed_delivery_is_counted_in_the_header() {
        let data = MessageSection::Data(b"hi".to_vec()).encode().unwrap();
        let mut m = Message::new(0, data.clone());
        m.count_failed_delivery();
        let (header, used) = MessageSection::decode(&m.payload).unwrap();
        let MessageSection::Header(AmqpExtValue::List(fields)) = header else {
            panic!("a header in front");
        };
        assert_eq!(fields.len(), 5);
        assert_eq!(fields[4], AmqpExtValue::Uint(1));
        assert_eq!(&m.payload[used..], data.as_slice());
        // With a header that says durable and count 2.
        let mut with_header = MessageSection::Header(AmqpExtValue::List(vec![
            AmqpExtValue::Boolean(true),
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Uint(2),
        ]))
        .encode()
        .unwrap();
        with_header.extend_from_slice(&data);
        let mut m = Message::new(0, with_header);
        m.count_failed_delivery();
        let (header, used) = MessageSection::decode(&m.payload).unwrap();
        let MessageSection::Header(AmqpExtValue::List(fields)) = header else {
            panic!("a header");
        };
        assert_eq!(fields[0], AmqpExtValue::Boolean(true));
        assert_eq!(fields[4], AmqpExtValue::Uint(3));
        assert_eq!(&m.payload[used..], data.as_slice());
        // Another message format has no header to count in.
        let mut other = Message::new(7, b"\x01\x02".to_vec());
        other.count_failed_delivery();
        assert_eq!(other.payload, b"\x01\x02");
    }

    /// §3.5.3 `dynamic`: a created node has an address that names the
    /// link and the container, and is gone when deleted.
    #[test]
    fn amqp10_3_5_3_a_dynamic_node_is_named_after_container_and_link() {
        let nodes = Nodes::new(false);
        let a = nodes.create_dynamic("client-1", "reply").expect("a node");
        let b = nodes.create_dynamic("client-1", "reply").expect("a node");
        assert_ne!(a, b);
        assert!(a.contains("client-1") && a.contains("reply"), "{a}");
        assert!(nodes.exists(&a));
        nodes.delete(&a);
        assert!(!nodes.exists(&a));
        assert!(nodes.exists(&b));
    }
}
