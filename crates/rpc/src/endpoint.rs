// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RPC endpoint helper — Spec §7.6.2.
//!
//! An RPC endpoint is a tuple of two DDS endpoints:
//!
//! * **Requester**: request writer (sends requests) + reply reader (reads
//!   replies).
//! * **Replier**: request reader (receives requests) + reply writer
//!   (sends replies).
//!
//! Spec §7.6.2 requires that the two related endpoints are recognizable
//! as a logical bundle in SEDP discovery. We set for this:
//!
//! * `PID_SERVICE_INSTANCE_NAME` (0x0080) on both endpoints — service
//!   identification.
//! * `PID_RELATED_ENTITY_GUID` (0x0081) — on each endpoint this
//!   PID points to the GUID of the counterpart endpoint (request writer ↔ reply reader
//!   for the requester; request reader ↔ reply writer for the replier).
//! * `PID_TOPIC_ALIASES` (0x0082) optional — alternative topic names.
//!
//! This stage (C6.1.B) only builds the **static construction**:
//! assign GUIDs, derive topic names, enter discovery PIDs into
//! [`PublicationBuiltinTopicData`] / [`SubscriptionBuiltinTopicData`].
//! Threading, history caches and correlation via
//! `PID_RELATED_SAMPLE_IDENTITY` are C6.1.C.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

use crate::error::{RpcError, RpcResult};
use crate::service_mapping::ServiceDef;
use crate::topic_naming::ServiceTopicNames;

// ---------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------

/// Builder for an RPC endpoint pair (requester _or_ replier).
#[derive(Debug, Clone)]
pub struct RpcEndpointBuilder {
    service: ServiceDef,
    instance_name: Option<String>,
    topic_aliases: Vec<String>,
    participant_prefix: GuidPrefix,
    request_entity: EntityId,
    reply_entity: EntityId,
    type_name_request: String,
    type_name_reply: String,
}

impl RpcEndpointBuilder {
    /// New builder for a service.
    ///
    /// `participant_prefix` is the GUID prefix of the hosting participant.
    /// `request_entity`/`reply_entity` are the EntityIds assigned internally
    /// for the request and reply endpoint respectively — the caller
    /// (DCPS layer) must ensure they are unique.
    ///
    /// # Errors
    /// `RpcError::InvalidServiceName` if the service name does not
    /// correspond to an IDL identifier.
    pub fn new(
        service: ServiceDef,
        participant_prefix: GuidPrefix,
        request_entity: EntityId,
        reply_entity: EntityId,
    ) -> RpcResult<Self> {
        // Validation round-trip via `topic_names()` — throws
        // InvalidServiceName if the name is empty/illegal.
        let _ = service.topic_names()?;
        let type_name_request = alloc::format!("{}_Request", service.name);
        let type_name_reply = alloc::format!("{}_Reply", service.name);
        Ok(Self {
            service,
            instance_name: None,
            topic_aliases: Vec::new(),
            participant_prefix,
            request_entity,
            reply_entity,
            type_name_request,
            type_name_reply,
        })
    }

    /// Sets the `PID_SERVICE_INSTANCE_NAME` (DDS-RPC §7.8.2). Allows
    /// addressing multiple instances of the same service type on one
    /// participant disjointly (e.g. `"calc-A"` vs. `"calc-B"`).
    #[must_use]
    pub fn instance_name(mut self, name: impl Into<String>) -> Self {
        self.instance_name = Some(name.into());
        self
    }

    /// Sets `PID_TOPIC_ALIASES`. Order is preserved.
    #[must_use]
    pub fn topic_aliases(mut self, aliases: Vec<String>) -> Self {
        self.topic_aliases = aliases;
        self
    }

    /// Override the type name of the request wire structure derived by
    /// default (`<Service>_Request`).
    #[must_use]
    pub fn request_type_name(mut self, n: impl Into<String>) -> Self {
        self.type_name_request = n.into();
        self
    }

    /// Override the type name of the reply wire structure derived by
    /// default.
    #[must_use]
    pub fn reply_type_name(mut self, n: impl Into<String>) -> Self {
        self.type_name_reply = n.into();
        self
    }

