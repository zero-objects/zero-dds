// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RPC discovery extensions and the Enhanced service-discovery algorithm
//! (Spec §7.6.2).
//!
//! The Basic Service Mapping uses plain DDS discovery (§7.6.1). The
//! Enhanced one extends `PublicationBuiltinTopicData` and
//! `SubscriptionBuiltinTopicData` with three members (§7.6.2.1) and adds
//! two rules (§7.6.2.2) that keep a reply from being lost to a discovery
//! race:
//!
//! * the client does not send a request until it has discovered both
//!   endpoints of the service — its reply reader the service's reply
//!   writer, its request writer the service's request reader
//!   ([`service_matching_complete`], Figure 7.4);
//! * the service does not accept a request from a client until it has
//!   discovered both endpoints of the client
//!   ([`client_matching_complete`], Figure 7.3).
//!
//! The announcement goes through `zerodds_dcps`
//! (`DataWriter::set_rpc_discovery`, `DataReader::set_rpc_discovery`) and
//! the peers' announcements come back as
//! [`zerodds_dcps::runtime::RpcPeer`]; this module holds the spec's data
//! model and the algorithms over it.
//!
//! # Spec mapping
//!
//! * **§7.6.2.1.1** [`PublicationBuiltinTopicDataExt`].
//! * **§7.6.2.1.2** [`SubscriptionBuiltinTopicDataExt`].
//! * **§7.6.2.2.1** [`client_matching_complete`].
//! * **§7.6.2.2.2** [`service_matching_complete`].

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_dcps::runtime::{RpcDiscoveryExt, RpcPeer};

/// `GUID_UNKNOWN` of RTPS 2.5 §9.3.1.5: what an absent
/// `related_datareader_key` / `related_datawriter_key` reads as.
pub const GUID_UNKNOWN: [u8; 16] = [0; 16];

/// Spec §7.6.2.1.1: the extension of `PublicationBuiltinTopicData`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublicationBuiltinTopicDataExt {
    /// `service_instance_name` (`@ID(0x0080) string<256>`): "the
    /// instance name specified by the user at the service-side"; the
    /// client-side DataWriter does not include it.
    pub service_instance_name: Option<String>,
    /// `related_datareader_key` (`@ID(0x0081) GUID_t`): of the client's
    /// request writer, its reply reader; of the service's reply writer,
    /// its request reader. `GUID_UNKNOWN` when absent.
    pub related_datareader_key: [u8; 16],
    /// `topic_aliases` (`@ID(0x0082) sequence<string<256>>`), as §7.4.3
    /// defines them. Empty when absent.
    pub topic_aliases: Vec<String>,
}

/// Spec §7.6.2.1.2: the extension of `SubscriptionBuiltinTopicData`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriptionBuiltinTopicDataExt {
    /// `service_instance_name` (`@ID(0x0080) string<256>`); the
    /// client-side DataReader does not include it.
    pub service_instance_name: Option<String>,
    /// `related_datawriter_key` (`@ID(0x0081) GUID_t`): of the client's
    /// reply reader, its request writer; of the service's request reader,
    /// its reply writer. `GUID_UNKNOWN` when absent.
    pub related_datawriter_key: [u8; 16],
    /// `topic_aliases` (`@ID(0x0082) sequence<string<256>>`).
    pub topic_aliases: Vec<String>,
}

impl PublicationBuiltinTopicDataExt {
    /// The announcement this extension stands for.
    #[must_use]
    pub fn announcement(&self) -> RpcDiscoveryExt {
        RpcDiscoveryExt {
            service_instance_name: self.service_instance_name.clone(),
            related_entity_guid: (self.related_datareader_key != GUID_UNKNOWN)
                .then_some(self.related_datareader_key),
            topic_aliases: (!self.topic_aliases.is_empty()).then(|| self.topic_aliases.clone()),
        }
    }

    /// The extension a matched publication announced.
    #[must_use]
    pub fn from_peer(peer: &RpcPeer) -> Self {
        Self {
            service_instance_name: peer.ext.service_instance_name.clone(),
            related_datareader_key: peer.ext.related_entity_guid.unwrap_or(GUID_UNKNOWN),
            topic_aliases: peer.ext.topic_aliases.clone().unwrap_or_default(),
        }
    }
}

