// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The transaction controller of this crate (`txn::TxnClient`) against
//! Apache ActiveMQ Artemis as the transactional resource of OASIS AMQP 1.0
//! Part 4: an Artemis broker of each test alone. What a commit and a
//! rollback did is read back from Artemis's queue.
//!
//! The tests need the Artemis release archive and a JDK; they run in the
//! integration pillar's job `integration-bridges-vendor-amqp`, where
//! `tests/integration/provision.sh artemis` installs them and a missing
//! program fails them:
//!
//! ```bash
//! cargo nextest run -p zerodds-amqp-endpoint --test artemis_txn_e2e --run-ignored all
//! ```

#![cfg(all(feature = "std", unix))]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

#[path = "support/artemis.rs"]
mod artemis;

use std::time::Duration;

use artemis::ArtemisBroker;
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_amqp_endpoint::broker::{ConnectParams, SaslCredentials};
use zerodds_amqp_endpoint::terminus::{DeliveryState, Outcome};
use zerodds_amqp_endpoint::transactions::TxnCapability;
use zerodds_amqp_endpoint::txn::{TxnClient, retired};

fn controller(broker: &ArtemisBroker) -> TxnClient {
    controller_desiring(broker, &[TxnCapability::LocalTransactions])
        .expect("SASL PLAIN, open, begin and the coordinator's attach with Artemis")
}

fn controller_desiring(
    broker: &ArtemisBroker,
    desired: &[TxnCapability],
) -> std::io::Result<TxnClient> {
    TxnClient::connect(
        &ConnectParams {
            addr: broker.addr(),
            host: "127.0.0.1".into(),
            tls: None,
            sasl: SaslCredentials::Plain {
                username: "guest".into(),
                password: "guest".into(),
            },
            container_id: "zerodds-txn".into(),
            hostname: None,
            max_frame_size: 65_536,
            channel_max: 7,
            idle_time_out_ms: 0,
            properties: Vec::new(),
            capabilities: Vec::new(),
            timeout: Duration::from_secs(10),
        },
        desired,
        Duration::from_secs(10),
    )
}

fn artemis_controller(broker: &ArtemisBroker) -> TxnClient {
    let mut c = controller(broker);
    c.set_txn_id_limit(ARTEMIS_TXN_ID_LEN);
    c
}

fn message(body: &[u8]) -> Vec<u8> {
    MessageSection::Data(body.to_vec())
        .encode()
        .expect("a data section")
}

/// The body of the data section of a message Artemis transferred; the
/// message is the sections Artemis sends (it may add a header and
/// annotations before the data section).
fn body(message: &[u8]) -> Vec<u8> {
    let mut rest = message;
    while !rest.is_empty() {
        let (section, used) = MessageSection::decode(rest).expect("a message section");
        if let MessageSection::Data(bytes) = section {
            return bytes;
        }
        rest = &rest[used..];
    }
    panic!("no data section in {message:02x?}");
}

/// Apache ActiveMQ Artemis declares transactions with the text of a UUID,
/// 36 octets, beyond the 32 of Part 4 §4.5.4.
const ARTEMIS_TXN_ID_LEN: usize = 36;

/// Every message the queue holds, read outside a transaction and
/// accepted, until none arrives within one second.
fn drain(c: &mut TxnClient, link: u32) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    while let Some(m) = c
        .receive(link, None, Duration::from_secs(1))
        .expect("receive")
    {
        c.retire(m.delivery_id, Outcome::Accepted, None, true)
            .expect("accepted");
        got.push(body(&m.message));
    }
    got
}

/// Part 4 §4.2, §4.3, §4.4.1, §4.5.1: Artemis's coordinator states
/// amqp:local-transactions; two messages posted in a transaction are
/// answered with a transactional-state of that transaction with the
/// outcome accepted and are in the queue after the commit; a message
/// posted in a transaction that is rolled back is not.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis and a JDK; runs in integration-bridges-vendor-amqp"]
fn amqp10_4_4_1_a_committed_send_is_in_the_artemis_queue_and_a_rolled_back_one_is_not() {
    let broker = ArtemisBroker::start(&["zd_txn_post"]);
    let mut c = artemis_controller(&broker);
    assert!(
        c.coordinator().has(TxnCapability::LocalTransactions),
        "{:?}",
        c.coordinator()
    );
    let sender = c.sender("zd_txn_post").expect("a sender");
    let committed = c.declare().expect("declared");
    for b in [&b"order-1"[..], b"order-2"] {
        let state = c
            .send(sender, &message(b), Some(&committed))
            .expect("sent in the transaction");
        assert_eq!(state, Some(retired(&committed, Outcome::Accepted)));
    }
    c.commit(&committed).expect("committed");
    let rolled_back = c.declare().expect("declared");
    assert_ne!(rolled_back, committed);
    let state = c
        .send(sender, &message(b"never"), Some(&rolled_back))
        .expect("sent in the transaction");
    assert_eq!(state, Some(retired(&rolled_back, Outcome::Accepted)));
    c.rollback(&rolled_back).expect("rolled back");
    let receiver = c.receiver("zd_txn_post").expect("a receiver");
    assert_eq!(
        drain(&mut c, receiver),
        [b"order-1".to_vec(), b"order-2".to_vec()]
    );
    c.close();
}

