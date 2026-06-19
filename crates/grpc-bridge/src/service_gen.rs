// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Auto-generation of gRPC service definitions per DDS topic.
//!
//! Spec: `zerodds-grpc-bridge-1.0.md` §4.2.
//!
//! Per DDS topic, the bridge generates **one** gRPC service with
//! exactly two methods:
//!
//! ```text
//! service <TopicSlug>Stream {
//!   rpc Publish(stream Sample) returns (PublishAck);
//!   rpc Subscribe(SubscribeReq) returns (stream Sample);
//! }
//! ```
//!
//! The slug is a `PascalCase` sanitization of the DDS topic name
//! (dots/slashes/hyphens → word boundaries). Method paths follow the
//! gRPC scheme `/<package>.<TopicSlug>Stream/<Method>`.

use alloc::string::String;
use alloc::vec::Vec;

/// Default package for ZeroDDS-generated services.
pub const DEFAULT_PACKAGE: &str = "zerodds.bridge.v1";

/// Service definition for a single DDS topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicService {
    /// gRPC package (e.g. `zerodds.bridge.v1`).
    pub package: String,
    /// Service name (e.g. `TradeStream`).
    pub service: String,
    /// Original DDS topic name.
    pub dds_topic: String,
    /// DDS type name.
    pub dds_type: String,
}

impl TopicService {
    /// Generates a service definition from a DDS topic name.
    #[must_use]
    pub fn from_topic(dds_topic: &str, dds_type: &str) -> Self {
        Self {
            package: DEFAULT_PACKAGE.into(),
            service: format!("{}Stream", topic_to_pascal(dds_topic)),
            dds_topic: dds_topic.into(),
            dds_type: dds_type.into(),
        }
    }

    /// gRPC path for the `Publish` method.
    #[must_use]
    pub fn publish_path(&self) -> String {
        format!("/{}.{}/Publish", self.package, self.service)
    }

    /// gRPC path for the `Subscribe` method.
    #[must_use]
    pub fn subscribe_path(&self) -> String {
        format!("/{}.{}/Subscribe", self.package, self.service)
    }

    /// Is `path` one of the two method paths?
    #[must_use]
    pub fn matches_path(&self, path: &str) -> Option<MethodKind> {
        if path == self.publish_path() {
            Some(MethodKind::Publish)
        } else if path == self.subscribe_path() {
            Some(MethodKind::Subscribe)
        } else {
            None
        }
    }
}

/// Which method a gRPC path addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    /// Client → Server: stream Sample → PublishAck.
    Publish,
    /// Client → Server: SubscribeReq → stream Sample.
    Subscribe,
}

/// Sanitize a DDS topic name to PascalCase.
/// `Trade` ⇒ `Trade`, `dds.demo.Trade` ⇒ `DdsDemoTrade`,
/// `chat-room/foo_bar` ⇒ `ChatRoomFooBar`.
#[must_use]
pub fn topic_to_pascal(topic: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in topic.chars() {
        if c == '.' || c == '/' || c == '-' || c == '_' || c == ' ' {
            up = true;
            continue;
        }
        if up {
            for u in c.to_uppercase() {
                out.push(u);
            }
            up = false;
        } else {
            out.push(c);
        }
    }
    if out.is_empty() {
        out.push_str("Topic");
    }
    out
}

/// Renderer: returns the proto source of the service.
/// Spec §4.2 — for the reflection service path.
#[must_use]
pub fn render_proto(svc: &TopicService) -> String {
    let mut out = String::new();
    out.push_str("syntax = \"proto3\";\n");
    out.push_str(&format!("package {};\n\n", svc.package));
    out.push_str("message Sample { bytes payload = 1; }\n");
    out.push_str("message PublishAck { uint64 accepted = 1; }\n");
    out.push_str("message SubscribeReq { string filter = 1; }\n\n");
    out.push_str(&format!("service {} {{\n", svc.service));
    out.push_str("  rpc Publish(stream Sample) returns (PublishAck);\n");
    out.push_str("  rpc Subscribe(SubscribeReq) returns (stream Sample);\n");
    out.push_str("}\n");
    out
}

/// Catalog of all registered topic services. Spec §5.2 — type
/// discovery via reflection is mapped onto this.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceCatalog {
    services: Vec<TopicService>,
}