impl SubscriptionBuiltinTopicDataExt {
    /// The announcement this extension stands for.
    #[must_use]
    pub fn announcement(&self) -> RpcDiscoveryExt {
        RpcDiscoveryExt {
            service_instance_name: self.service_instance_name.clone(),
            related_entity_guid: (self.related_datawriter_key != GUID_UNKNOWN)
                .then_some(self.related_datawriter_key),
            topic_aliases: (!self.topic_aliases.is_empty()).then(|| self.topic_aliases.clone()),
        }
    }

    /// The extension a matched subscription announced.
    #[must_use]
    pub fn from_peer(peer: &RpcPeer) -> Self {
        Self {
            service_instance_name: peer.ext.service_instance_name.clone(),
            related_datawriter_key: peer.ext.related_entity_guid.unwrap_or(GUID_UNKNOWN),
            topic_aliases: peer.ext.topic_aliases.clone().unwrap_or_default(),
        }
    }
}

/// Whether `peer` named `counterpart` as its related endpoint — or, where
/// `accept_unannounced` is set, named none at all.
fn names(peer: &RpcPeer, counterpart: &[u8; 16], accept_unannounced: bool) -> bool {
    match peer.ext.related_entity_guid {
        Some(related) => &related == counterpart,
        None => accept_unannounced,
    }
}

/// Spec §7.6.2.2.2, Figure 7.4: has the client discovered both endpoints
/// of a service instance it may address?
///
/// `request_readers` are the readers the client's request writer is
/// matched with, `reply_writers` the writers its reply reader is matched
/// with. A service instance is complete when one of the readers and one
/// of the writers name each other (`related_datawriter_key` of the reader
/// is the writer, `related_datareader_key` of the writer is the reader)
/// and the reader announced the client's `instance_name` (the empty name
/// is the service that announced none).
///
/// A peer that announced no `related_entity_guid`, a service that does
/// not speak the extension, never completes — the algorithm cannot tell
/// which of its endpoints belong together — unless `accept_unannounced`
/// reads such a pair as one by the DDS match alone.
#[must_use]
pub fn service_matching_complete(
    request_readers: &[RpcPeer],
    reply_writers: &[RpcPeer],
    instance_name: &str,
    accept_unannounced: bool,
) -> bool {
    request_readers.iter().any(|reader| {
        let announced = reader.ext.service_instance_name.as_deref().unwrap_or("");
        announced == instance_name
            && reply_writers.iter().any(|writer| {
                names(reader, &writer.key, accept_unannounced)
                    && names(writer, &reader.key, accept_unannounced)
            })
    })
}

