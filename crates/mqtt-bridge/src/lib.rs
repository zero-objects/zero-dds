// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-mqtt-bridge`. Safety classification: **STANDARD**.
//!
//! MQTT v5.0 (OASIS Standard 07 March 2019) wire codec + broker +
//! DDS bridge — pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
//! Implements the full MQTT 5.0 spec incl. all control-packet
//! bodies (CONNECT / CONNACK / PUBLISH / PUBACK / PUBREC / PUBREL /
//! PUBCOMP / SUBSCRIBE / SUBACK / UNSUBSCRIBE / UNSUBACK / PINGREQ /
//! PINGRESP / DISCONNECT / AUTH), all properties (§2.2.2),
//! a keep-alive tracker (§3.1.2.10), topic-filter matching with
//! wildcards (§4.7), an in-memory broker with session state + retained
//! messages + will messages, and an MQTT↔DDS topic bridge.
//!
//! Spec: OASIS MQTT 5.0 §1 (Introduction), §1.5 (Data Types), §2
//! (MQTT Control Packet Format), §3 (MQTT Control Packets), §4
//! (Operational Behavior).
//!
//! ## Layer position
//!
//! Layer 5 — bridges. Substrate for DDS↔MQTT endpoint mapping
//! (IoT broker integration, cloud MQTT endpoints).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`FixedHeader`] / [`ControlPacketType`] — §2.1.
//! - [`encode_publish`] / [`decode_publish`] / [`CodecError`] —
//!   PUBLISH codec (§3.3).
//! - Encoders/decoders for all 14 control-packet bodies — see
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
//!   — broker logic (§3 + §4).
//! - [`KeepAliveTracker`] — §3.1.2.10.
//! - [`ReasonCode`] — §2.4.
//! - [`topic_matches`] / [`validate_filter`] / [`validate_topic_name`]
//!   / [`TopicFilterError`] — topic-filter matching (§4.7).
//! - [`MqttDdsBridge`] / [`TopicMapper`] / [`BridgeStats`] /
//!   [`DdsDurability`] / [`DdsReliability`] / [`mqtt_qos_to_dds`] /
//!   [`dds_qos_to_mqtt`] / [`forward_user_properties`] —
//!   MQTT↔DDS topic mapping.
//!
//! ## Example
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
pub mod version;

pub use codec::{CodecError, decode_publish, decode_publish_v, encode_publish, encode_publish_v};
pub use control_packets::{
    AckBody, AuthBody, ConnackBody, ConnectBody, DisconnectBody, PropertyDataType, SubackBody,
    SubscribeBody, Subscription, UnsubackBody, UnsubscribeBody, decode_ack_body, decode_auth_body,
    decode_connack_body, decode_connack_body_v, decode_connect_body, decode_connect_body_v,
    decode_disconnect_body, decode_suback_body, decode_suback_body_v, decode_subscribe_body,
    decode_subscribe_body_v, decode_unsuback_body, decode_unsuback_body_v, decode_unsubscribe_body,
    decode_unsubscribe_body_v, encode_ack_body, encode_ack_body_v, encode_auth_body,
    encode_connack_body, encode_connack_body_v, encode_connect_body, encode_connect_body_v,
    encode_disconnect_body, encode_disconnect_body_v, encode_suback_body, encode_suback_body_v,
    encode_subscribe_body, encode_subscribe_body_v, encode_unsuback_body, encode_unsuback_body_v,
    encode_unsubscribe_body, encode_unsubscribe_body_v, property_data_type,
};
pub use data_types::{
    decode_binary_data, decode_two_byte_int, decode_utf8_string, encode_binary_data,
    encode_two_byte_int, encode_utf8_string,
};
pub use packet::{ControlPacketType, FixedHeader};
pub use properties::{Property, PropertyId, PropertyValueKind};
pub use vbi::{decode_vbi, encode_vbi, vbi_size};
pub use version::ProtocolVersion;

pub mod broker;
#[cfg(feature = "daemon")]
pub mod daemon;
pub mod dds_bridge;
pub mod keep_alive;
pub mod reason_codes;
pub mod topic_filter;

#[cfg(feature = "std")]
pub mod net;
#[cfg(feature = "std")]
pub mod server;

#[cfg(feature = "std")]
pub use net::MqttClient;
#[cfg(feature = "std")]
pub use server::{MqttBrokerServer, ServerHandle};

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
