// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What a [`crate::requester::Requester`] or [`crate::replier::Replier`]
//! is built with besides names and QoS: the service mapping it speaks,
//! the DDS type names of its topics, and how it treats the Enhanced
//! discovery rules of §7.6.2.2.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::codegen::REMOTE_EX_MEMBER;
use crate::error::{RpcError, RpcResult};

/// The service mapping the endpoints speak (Spec §7.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RpcMapping {
    /// §7.5.1.1: `${I}_Request { RequestHeader header; ${I}_Call data; }`
    /// and `${I}_Reply { ReplyHeader header; ${I}_Return data; }`,
    /// correlated by the identity in the header (§7.8.1). Plain DDS
    /// discovery (§7.6.1). The default.
    #[default]
    Basic,
    /// §7.5.1.2: `@Choice @Autoid` `${I}_Request` and `${I}_Reply`
    /// without a header, correlated by the sample identity of the request
    /// and `PID_RELATED_SAMPLE_IDENTITY` of the reply (§7.8.2), with the
    /// discovery extension and rules of §7.6.2.
    Enhanced,
}

/// The CDR version of the Basic Service Mapping's samples.
///
/// §7.5.1.1: "The Basic Service Mapping maps interfaces to data-types
/// that can be used by DDS version 1.3 compliant implementations, which
/// may lack support for the [DDS-XTypes] specification." DDS 1.3 has
/// classic CDR only (XCDR1, `CDR_LE` in the encapsulation), where an
/// 8-octet member aligns to 8; XCDR2 aligns it to 4. The headers
/// (§7.5.1.1.1) are the same octets in both, which is why the difference
/// shows only in a payload that has such a member. The Enhanced Service
/// Mapping is defined over XTypes (§7.5.1.2) and is always XCDR2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireEncoding {
    /// Classic CDR: the samples are written and read in XCDR1 and the
    /// endpoints offer and request the XCDR1 data representation only,
    /// so a DDS 1.3 peer without XTypes matches them. The default.
    #[default]
    Xcdr1,
    /// XCDR2, the encoding of XTypes: the samples are written in XCDR2.
    /// For peers that speak XTypes and for payload codecs that have no
    /// XCDR1 form.
    Xcdr2,
}

impl WireEncoding {
    /// The `data_representation` list a writer of this encoding offers
    /// (`XCDR = 0`, `XCDR2 = 2`). A writer of XCDR2 samples says so: an absent list
    /// means XCDR1 to a peer (XTypes 1.3 §7.6.3.1.1), which would not match it.
    #[must_use]
    pub fn representation(self) -> Option<Vec<i16>> {
        match self {
            Self::Xcdr1 => Some(alloc::vec![0]),
            Self::Xcdr2 => Some(alloc::vec![2]),
        }
    }
}

/// The DDS type names of the request and reply topics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicTypeNames {
    /// Type of the request topic: `${interfaceName}_Request`
    /// (§7.5.1.1.6 rule 1, §7.5.1.2.4 rule 1).
    pub request: String,
    /// Type of the reply topic: `${interfaceName}_Reply`
    /// (§7.5.1.1.7 rule 1, §7.5.1.2.5 rule 1).
    pub reply: String,
}

impl TopicTypeNames {
    /// The names for interface `interface` declared in the IDL module
    /// `scope` (`""` at file scope, `"robot"`, `"a::b"`): the structure
    /// names of the spec in the scope they are declared in, which is how
    /// a DDS implementation names the type of a structure in a module.
    #[must_use]
    pub fn for_interface(scope: &str, interface: &str) -> Self {
        let qualify = |leaf: String| {
            if scope.is_empty() {
                leaf
            } else {
                alloc::format!("{scope}::{leaf}")
            }
        };
        Self {
            request: qualify(alloc::format!("{interface}_Request")),
            reply: qualify(alloc::format!("{interface}_Reply")),
        }
    }
}

/// What the client does with a request while the service is not
/// discovered (Enhanced, §7.6.2.2.2, Figure 7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceMatching {
    /// The default, and the spec's: the client "does not send any
    /// request" until service matching completes; "attempts to send
    /// requests would be queued". The sending call holds the request and
    /// writes it once the service is complete, waiting at most the
    /// request timeout of the QoS; after that it fails with
    /// `RpcError::ServiceNotMatched` and the request has not been sent.
    #[default]
    Queue,
    /// The sending call fails at once with `RpcError::ServiceNotMatched`
    /// when the service is not complete.
    Fail,
    /// No check: the request goes out at once, as in the Basic mapping.
    Off,
}