    /// Builds the requester pair: request **writer** discovery + reply
    /// **reader** discovery, with correctly cross-referenced `RELATED_ENTITY_GUID`
    /// references.
    ///
    /// # Errors
    /// `RpcError::EmptyService` if the service has no methods
    /// (nothing to transport).
    pub fn build_requester(&self) -> RpcResult<RequesterEndpoint> {
        self.check_non_empty()?;
        let topics = self.service.topic_names()?;
        let req_writer_guid = Guid::new(self.participant_prefix, self.request_entity);
        let rep_reader_guid = Guid::new(self.participant_prefix, self.reply_entity);

        let request_writer = self.publication(
            &topics,
            req_writer_guid,
            /*related=*/ rep_reader_guid,
            /*request_side=*/ true,
        );
        let reply_reader = self.subscription(
            &topics,
            rep_reader_guid,
            /*related=*/ req_writer_guid,
            /*request_side=*/ false,
        );
        Ok(RequesterEndpoint {
            request_writer,
            reply_reader,
        })
    }

    /// Builds the replier pair: request **reader** discovery + reply
    /// **writer** discovery.
    ///
    /// # Errors
    /// See [`Self::build_requester`].
    pub fn build_replier(&self) -> RpcResult<ReplierEndpoint> {
        self.check_non_empty()?;
        let topics = self.service.topic_names()?;
        let req_reader_guid = Guid::new(self.participant_prefix, self.request_entity);
        let rep_writer_guid = Guid::new(self.participant_prefix, self.reply_entity);

        let request_reader = self.subscription(
            &topics,
            req_reader_guid,
            /*related=*/ rep_writer_guid,
            /*request_side=*/ true,
        );
        let reply_writer = self.publication(
            &topics,
            rep_writer_guid,
            /*related=*/ req_reader_guid,
            /*request_side=*/ false,
        );
        Ok(ReplierEndpoint {
            request_reader,
            reply_writer,
        })
    }

    fn check_non_empty(&self) -> RpcResult<()> {
        if self.service.methods.is_empty() {
            return Err(RpcError::EmptyService(self.service.name.clone()));
        }
        Ok(())
    }

    fn publication(
        &self,
        topics: &ServiceTopicNames,
        my_guid: Guid,
        related: Guid,
        request_side: bool,
    ) -> PublicationBuiltinTopicData {
        let (topic_name, type_name) = if request_side {
            (topics.request.clone(), self.type_name_request.clone())
        } else {
            (topics.reply.clone(), self.type_name_reply.clone())
        };
        PublicationBuiltinTopicData {
            key: my_guid,
            participant_key: Guid::new(self.participant_prefix, EntityId::PARTICIPANT),
            topic_name,
            type_name,
            durability: zerodds_rtps::publication_data::DurabilityKind::default(),
            reliability: zerodds_rtps::publication_data::ReliabilityQos::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: Vec::new(),
            user_data: Vec::new(),
            topic_data: Vec::new(),
            group_data: Vec::new(),
            type_information: None,
            data_representation: Vec::new(),
            security_info: None,
            service_instance_name: self.instance_name.clone(),
            related_entity_guid: Some(related),
            topic_aliases: if self.topic_aliases.is_empty() {
                None
            } else {
                Some(self.topic_aliases.clone())
            },
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
        }
    }

