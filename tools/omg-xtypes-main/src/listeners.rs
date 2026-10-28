// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The two listeners `srcCxx/ShapeType_variants.h` installs, printing
//! the exact lines it prints.
//!
//! Unlike `dds-rtps`, this suite has no `pexpect` harness: `dds-xtypes`
//! ships one C++ source and a spreadsheet, and a run is read by a person
//! comparing two terminals. The lines below are still transcribed
//! character for character from `ShapeType_variants.h`
//! (`TheTopicListener::on_inconsistent_topic`,
//! `TheReaderListener::on_requested_incompatible_qos` and
//! `::on_subscription_matched`), because a reviewer holding our log
//! against a vendor's should be diffing the measurement and not the
//! formatting.
//!
//! All three go to **stderr**, as they do in the suite. The sample data
//! is the only thing on stdout.

#![allow(clippy::print_stderr)]

use zerodds_dcps::InstanceHandle;
use zerodds_dcps::listener::{DataReaderListener, TopicListener};
use zerodds_dcps::status::{
    InconsistentTopicStatus, RequestedIncompatibleQosStatus, SubscriptionMatchedStatus,
};

/// `TheTopicListener` — the one status this suite exists to provoke.
///
/// Two peers of this suite always share the topic name
/// `XTYPESTestTopic` and differ in the type name, which is the textbook
/// INCONSISTENT_TOPIC condition of DDS 1.4 §2.2.4.2.4. See the crate
/// README, "Flag and status coverage", for what our runtime does and
/// does not raise it on today.
pub struct ShapeTopicListener {
    topic_name: String,
    type_name: &'static str,
}

impl ShapeTopicListener {
    /// The listener carries the two names because the callback is only
    /// handed an opaque `InstanceHandle`.
    pub fn new(topic_name: String, type_name: &'static str) -> Self {
        Self {
            topic_name,
            type_name,
        }
    }
}

impl TopicListener for ShapeTopicListener {
    fn on_inconsistent_topic(&self, _topic: InstanceHandle, status: InconsistentTopicStatus) {
        eprintln!(
            "on_inconsistent_topic: topic \"{}\", type \"{}\" count: {}, change: {}",
            self.topic_name, self.type_name, status.total_count, status.total_count_change
        );
    }
}

/// `TheReaderListener` — `on_requested_incompatible_qos` and
/// `on_subscription_matched`, the two statuses the suite's reader arms
/// (`REQUESTED_INCOMPATIBLE_QOS_STATUS | SUBSCRIPTION_MATCHED_STATUS`).
///
/// `on_subscription_matched` is the line that answers the suite's
/// question for a compatible pair, and its absence is the answer for an
/// incompatible one.
pub struct ShapeReaderListener {
    topic_name: String,
    type_name: &'static str,
}

impl ShapeReaderListener {
    /// See [`ShapeTopicListener::new`].
    pub fn new(topic_name: String, type_name: &'static str) -> Self {
        Self {
            topic_name,
            type_name,
        }
    }
}

impl DataReaderListener for ShapeReaderListener {
    fn on_requested_incompatible_qos(
        &self,
        _reader: InstanceHandle,
        status: RequestedIncompatibleQosStatus,
    ) {
        eprintln!(
            "on_requested_incompatible_qos: topic \"{}\", type \"{}\" last_policy: {}, count: {}, change: {}",
            self.topic_name,
            self.type_name,
            status.last_policy_id,
            status.total_count,
            status.total_count_change
        );
    }

    fn on_subscription_matched(&self, _reader: InstanceHandle, status: SubscriptionMatchedStatus) {
        eprintln!(
            "on_subscription_matched: topic \"{}\", type \"{}\", count: {}, change: {}",
            self.topic_name, self.type_name, status.current_count, status.current_count_change
        );
    }
}