/// What the service does with a request from a client it has not fully
/// discovered (Enhanced, §7.6.2.2.1, Figure 7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClientMatching {
    /// The default, and the spec's: the service "shall reject requests"
    /// of a client writer until its reply writer has discovered the
    /// client's reply reader. A rejected request is held and processed
    /// as soon as the client is complete, so the client's request is
    /// not lost to the race the rule exists to close.
    #[default]
    Hold,
    /// No check.
    Off,
}

/// The `CompleteTypeObject` of one topic type and the objects it names, as
/// the generator writes them (XCDR2 little-endian, the form
/// `TypeObject::to_bytes_le` produces).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopicTypeBytes {
    /// The object of the topic type.
    pub object: &'static [u8],
    /// Every object `object` names by hash, directly or through another.
    pub dependencies: &'static [&'static [u8]],
}

impl TopicTypeBytes {
    fn decode(&self) -> RpcResult<zerodds_dcps::TopicTypeInformation> {
        let read = |bytes: &[u8]| {
            zerodds_types::TypeObject::from_bytes_le(bytes)
                .map_err(|e| RpcError::Dcps(alloc::format!("type object: {e:?}")))
        };
        Ok(zerodds_dcps::TopicTypeInformation {
            object: read(self.object)?,
            dependencies: self
                .dependencies
                .iter()
                .map(|bytes| read(bytes))
                .collect::<RpcResult<Vec<_>>>()?,
        })
    }
}

/// The XTypes description of the two topic types of a service
/// (DDS-RPC 1.0 §7.5.1.2, §7.7): with it the endpoints announce a
/// `TypeIdentifier` and match a peer by assignability, which is what lets a
/// client of a base interface meet a service of a derived one
/// (§7.4.3) and an old client a new service (§7.7.2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpcTypeInformation {
    /// `${interface}_Request`.
    pub request: TopicTypeBytes,
    /// `${interface}_Reply`.
    pub reply: TopicTypeBytes,
}

/// Creates the `RawBytes` topic `name` of type `type_name`, described by
/// `information` when there is one.
pub(crate) fn create_topic(
    participant: &zerodds_dcps::participant::DomainParticipant,
    name: &str,
    type_name: &str,
    information: Option<&TopicTypeBytes>,
) -> RpcResult<zerodds_dcps::topic::Topic<zerodds_dcps::dds_type::RawBytes>> {
    use zerodds_dcps::dds_type::RawBytes;
    use zerodds_dcps::qos::TopicQos;
    let created = match information {
        Some(bytes) => participant.create_topic_with_type_information::<RawBytes>(
            name,
            type_name,
            bytes.decode()?,
            TopicQos::default(),
        ),
        None => participant.create_topic_with_type_name::<RawBytes>(
            name,
            type_name,
            TopicQos::default(),
        ),
    };
    created.map_err(|e| RpcError::Dcps(alloc::format!("create_topic {name}: {e:?}")))
}

/// Everything besides names and QoS that an RPC endpoint is built with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcEndpointOptions {
    /// The service mapping. Default [`RpcMapping::Basic`].
    pub mapping: RpcMapping,
    /// The DDS type names of the topics. `None` derives them from the
    /// interface name the endpoint is created with.
    pub type_names: Option<TopicTypeNames>,
    /// The qualified names of the base interfaces: their topics are the
    /// `topic_aliases` the endpoint announces (§7.4.3).
    pub parents: Vec<String>,
    /// The CDR version of Basic samples. Default [`WireEncoding::Xcdr1`].
    /// Ignored under [`RpcMapping::Enhanced`], which is XCDR2.
    pub wire_encoding: WireEncoding,
    /// See [`ServiceMatching`]. Enhanced requester only.
    pub service_matching: ServiceMatching,
    /// See [`ClientMatching`]. Enhanced replier only.
    pub client_matching: ClientMatching,
    /// Read a peer that announced none of the discovery extension as
    /// complete by the DDS match alone. The spec's algorithm needs the
    /// announcement to know which endpoints of a peer belong together
    /// (`false`, the default); a peer that is not a DDS-RPC Enhanced
    /// implementation announces none.
    pub accept_unannounced: bool,
    /// The name of the `remoteEx` member of the Enhanced `Reply`
    /// (`remoteEx_N` where an operation is called `remoteEx`,
    /// §7.5.1.2.5 rule 5).
    pub remote_ex_member: String,
    /// The XTypes description of the two topic types. `None`: the topics
    /// carry a type name only and peers match by it (§7.5.1.1.6/7; every
    /// Basic binding).
    pub type_information: Option<RpcTypeInformation>,
}

