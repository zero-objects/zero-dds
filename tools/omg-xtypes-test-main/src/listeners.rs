// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The lines `interoperability_report.py` matches on.
//!
//! The harness is a `pexpect` state machine, so these strings are not
//! diagnostics: they are the protocol. Each one below carries the
//! `expect` list it satisfies, by index, from the branch of
//! `interoperability_report.py` that reads it. A run that prints a line
//! the harness matches earlier than it should reports a different
//! `ReturnCode` for the same behaviour, so nothing else this client
//! writes to the console may contain `[<digits>]`: that is the
//! subscriber's index 0 and would be read as a received sample.
//!
//! Everything goes to stdout. `pexpect.spawnu` gives the child one pty
//! for both streams, so stderr would be matched too; keeping the
//! protocol on one stream keeps the interleaving deterministic.

#![allow(clippy::print_stdout)]

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use zerodds_dcps::InstanceHandle;
use zerodds_dcps::listener::{DataReaderListener, DataWriterListener, TopicListener};
use zerodds_dcps::status::{
    InconsistentTopicStatus, OfferedIncompatibleQosStatus, PublicationMatchedStatus,
    RequestedIncompatibleQosStatus, SubscriptionMatchedStatus,
};

/// Writes one protocol line and flushes it.
///
/// The harness reads the pty as the child writes it and times out after
/// ten seconds; a block-buffered line would be read as an absent line.
pub fn line(text: &str) {
    println!("{text}");
    let _ = std::io::stdout().flush();
}

/// Whether a status callback carries an actual change.
///
/// `DataReader::drive_listeners` reaches `poll_subscription_matched` and
/// `poll_requested_incompatible_qos` (`crates/dcps/src/subscriber.rs`),
/// and both hold the previous value in an `AtomicI64` initialised to
/// `-1`. The first poll on a fresh reader compares `-1` against `0`,
/// finds them unequal, and dispatches with `count: 0, change: 0`. DDS
/// 1.4 §2.2.4.1 has a listener invoked when the status *changes*.
///
/// For this suite the sentinel is not cosmetic: 430 of the 670 entity
/// expectations in `test_suite.py` are `INCONSISTENT_TOPIC`, which the
/// harness reaches only if no earlier pattern matched. A spurious
/// `on_requested_incompatible_qos()` turns every one of them into
/// `INCOMPATIBLE_QOS`.
#[must_use]
pub fn is_real_change(count: i32, change: i32) -> bool {
    count != 0 || change != 0
}

/// Set once the type consistency check has failed, so the run stops
/// waiting for a match that will not come.
#[derive(Debug, Default)]
pub struct Flags {
    /// `on_inconsistent_topic` has been printed.
    pub inconsistent: AtomicBool,
    /// `on_publication_matched()` / a match has been printed.
    pub matched: AtomicBool,
}

/// The topic listener. XTypes 1.3 §7.6.3.4.2: when either step of the
/// type-consistency rules fails, "the Service shall trigger an
/// INCONSISTENT_TOPIC status change for both the DataReader's Topic and
/// the DataWriter's Topic".
///
/// Harness patterns satisfied: publisher step 4 index 2 and subscriber
/// step 4 index 4, both spelled `on_inconsistent_topic` without
/// parentheses.
pub struct XtypesTopicListener {
    topic_name: String,
    flags: std::sync::Arc<Flags>,
}

impl XtypesTopicListener {
    /// The listener carries the topic name because the callback is
    /// handed an opaque `InstanceHandle`.
    #[must_use]
    pub fn new(topic_name: String, flags: std::sync::Arc<Flags>) -> Self {
        Self { topic_name, flags }
    }
}

impl TopicListener for XtypesTopicListener {
    fn on_inconsistent_topic(&self, _topic: InstanceHandle, status: InconsistentTopicStatus) {
        if !is_real_change(status.total_count, status.total_count_change) {
            return;
        }
        if self.flags.inconsistent.swap(true, Ordering::SeqCst) {
            return;
        }
        line(&format!(
            "on_inconsistent_topic total_count={} topic=\"{}\"",
            status.total_count, self.topic_name
        ));
    }
}

/// The writer listener. Harness publisher step 4: index 0
/// `on_publication_matched()`, index 1 `on_offered_incompatible_qos`.
pub struct XtypesWriterListener {
    flags: std::sync::Arc<Flags>,
}

impl XtypesWriterListener {
    /// One per writer.
    #[must_use]
    pub fn new(flags: std::sync::Arc<Flags>) -> Self {
        Self { flags }
    }
}

impl DataWriterListener for XtypesWriterListener {
    fn on_publication_matched(&self, _writer: InstanceHandle, status: PublicationMatchedStatus) {
        if !is_real_change(status.current_count, status.current_count_change) {
            return;
        }
        if self.flags.matched.swap(true, Ordering::SeqCst) {
            return;
        }
        line(&format!(
            "on_publication_matched() current_count={}",
            status.current_count
        ));
    }

    fn on_offered_incompatible_qos(
        &self,
        _writer: InstanceHandle,
        status: OfferedIncompatibleQosStatus,
    ) {
        if !is_real_change(status.total_count, status.total_count_change) {
            return;
        }
        line(&format!(
            "on_offered_incompatible_qos last_policy_id={}",
            status.last_policy_id
        ));
    }
}

/// The reader listener. Harness subscriber step 4: index 1
/// `on_requested_incompatible_qos()`.
///
/// `on_subscription_matched` is deliberately **not** printed. The
/// harness's subscriber does not key its success on it — index 0 is
/// `[<digits>]` and index 3 is `sample_received()`, both of which mean a
/// sample arrived — and a matched pair that then delivers nothing is
/// `DATA_NOT_RECEIVED`, which is what 21 of the suite's expectations
/// are. Printing a match line here would satisfy nothing the harness
/// reads and would risk matching index 0 through its count.
pub struct XtypesReaderListener {
    flags: std::sync::Arc<Flags>,
}

impl XtypesReaderListener {
    /// One per reader.
    #[must_use]
    pub fn new(flags: std::sync::Arc<Flags>) -> Self {
        Self { flags }
    }
}

impl DataReaderListener for XtypesReaderListener {
    fn on_requested_incompatible_qos(
        &self,
        _reader: InstanceHandle,
        status: RequestedIncompatibleQosStatus,
    ) {
        if !is_real_change(status.total_count, status.total_count_change) {
            return;
        }
        line(&format!(
            "on_requested_incompatible_qos() last_policy_id={}",
            status.last_policy_id
        ));
    }

    fn on_subscription_matched(&self, _reader: InstanceHandle, status: SubscriptionMatchedStatus) {
        if is_real_change(status.current_count, status.current_count_change) {
            self.flags.matched.store(true, Ordering::SeqCst);
        }
    }
}