/// Part 4 §4.4.2: a message accepted in a transaction that is rolled back
/// is not consumed: Artemis does not settle the delivery, which stays with
/// the controller, and after the controller releases it outside a
/// transaction Artemis delivers it again. Accepted in a transaction that
/// commits, Artemis settles the delivery and the message is gone.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis and a JDK; runs in integration-bridges-vendor-amqp"]
fn amqp10_4_4_2_a_rolled_back_accept_is_delivered_again_by_artemis() {
    let broker = ArtemisBroker::start(&["zd_txn_retire"]);
    let mut c = artemis_controller(&broker);
    let sender = c.sender("zd_txn_retire").expect("a sender");
    let state = c.send(sender, &message(b"job"), None).expect("sent");
    assert_eq!(state, Some(DeliveryState::Outcome(Outcome::Accepted)));
    let receiver = c.receiver("zd_txn_retire").expect("a receiver");
    let first = c
        .receive(receiver, None, Duration::from_secs(5))
        .expect("receive")
        .expect("the message");
    assert_eq!(body(&first.message), b"job");
    let txn = c.declare().expect("declared");
    c.retire(first.delivery_id, Outcome::Accepted, Some(&txn), false)
        .expect("accepted in the transaction");
    c.rollback(&txn).expect("rolled back");
    assert_eq!(
        c.settled(first.delivery_id, Duration::from_millis(500))
            .expect("io"),
        None,
        "the accept of the rolled-back transaction was not applied"
    );
    c.retire(first.delivery_id, Outcome::Released, None, true)
        .expect("released");
    let again = c
        .receive(receiver, None, Duration::from_secs(5))
        .expect("receive")
        .expect("delivered again after the release");
    assert_eq!(body(&again.message), b"job");
    let txn = c.declare().expect("declared");
    c.retire(again.delivery_id, Outcome::Accepted, Some(&txn), false)
        .expect("accepted in the transaction");
    assert_eq!(
        c.settled(again.delivery_id, Duration::from_millis(300))
            .expect("io"),
        None,
        "not settled before the commit"
    );
    c.commit(&txn).expect("committed");
    let settled = c
        .settled(again.delivery_id, Duration::from_secs(5))
        .expect("io");
    assert!(settled.is_some(), "settled with the commit: {settled:?}");
    assert_eq!(drain(&mut c, receiver), Vec::<Vec<u8>>::new());
    c.close();
}

/// Part 4 §4.5.4, "A transaction-id can be up to 32 octets of binary
/// data": Artemis declares a transaction with 36 octets. The controller
/// with its default refuses that `declared` as a protocol error; told to
/// take 36 octets, it gets the identifier and commits with it.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis and a JDK; runs in integration-bridges-vendor-amqp"]
fn amqp10_4_5_4_artemis_declares_a_txn_id_of_36_octets() {
    let broker = ArtemisBroker::start(&["zd_txn_id"]);
    let mut strict = controller(&broker);
    let error = strict.declare().expect_err("36 octets are more than 32");
    assert!(
        error
            .to_string()
            .contains("a txn-id of 36 octets, more than 32"),
        "{error}"
    );
    let mut c = artemis_controller(&broker);
    let txn = c.declare().expect("declared");
    assert_eq!(txn.as_bytes().len(), ARTEMIS_TXN_ID_LEN);
    c.commit(&txn).expect("committed");
    c.close();
}

/// Part 4 §4.5.1, §4.5.7: Artemis's coordinator states
/// amqp:local-transactions, amqp:multi-txns-per-ssn and
/// amqp:multi-ssns-per-txn. A controller that desires those three
/// connects (the controller verifies them, §4.5.1), and declares a second
/// transaction on its session while the first is open; each is
/// discharged apart: the queue holds the message of the committed one
/// only. A controller that desires amqp:distributed-transactions does not
/// connect.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis and a JDK; runs in integration-bridges-vendor-amqp"]
fn amqp10_4_5_7_two_transactions_on_one_session_with_artemis() {
    let broker = ArtemisBroker::start(&["zd_txn_two"]);
    let desired = [
        TxnCapability::LocalTransactions,
        TxnCapability::MultiTxnsPerSsn,
        TxnCapability::MultiSsnsPerTxn,
    ];
    let mut c = controller_desiring(&broker, &desired).expect("Artemis has the three");
    c.set_txn_id_limit(ARTEMIS_TXN_ID_LEN);
    let sender = c.sender("zd_txn_two").expect("a sender");
    let first = c.declare().expect("declared");
    let second = c
        .declare()
        .expect("a second transaction while the first is open");
    assert_ne!(first, second);
    for (txn, b) in [(&first, &b"kept"[..]), (&second, b"dropped")] {
        let state = c
            .send(sender, &message(b), Some(txn))
            .expect("sent in the transaction");
        assert_eq!(state, Some(retired(txn, Outcome::Accepted)));
    }
    c.commit(&first).expect("committed");
    c.rollback(&second).expect("rolled back");
    let receiver = c.receiver("zd_txn_two").expect("a receiver");
    assert_eq!(drain(&mut c, receiver), [b"kept".to_vec()]);
    c.close();
    let Err(error) = controller_desiring(
        &broker,
        &[
            TxnCapability::LocalTransactions,
            TxnCapability::DistributedTransactions,
        ],
    ) else {
        panic!("Artemis states no amqp:distributed-transactions");
    };
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported, "{error}");
}