impl Default for RpcEndpointOptions {
    fn default() -> Self {
        Self {
            mapping: RpcMapping::Basic,
            type_names: None,
            parents: Vec::new(),
            wire_encoding: WireEncoding::Xcdr1,
            service_matching: ServiceMatching::Queue,
            client_matching: ClientMatching::Hold,
            accept_unannounced: false,
            remote_ex_member: REMOTE_EX_MEMBER.to_string(),
            type_information: None,
        }
    }
}

impl RpcEndpointOptions {
    /// Options for interface `interface` declared in the IDL module
    /// `scope` (`""` at file scope, `"a::b"`): its topic types are
    /// `${interface}_Request` and `_Reply` in that scope. What a generated
    /// binding passes.
    #[must_use]
    pub fn for_interface(scope: &str, interface: &str) -> Self {
        Self {
            type_names: Some(TopicTypeNames::for_interface(scope, interface)),
            ..Self::default()
        }
    }

    /// The same options for another service mapping. What a binding
    /// generated for the Enhanced Service Mapping passes.
    #[must_use]
    pub fn with_mapping(mut self, mapping: RpcMapping) -> Self {
        self.mapping = mapping;
        self
    }

    /// The XTypes description of the two topic types, as a generated
    /// binding passes it.
    #[must_use]
    pub fn with_type_information(mut self, information: RpcTypeInformation) -> Self {
        self.type_information = Some(information);
        self
    }

    /// Adds a base interface (by its qualified name), whose topics are
    /// announced as topic aliases (§7.4.3).
    #[must_use]
    pub fn with_parent(mut self, qualified_name: &str) -> Self {
        self.parents.push(qualified_name.to_string());
        self
    }

    /// Whether Basic samples are classic CDR under these options: the
    /// Basic mapping with [`WireEncoding::Xcdr1`].
    #[must_use]
    pub fn xcdr1(&self) -> bool {
        self.mapping == RpcMapping::Basic && self.wire_encoding == WireEncoding::Xcdr1
    }

    /// The Enhanced Service Mapping with every other setting at its
    /// default.
    #[must_use]
    pub fn enhanced() -> Self {
        Self {
            mapping: RpcMapping::Enhanced,
            ..Self::default()
        }
    }

    /// The DDS type names for `interface`, those the options carry or
    /// `${interface}_Request` / `_Reply`.
    #[must_use]
    pub fn type_names_for(&self, interface: &str) -> TopicTypeNames {
        self.type_names
            .clone()
            .unwrap_or_else(|| TopicTypeNames::for_interface("", interface))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §7.5.1.1.6 rule 1 and §7.5.1.1.7 rule 1.
    #[test]
    fn the_type_names_are_the_interface_name_with_the_suffix() {
        let names = TopicTypeNames::for_interface("", "Calculator");
        assert_eq!(names.request, "Calculator_Request");
        assert_eq!(names.reply, "Calculator_Reply");
        let scoped = TopicTypeNames::for_interface("robot::arm", "Calculator");
        assert_eq!(scoped.request, "robot::arm::Calculator_Request");
        assert_eq!(scoped.reply, "robot::arm::Calculator_Reply");
    }

    #[test]
    fn the_defaults_are_the_spec_readings() {
        let options = RpcEndpointOptions::default();
        assert_eq!(options.mapping, RpcMapping::Basic);
        assert_eq!(options.wire_encoding, WireEncoding::Xcdr1);
        assert!(options.xcdr1());
        assert!(!RpcEndpointOptions::enhanced().xcdr1());
        assert_eq!(options.service_matching, ServiceMatching::Queue);
        assert_eq!(options.client_matching, ClientMatching::Hold);
        assert!(!options.accept_unannounced);
        assert_eq!(options.remote_ex_member, "remoteEx");
        assert_eq!(
            RpcEndpointOptions::default().type_names_for("Calc").request,
            "Calc_Request"
        );
    }

    #[test]
    fn the_mapping_of_a_generated_binding_is_a_setting() {
        let options =
            RpcEndpointOptions::for_interface("demo", "Calc").with_mapping(RpcMapping::Enhanced);
        assert_eq!(options.mapping, RpcMapping::Enhanced);
        assert!(!options.xcdr1());
        assert_eq!(options.type_names_for("x").request, "demo::Calc_Request");
    }

    #[test]
    fn a_generated_binding_names_the_scope_and_the_parents() {
        let options = RpcEndpointOptions::for_interface("demo", "Derived")
            .with_parent("demo_Base")
            .with_parent("demo_Other");
        assert_eq!(
            options.type_names_for("ignored").reply,
            "demo::Derived_Reply"
        );
        assert_eq!(options.parents, ["demo_Base", "demo_Other"]);
    }
}