    fn subscription(
        &self,
        topics: &ServiceTopicNames,
        my_guid: Guid,
        related: Guid,
        request_side: bool,
    ) -> SubscriptionBuiltinTopicData {
        let (topic_name, type_name) = if request_side {
            (topics.request.clone(), self.type_name_request.clone())
        } else {
            (topics.reply.clone(), self.type_name_reply.clone())
        };
        SubscriptionBuiltinTopicData {
            key: my_guid,
            participant_key: Guid::new(self.participant_prefix, EntityId::PARTICIPANT),
            topic_name,
            type_name,
            durability: zerodds_rtps::publication_data::DurabilityKind::default(),
            reliability: zerodds_rtps::publication_data::ReliabilityQos::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: Vec::new(),
            user_data: Vec::new(),
            topic_data: Vec::new(),
            group_data: Vec::new(),
            type_information: None,
            data_representation: Vec::new(),
            content_filter: None,
            security_info: None,
            service_instance_name: self.instance_name.clone(),
            related_entity_guid: Some(related),
            topic_aliases: if self.topic_aliases.is_empty() {
                None
            } else {
                Some(self.topic_aliases.clone())
            },
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------
// Endpoint-Pair-Strukturen
// ---------------------------------------------------------------------

/// Requester-Pair (Request-Writer + Reply-Reader).
#[derive(Debug, Clone, PartialEq)]
pub struct RequesterEndpoint {
    /// SEDP-Daten des Request-Writers.
    pub request_writer: PublicationBuiltinTopicData,
    /// SEDP-Daten des Reply-Readers.
    pub reply_reader: SubscriptionBuiltinTopicData,
}

/// Replier-Pair (Request-Reader + Reply-Writer).
#[derive(Debug, Clone, PartialEq)]
pub struct ReplierEndpoint {
    /// SEDP-Daten des Request-Readers.
    pub request_reader: SubscriptionBuiltinTopicData,
    /// SEDP-Daten des Reply-Writers.
    pub reply_writer: PublicationBuiltinTopicData,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::annotations::lower_rpc_annotations;
    use crate::service_mapping::lower_service;
    use zerodds_idl::ast::{
        Annotation, AnnotationParams, Export, Identifier, IntegerType, InterfaceDef, InterfaceKind,
        OpDecl, ParamAttribute, ParamDecl, PrimitiveType, ScopedName, TypeSpec,
    };
    use zerodds_idl::errors::Span;

    fn sp() -> Span {
        Span::SYNTHETIC
    }

    fn ident(t: &str) -> Identifier {
        Identifier::new(t, sp())
    }

    fn long_t() -> TypeSpec {
        TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long))
    }

    fn ann_simple(name: &str) -> Annotation {
        Annotation {
            name: ScopedName {
                absolute: false,
                parts: alloc::vec![ident(name)],
                span: sp(),
            },
            params: AnnotationParams::None,
            span: sp(),
        }
    }

    fn calc_service() -> ServiceDef {
        let add = OpDecl {
            name: ident("add"),
            oneway: false,
            return_type: Some(long_t()),
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: long_t(),
                name: ident("a"),
                annotations: Vec::new(),
                span: sp(),
            }],
            raises: Vec::new(),
            context: Vec::new(),
            annotations: Vec::new(),
            span: sp(),
        };
        let i = InterfaceDef {
            kind: InterfaceKind::Plain,
            name: ident("Calculator"),
            bases: Vec::new(),
            exports: alloc::vec![Export::Op(add)],
            annotations: alloc::vec![ann_simple("service")],
            span: sp(),
        };
        let lowered = lower_rpc_annotations(&i.annotations);
        lower_service(&i, &lowered).unwrap()
    }

    fn pp() -> GuidPrefix {
        GuidPrefix::from_bytes([0x99; 12])
    }

    fn req_eid() -> EntityId {
        EntityId::user_writer_with_key([0xA0, 0xA1, 0xA2])
    }

    fn rep_eid() -> EntityId {
        EntityId::user_reader_with_key([0xB0, 0xB1, 0xB2])
    }

    #[test]
    fn requester_topic_names_match_service() {
        let svc = calc_service();
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid()).unwrap();
        let r = b.build_requester().unwrap();
        assert_eq!(r.request_writer.topic_name, "Calculator_Request");
        assert_eq!(r.reply_reader.topic_name, "Calculator_Reply");
    }

    #[test]
    fn requester_related_entity_guids_cross_link() {
        let svc = calc_service();
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid()).unwrap();
        let r = b.build_requester().unwrap();
        // Request-Writer.related → Reply-Reader.guid
        assert_eq!(
            r.request_writer.related_entity_guid,
            Some(r.reply_reader.key)
        );
        // Reply-Reader.related → Request-Writer.guid
        assert_eq!(
            r.reply_reader.related_entity_guid,
            Some(r.request_writer.key)
        );
    }

    #[test]
    fn replier_topic_names_match_service() {
        let svc = calc_service();
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid()).unwrap();
        let r = b.build_replier().unwrap();
        assert_eq!(r.request_reader.topic_name, "Calculator_Request");
        assert_eq!(r.reply_writer.topic_name, "Calculator_Reply");
    }

    #[test]
    fn replier_related_entity_guids_cross_link() {
        let svc = calc_service();
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid()).unwrap();
        let r = b.build_replier().unwrap();
        assert_eq!(
            r.request_reader.related_entity_guid,
            Some(r.reply_writer.key)
        );
        assert_eq!(
            r.reply_writer.related_entity_guid,
            Some(r.request_reader.key)
        );
    }

    #[test]
    fn instance_name_propagated_to_both_sides() {
        let svc = calc_service();
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid())
            .unwrap()
            .instance_name("calc-A");
        let r = b.build_requester().unwrap();
        assert_eq!(
            r.request_writer.service_instance_name.as_deref(),
            Some("calc-A")
        );
        assert_eq!(
            r.reply_reader.service_instance_name.as_deref(),
            Some("calc-A")
        );
        let p = b.build_replier().unwrap();
        assert_eq!(
            p.request_reader.service_instance_name.as_deref(),
            Some("calc-A")
        );
        assert_eq!(
            p.reply_writer.service_instance_name.as_deref(),
            Some("calc-A")
        );
    }

    #[test]
    fn topic_aliases_propagated_to_both_sides() {
        let svc = calc_service();
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid())
            .unwrap()
            .topic_aliases(alloc::vec!["LegacyCalc_Request".into()]);
        let r = b.build_requester().unwrap();
        assert_eq!(
            r.request_writer.topic_aliases.as_deref(),
            Some(alloc::vec!["LegacyCalc_Request".to_string()].as_slice())
        );
    }

    #[test]
    fn topic_aliases_empty_yields_none() {
        let svc = calc_service();
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid()).unwrap();
        let r = b.build_requester().unwrap();
        assert!(r.request_writer.topic_aliases.is_none());
    }

    #[test]
    fn type_names_default_to_service_request_reply() {
        let svc = calc_service();
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid()).unwrap();
        let r = b.build_requester().unwrap();
        assert_eq!(r.request_writer.type_name, "Calculator_Request");
        assert_eq!(r.reply_reader.type_name, "Calculator_Reply");
    }

    #[test]
    fn type_name_overrides_apply() {
        let svc = calc_service();
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid())
            .unwrap()
            .request_type_name("X")
            .reply_type_name("Y");
        let r = b.build_requester().unwrap();
        assert_eq!(r.request_writer.type_name, "X");
        assert_eq!(r.reply_reader.type_name, "Y");
    }

    #[test]
    fn empty_service_yields_error() {
        let svc = ServiceDef {
            name: "Empty".into(),
            methods: Vec::new(),
        };
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid()).unwrap();
        let err = b.build_requester().unwrap_err();
        assert!(matches!(err, RpcError::EmptyService(ref n) if n == "Empty"));
        let err = b.build_replier().unwrap_err();
        assert!(matches!(err, RpcError::EmptyService(_)));
    }

    #[test]
    fn invalid_service_name_rejected_in_builder_new() {
        let svc = ServiceDef {
            name: String::new(),
            methods: Vec::new(),
        };
        let err = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid()).unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    #[test]
    fn participant_key_uses_participant_entity_id() {
        let svc = calc_service();
        let b = RpcEndpointBuilder::new(svc, pp(), req_eid(), rep_eid()).unwrap();
        let r = b.build_requester().unwrap();
        assert_eq!(
            r.request_writer.participant_key.entity_id,
            EntityId::PARTICIPANT
        );
        assert_eq!(r.request_writer.participant_key.prefix, pp());
    }
}