impl ServiceCatalog {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a topic service.
    pub fn register(&mut self, svc: TopicService) {
        if !self.services.iter().any(|s| s.service == svc.service) {
            self.services.push(svc);
        }
    }

    /// Iterate over all services.
    pub fn iter(&self) -> impl Iterator<Item = &TopicService> {
        self.services.iter()
    }

    /// Number of services.
    #[must_use]
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// Empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    /// Look up by `service` name (e.g. `TradeStream`).
    #[must_use]
    pub fn find_by_service(&self, service: &str) -> Option<&TopicService> {
        self.services.iter().find(|s| s.service == service)
    }

    /// Look up by gRPC path (e.g. `/zerodds.bridge.v1.TradeStream/Publish`).
    #[must_use]
    pub fn find_by_path(&self, path: &str) -> Option<(&TopicService, MethodKind)> {
        for s in &self.services {
            if let Some(k) = s.matches_path(path) {
                return Some((s, k));
            }
        }
        None
    }

    /// All fully-qualified service names (`<package>.<service>`).
    /// Spec §4.6 — reflection ListServices response.
    #[must_use]
    pub fn fully_qualified_service_names(&self) -> Vec<String> {
        self.services
            .iter()
            .map(|s| format!("{}.{}", s.package, s.service))
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn pascal_simple() {
        assert_eq!(topic_to_pascal("Trade"), "Trade");
    }

    #[test]
    fn pascal_dotted() {
        assert_eq!(topic_to_pascal("dds.demo.Trade"), "DdsDemoTrade");
    }

    #[test]
    fn pascal_mixed_separators() {
        assert_eq!(topic_to_pascal("chat-room/foo_bar"), "ChatRoomFooBar");
    }

    #[test]
    fn pascal_empty_falls_back() {
        assert_eq!(topic_to_pascal(""), "Topic");
    }

    #[test]
    fn topic_service_paths() {
        let s = TopicService::from_topic("Trade", "Trade");
        assert_eq!(s.publish_path(), "/zerodds.bridge.v1.TradeStream/Publish");
        assert_eq!(
            s.subscribe_path(),
            "/zerodds.bridge.v1.TradeStream/Subscribe"
        );
    }

    #[test]
    fn matches_path_publish() {
        let s = TopicService::from_topic("Trade", "Trade");
        assert_eq!(
            s.matches_path("/zerodds.bridge.v1.TradeStream/Publish"),
            Some(MethodKind::Publish)
        );
        assert_eq!(
            s.matches_path("/zerodds.bridge.v1.TradeStream/Subscribe"),
            Some(MethodKind::Subscribe)
        );
        assert_eq!(s.matches_path("/wrong"), None);
    }

    #[test]
    fn render_proto_contains_methods() {
        let s = TopicService::from_topic("Trade", "Trade");
        let proto = render_proto(&s);
        assert!(proto.contains("service TradeStream"));
        assert!(proto.contains("rpc Publish(stream Sample)"));
        assert!(proto.contains("rpc Subscribe(SubscribeReq)"));
    }

    #[test]
    fn catalog_registration_dedup() {
        let mut cat = ServiceCatalog::new();
        cat.register(TopicService::from_topic("Trade", "Trade"));
        cat.register(TopicService::from_topic("Trade", "Trade"));
        assert_eq!(cat.len(), 1);
    }

    #[test]
    fn catalog_find_by_path() {
        let mut cat = ServiceCatalog::new();
        cat.register(TopicService::from_topic("Trade", "Trade"));
        let (svc, kind) = cat
            .find_by_path("/zerodds.bridge.v1.TradeStream/Subscribe")
            .expect("found");
        assert_eq!(svc.service, "TradeStream");
        assert_eq!(kind, MethodKind::Subscribe);
    }

    #[test]
    fn catalog_fully_qualified_names() {
        let mut cat = ServiceCatalog::new();
        cat.register(TopicService::from_topic("Trade", "Trade"));
        cat.register(TopicService::from_topic("Quote", "Quote"));
        let names = cat.fully_qualified_service_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"zerodds.bridge.v1.TradeStream".into()));
        assert!(names.contains(&"zerodds.bridge.v1.QuoteStream".into()));
    }
}
