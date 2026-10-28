// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What the AccessControl checks of DDS-Security 1.2 §9.4.2.9 are given of an
//! endpoint: its QoS and its data tags (`qos` and `data_tag` of
//! `check_create_datawriter`, `check_create_datareader`,
//! `check_remote_datawriter`, `check_remote_datareader`, ...).
//!
//! The QoS goes as `(name, value)` pairs, the name that of the DDS QoS policy
//! and the value its textual rendering; the `PropertyQosPolicy` of the
//! endpoint goes as `property.<name>`. The plugin of the application reads
//! what it needs; the builtin plugin decides on partitions and data tags
//! (Table 70).

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Debug;

use zerodds_rtps::property_list::{WireDataTags, WirePropertyList};
use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;

use crate::runtime::{UserReaderConfig, UserWriterConfig};

/// The QoS entries of an endpoint, owned.
#[derive(Debug, Default, Clone)]
pub(crate) struct AccessQos {
    entries: Vec<(String, String)>,
}

impl AccessQos {
    fn push(&mut self, name: &str, value: &dyn Debug) {
        self.entries
            .push((name.to_string(), alloc::format!("{value:?}")));
    }

    fn properties(&mut self, properties: &WirePropertyList) {
        for property in &properties.entries {
            self.entries.push((
                alloc::format!("property.{}", property.name),
                property.value.clone(),
            ));
        }
    }

    /// The entries as the borrowed pairs the checks take.
    pub(crate) fn pairs(&self) -> Vec<(&str, &str)> {
        self.entries
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect()
    }
}

/// The QoS of a local DataWriter being created.
pub(crate) fn writer_qos(cfg: &UserWriterConfig) -> AccessQos {
    let mut q = AccessQos::default();
    q.push(
        "reliability",
        &if cfg.reliable {
            "RELIABLE"
        } else {
            "BEST_EFFORT"
        },
    );
    q.push("durability", &cfg.durability);
    q.push("deadline", &cfg.deadline);
    q.push("latency_budget", &cfg.latency_budget);
    q.push("destination_order", &cfg.destination_order);
    q.push("lifespan", &cfg.lifespan);
    q.push("liveliness", &cfg.liveliness);
    q.push("ownership", &cfg.ownership);
    q.push("ownership_strength", &cfg.ownership_strength);
    q.push("transport_priority", &cfg.transport_priority);
    q.push("presentation", &cfg.presentation);
    for (name, value) in cfg.property.iter() {
        q.entries
            .push((alloc::format!("property.{name}"), value.to_string()));
    }
    q
}

/// The QoS of a local DataReader being created.
pub(crate) fn reader_qos(cfg: &UserReaderConfig) -> AccessQos {
    let mut q = AccessQos::default();
    q.push(
        "reliability",
        &if cfg.reliable {
            "RELIABLE"
        } else {
            "BEST_EFFORT"
        },
    );
    q.push("durability", &cfg.durability);
    q.push("deadline", &cfg.deadline);
    q.push("latency_budget", &cfg.latency_budget);
    q.push("destination_order", &cfg.destination_order);
    q.push("liveliness", &cfg.liveliness);
    q.push("ownership", &cfg.ownership);
    q.push("presentation", &cfg.presentation);
    q.push("time_based_filter", &cfg.time_based_filter);
    for (name, value) in cfg.property.iter() {
        q.entries
            .push((alloc::format!("property.{name}"), value.to_string()));
    }
    q
}

/// The QoS of an announced DataWriter, local or remote.
pub(crate) fn publication_qos(d: &PublicationBuiltinTopicData) -> AccessQos {
    let mut q = AccessQos::default();
    q.push("reliability", &d.reliability);
    q.push("durability", &d.durability);
    q.push("deadline", &d.deadline);
    q.push("latency_budget", &d.latency_budget);
    q.push("destination_order", &d.destination_order);
    q.push("lifespan", &d.lifespan);
    q.push("liveliness", &d.liveliness);
    q.push("ownership", &d.ownership);
    q.push("ownership_strength", &d.ownership_strength);
    q.push("presentation", &d.presentation);
    q.properties(&d.properties);
    q
}

/// The QoS of an announced DataReader, local or remote.
pub(crate) fn subscription_qos(d: &SubscriptionBuiltinTopicData) -> AccessQos {
    let mut q = AccessQos::default();
    q.push("reliability", &d.reliability);
    q.push("durability", &d.durability);
    q.push("deadline", &d.deadline);
    q.push("latency_budget", &d.latency_budget);
    q.push("destination_order", &d.destination_order);
    q.push("liveliness", &d.liveliness);
    q.push("ownership", &d.ownership);
    q.push("presentation", &d.presentation);
    q.push("time_based_filter", &d.time_based_filter);
    q.properties(&d.properties);
    q
}

/// The QoS of a Topic being created or changed.
pub(crate) fn topic_qos(qos: &crate::qos::TopicQos) -> AccessQos {
    let mut q = AccessQos::default();
    q.push("durability", &qos.durability);
    q.push("durability_service", &qos.durability_service);
    q.push("deadline", &qos.deadline);
    q.push("latency_budget", &qos.latency_budget);
    q.push("liveliness", &qos.liveliness);
    q.push("reliability", &qos.reliability);
    q.push("destination_order", &qos.destination_order);
    q.push("history", &qos.history);
    q.push("resource_limits", &qos.resource_limits);
    q.push("transport_priority", &qos.transport_priority);
    q.push("lifespan", &qos.lifespan);
    q.push("ownership", &qos.ownership);
    q
}

/// The QoS of a remote Topic, from the topic data its participant announced.
pub(crate) fn remote_topic_qos(topic_data: &[u8]) -> AccessQos {
    let mut q = AccessQos::default();
    q.push("topic_data", &topic_data);
    q
}

/// The data tags of a local endpoint as (name, value).
pub(crate) fn tag_pairs(tags: &zerodds_qos::DataTagQosPolicy) -> Vec<(String, String)> {
    tags.tags
        .iter()
        .map(|t| (t.name.clone(), t.value.clone()))
        .collect()
}

/// The data tags of an announced endpoint as (name, value).
pub(crate) fn wire_tag_pairs(tags: &WireDataTags) -> Vec<(String, String)> {
    tags.tags
        .iter()
        .map(|t| (t.name.clone(), t.value.clone()))
        .collect()
}

/// Borrows owned pairs.
pub(crate) fn borrowed(pairs: &[(String, String)]) -> Vec<(&str, &str)> {
    pairs
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect()
}