/// Spec §7.6.2.2.1, Figure 7.3: has the service discovered both
/// endpoints of the client whose request writer is `client_writer`?
///
/// `reply_readers` are the readers the service's reply writer is matched
/// with. The client's request writer names its reply reader in
/// `related_datareader_key`; the service has discovered the client when
/// that reader is among `reply_readers`. Until then the service does not
/// process the client's requests (`local_mask = All_DW`); afterwards it
/// unmasks that one writer.
///
/// A client that announced no related endpoint is unmasked only when
/// `accept_unannounced` is set.
#[must_use]
pub fn client_matching_complete(
    client_writer: &RpcPeer,
    reply_readers: &[RpcPeer],
    accept_unannounced: bool,
) -> bool {
    match client_writer.ext.related_entity_guid {
        Some(reader) => reply_readers.iter().any(|candidate| {
            candidate.key == reader && names(candidate, &client_writer.key, accept_unannounced)
        }),
        None => accept_unannounced,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const REQUEST_READER: [u8; 16] = [1; 16];
    const REPLY_WRITER: [u8; 16] = [2; 16];
    const REQUEST_WRITER: [u8; 16] = [3; 16];
    const REPLY_READER: [u8; 16] = [4; 16];

    fn peer(key: [u8; 16], related: Option<[u8; 16]>, instance: Option<&str>) -> RpcPeer {
        RpcPeer {
            key,
            ext: RpcDiscoveryExt {
                service_instance_name: instance.map(str::to_string),
                related_entity_guid: related,
                topic_aliases: None,
            },
        }
    }

    fn service_reader(instance: Option<&str>) -> RpcPeer {
        peer(REQUEST_READER, Some(REPLY_WRITER), instance)
    }

    fn service_writer(instance: Option<&str>) -> RpcPeer {
        peer(REPLY_WRITER, Some(REQUEST_READER), instance)
    }

    /// Figure 7.4: with neither service endpoint discovered, with only
    /// the request reader, with only the reply writer — not complete;
    /// with both — complete.
    #[test]
    fn the_client_waits_for_both_service_endpoints() {
        assert!(!service_matching_complete(&[], &[], "", false));
        assert!(!service_matching_complete(
            &[service_reader(None)],
            &[],
            "",
            false
        ));
        assert!(!service_matching_complete(
            &[],
            &[service_writer(None)],
            "",
            false
        ));
        assert!(service_matching_complete(
            &[service_reader(None)],
            &[service_writer(None)],
            "",
            false
        ));
    }

    /// A reply writer of another service instance is not the one the
    /// request reader names.
    #[test]
    fn the_two_endpoints_must_name_each_other() {
        let other_writer = peer([9; 16], Some([8; 16]), None);
        assert!(!service_matching_complete(
            &[service_reader(None)],
            &[other_writer],
            "",
            false
        ));
    }

    /// §7.6.2.1.1: the instance name is the service's, announced by the
    /// service; the client addresses the instance of its own name.
    #[test]
    fn the_client_addresses_the_instance_it_names() {
        let readers = [service_reader(Some("calc-A"))];
        let writers = [service_writer(Some("calc-A"))];
        assert!(service_matching_complete(
            &readers, &writers, "calc-A", false
        ));
        assert!(!service_matching_complete(
            &readers, &writers, "calc-B", false
        ));
        assert!(!service_matching_complete(&readers, &writers, "", false));
    }

    /// A service that announced no extension never completes under the
    /// spec's algorithm, and does under the named alternative.
    #[test]
    fn an_unannounced_service_completes_only_when_accepted() {
        let reader = peer(REQUEST_READER, None, None);
        let writer = peer(REPLY_WRITER, None, None);
        assert!(!service_matching_complete(
            core::slice::from_ref(&reader),
            core::slice::from_ref(&writer),
            "",
            false
        ));
        assert!(service_matching_complete(&[reader], &[writer], "", true));
    }

    /// Figure 7.3: the service unmasks a client's request writer when its
    /// reply writer has discovered the reader that writer names.
    #[test]
    fn the_service_unmasks_a_client_once_both_its_endpoints_are_discovered() {
        let client_writer = peer(REQUEST_WRITER, Some(REPLY_READER), None);
        assert!(!client_matching_complete(&client_writer, &[], false));
        let client_reader = peer(REPLY_READER, Some(REQUEST_WRITER), None);
        assert!(client_matching_complete(
            &client_writer,
            &[client_reader],
            false
        ));
        let someone_else = peer([7; 16], Some(REQUEST_WRITER), None);
        assert!(!client_matching_complete(
            &client_writer,
            &[someone_else],
            false
        ));
    }

    #[test]
    fn an_unannounced_client_is_unmasked_only_when_accepted() {
        let client_writer = peer(REQUEST_WRITER, None, None);
        assert!(!client_matching_complete(&client_writer, &[], false));
        assert!(client_matching_complete(&client_writer, &[], true));
    }

    /// §7.6.2.1: an absent related key reads as `GUID_UNKNOWN`.
    #[test]
    fn an_absent_related_key_reads_as_guid_unknown() {
        let none = peer(REQUEST_READER, None, None);
        assert_eq!(
            PublicationBuiltinTopicDataExt::from_peer(&none).related_datareader_key,
            GUID_UNKNOWN
        );
        assert_eq!(
            SubscriptionBuiltinTopicDataExt::from_peer(&none).related_datawriter_key,
            GUID_UNKNOWN
        );
        assert_eq!(
            PublicationBuiltinTopicDataExt::default().announcement(),
            RpcDiscoveryExt::default()
        );
    }

    #[test]
    fn the_extension_carries_the_three_members_of_the_spec() {
        let publication = PublicationBuiltinTopicDataExt {
            service_instance_name: Some("calc".into()),
            related_datareader_key: REQUEST_READER,
            topic_aliases: alloc::vec!["Base_Service_Request".into()],
        };
        let announced = publication.announcement();
        assert_eq!(announced.service_instance_name.as_deref(), Some("calc"));
        assert_eq!(announced.related_entity_guid, Some(REQUEST_READER));
        assert_eq!(
            announced.topic_aliases,
            Some(alloc::vec!["Base_Service_Request".to_string()])
        );
        let subscription = SubscriptionBuiltinTopicDataExt {
            service_instance_name: None,
            related_datawriter_key: REPLY_WRITER,
            topic_aliases: Vec::new(),
        };
        assert_eq!(
            subscription.announcement().related_entity_guid,
            Some(REPLY_WRITER)
        );
        assert_eq!(subscription.announcement().topic_aliases, None);
    }
}
