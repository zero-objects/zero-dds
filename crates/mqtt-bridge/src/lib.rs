// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-mqtt-bridge`. Safety classification: **STANDARD**.
//!
//! MQTT v5.0 (OASIS Standard 07 March 2019) Wire-Codec + Broker +
//! DDS-Bridge — pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
//! Implementiert die volle MQTT-5.0-Spec inkl. aller Control-Packet-
//! Bodies (CONNECT / CONNACK / PUBLISH / PUBACK / PUBREC / PUBREL /
//! PUBCOMP / SUBSCRIBE / SUBACK / UNSUBSCRIBE / UNSUBACK / PINGREQ /
//! PINGRESP / DISCONNECT / AUTH), aller Properties (§2.2.2),
//! Keep-Alive-Tracker (§3.1.2.10), Topic-Filter-Matching mit
//! Wildcards (§4.7), In-Memory-Broker mit Session-State + Retained-
//! Messages + Will-Messages, und einen MQTT↔DDS-Topic-Bridge.
//!
//! Spec: OASIS MQTT 5.0 §1 (Introduction), §1.5 (Data Types), §2
//! (MQTT Control Packet Format), §3 (MQTT Control Packets), §4
//! (Operational Behavior).
//!
//! ## Schichten-Position
//!
//! Layer 5 — Bridges. Substrat fuer DDS↔MQTT-Endpoint-Mapping
//! (IoT-Broker-Integration, Cloud-MQTT-Endpoints).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`FixedHeader`] / [`ControlPacketType`] — §2.1.
//! - [`encode_publish`] / [`decode_publish`] / [`CodecError`] —
//!   PUBLISH-Codec (§3.3).
//! - Encoder/Decoder fuer alle 14 Control-Packet-Bodies — siehe
//!   [`control_packets`].
//! - [`Property`] / [`PropertyId`] / [`PropertyValueKind`] /
//!   [`PropertyDataType`] — §2.2.2.
//! - [`encode_vbi`] / [`decode_vbi`] / [`vbi_size`] — Variable Byte
//!   Integer (§1.5.5).
//! - [`encode_utf8_string`] / [`decode_utf8_string`] /
//!   [`encode_binary_data`] / [`decode_binary_data`] /
//!   [`encode_two_byte_int`] / [`decode_two_byte_int`] — Data Types
//!   §1.5.
//! - [`Broker`] / [`Session`] / [`BrokerSubscription`] /
//!   [`RetainedMessage`] / [`DeliveryEnvelope`] / [`Will`] / [`QoS`]
//!   — Broker-Logic (§3 + §4).
//! - [`KeepAliveTracker`] — §3.1.2.10.
//! - [`ReasonCode`] — §2.4.
//! - [`topic_matches`] / [`validate_filter`] / [`validate_topic_name`]
//!   / [`TopicFilterError`] — Topic-Filter-Matching (§4.7).
//! - [`MqttDdsBridge`] / [`TopicMapper`] / [`BridgeStats`] /
//!   [`DdsDurability`] / [`DdsReliability`] / [`mqtt_qos_to_dds`] /
//!   [`dds_qos_to_mqtt`] / [`forward_user_properties`] —
//!   MQTT↔DDS-Topic-Mapping.
//!
//! ## Beispiel
//!
//! ```rust
//! use zerodds_mqtt_bridge::{encode_vbi, decode_vbi};
//!
//! let buf = encode_vbi(268_435_455).expect("encode max VBI");
//! let (v, consumed) = decode_vbi(&buf).expect("decode");
//! assert_eq!(v, 268_435_455);
//! assert_eq!(consumed, 4);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::approx_constant,
    clippy::unreachable,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    clippy::useless_conversion,
    clippy::manual_strip,
    missing_docs
)]

extern crate alloc;

pub mod codec;
pub mod control_packets;
pub mod data_types;
pub mod packet;
pub mod properties;
pub mod vbi;

pub use codec::{CodecError, decode_publish, encode_publish};
pub use control_packets::{
    AckBody, AuthBody, ConnackBody, ConnectBody, DisconnectBody, PropertyDataType, SubackBody,
    SubscribeBody, Subscription, UnsubackBody, UnsubscribeBody, decode_ack_body, decode_auth_body,
    decode_connack_body, decode_connect_body, decode_disconnect_body, decode_suback_body,
    decode_subscribe_body, decode_unsuback_body, decode_unsubscribe_body, encode_ack_body,
    encode_auth_body, encode_connack_body, encode_connect_body, encode_disconnect_body,
    encode_suback_body, encode_subscribe_body, encode_unsuback_body, encode_unsubscribe_body,
    property_data_type,
};
pub use data_types::{
    decode_binary_data, decode_two_byte_int, decode_utf8_string, encode_binary_data,
    encode_two_byte_int, encode_utf8_string,
};
pub use packet::{ControlPacketType, FixedHeader};
pub use properties::{Property, PropertyId, PropertyValueKind};
pub use vbi::{decode_vbi, encode_vbi, vbi_size};

pub mod broker;
#[cfg(feature = "daemon")]
pub mod daemon;
pub mod dds_bridge;
pub mod keep_alive;
pub mod reason_codes;
pub mod topic_filter;

pub use broker::{
    Broker, DeliveryEnvelope, QoS, RetainedMessage, Session, Subscription as BrokerSubscription,
    Will,
};
pub use dds_bridge::{
    BridgeStats, DdsDurability, DdsReliability, MqttDdsBridge, TopicMapper, dds_qos_to_mqtt,
    forward_user_properties, mqtt_qos_to_dds,
};
pub use keep_alive::KeepAliveTracker;
pub use reason_codes::ReasonCode;
pub use topic_filter::{
    TopicFilterError, matches as topic_matches, validate_filter, validate_topic_name,
};
