// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The object key of a mapping (spec §4.5).

#[allow(unused_imports)]
use super::prelude::*;

/// The canonical topic pair of §4.5: `request_topic->reply_topic` for a
/// mapping that receives GIOP, the notify topic for one that only sends it.
#[must_use]
pub(crate) fn canonical_topics(m: &Mapping) -> String {
    if m.receives_giop() {
        format!("{}->{}", m.request_topic.dds_name, m.reply_topic.dds_name)
    } else {
        m.notify_topic.dds_name.clone()
    }
}

/// §4.5: `SHA-256(repo_id + "\0" + canonical topics)[..16]`.
#[must_use]
pub fn object_key(m: &Mapping) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(m.repo_id.as_bytes());
    h.update(b"\0");
    h.update(canonical_topics(m).as_bytes());
    h.finalize()[..16].to_vec()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::config::tests::load;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// The digest of `repo_id\0request->reply` as `shasum -a 256` gives it
    /// for the same octets (`printf 'IDL:zerodds/MarketData/Quote:1.0\0…'`).
    #[test]
    fn corbabridge_4_5_the_key_of_a_request_reply_mapping_is_the_sha_256_prefix() {
        let c = load(
            "mappings:\n  - repo_id: \"IDL:zerodds/MarketData/Quote:1.0\"\n    operation: request_quote\n",
        )
        .unwrap();
        assert_eq!(
            hex(&object_key(&c.mappings[0])),
            "c2dca33f48a8e515219a60f87a59d185"
        );
    }

    /// The notify form hashes the notify topic alone
    /// (`printf 'IDL:zerodds/Trading/OrderEvents:1.0\0Trading::OrderEvent' | shasum -a 256`).
    #[test]
    fn corbabridge_4_5_the_key_of_a_notify_mapping_is_of_the_notify_topic() {
        let c = load(
            "mappings:\n  - repo_id: \"IDL:zerodds/Trading/OrderEvents:1.0\"\n    operation: on_order_event\n    direction: dds_to_corba\n    notify_topic:\n      dds_name: \"Trading::OrderEvent\"\n    targets: [\"corbaloc:iiop:1.2@127.0.0.1:2809/L\"]\n",
        )
        .unwrap();
        assert_eq!(canonical_topics(&c.mappings[0]), "Trading::OrderEvent");
        assert_eq!(
            hex(&object_key(&c.mappings[0])),
            "2ead2574523b416ec900395e311a405a"
        );
    }

    #[test]
    fn corbabridge_4_5_a_key_is_16_octets() {
        let c = load("mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n").unwrap();
        assert_eq!(object_key(&c.mappings[0]).len(), 16);
    }

    /// A bidirectional mapping receives GIOP, so its key is of the
    /// request-reply pair.
    #[test]
    fn corbabridge_4_5_a_bidir_mapping_takes_the_request_reply_form() {
        let c = load(
            "mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n    direction: bidir\n    targets: [\"corbaloc:iiop:1.2@127.0.0.1:2809/L\"]\n",
        )
        .unwrap();
        assert_eq!(
            canonical_topics(&c.mappings[0]),
            "Y::op::Request->Y::op::Reply"
        );
    }

    /// Each part of the input changes the key: the repo_id, the request
    /// topic, the reply topic.
    #[test]
    fn corbabridge_4_5_repo_id_and_topics_name_the_key() {
        let key = |repo: &str, req: &str, rep: &str| {
            let c = load(&format!(
                "mappings:\n  - repo_id: \"{repo}\"\n    operation: op\n    request_topic:\n      dds_name: \"{req}\"\n    reply_topic:\n      dds_name: \"{rep}\"\n"
            ))
            .unwrap();
            object_key(&c.mappings[0])
        };
        let base = key("IDL:x/Y:1.0", "A", "B");
        assert_eq!(base, key("IDL:x/Y:1.0", "A", "B"), "stable");
        assert_ne!(base, key("IDL:x/Z:1.0", "A", "B"));
        assert_ne!(base, key("IDL:x/Y:1.0", "A2", "B"));
        assert_ne!(base, key("IDL:x/Y:1.0", "A", "B2"));
    }

    /// Two operations of one interface on the same topics would be one
    /// object; the configuration refuses them.
    #[test]
    fn corbabridge_4_5_two_mappings_with_one_key_are_refused() {
        let e = load(
            "mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: a\n    request_topic:\n      dds_name: R\n    reply_topic:\n      dds_name: S\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: b\n    request_topic:\n      dds_name: R\n    reply_topic:\n      dds_name: S\n",
        )
        .expect_err("same key");
        assert!(e.contains("same object key"), "{e}");
    }
}
