// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Auto-generation of gRPC service definitions per DDS topic.
//!
//! Spec: `zerodds-grpc-bridge-1.0.md` §4.2.
//!
//! Per DDS topic, the bridge generates **one** gRPC service with four
//! methods and the messages of [`crate::messages`]:
//!
//! ```text
//! service <TopicSlug>Stream {
//!   rpc Publish(stream Sample) returns (PublishAck);
//!   rpc Subscribe(SubscribeReq) returns (stream Sample);
//!   rpc PublishOne(Sample) returns (PublishAck);
//!   rpc Catalog(CatalogReq) returns (CatalogResp);
//! }
//! ```
//!
//! The default service name and package of spec §5.1: the topic name split
//! at `::` (and at every other character that is not an ASCII letter or
//! digit), each segment capitalized and joined, with the suffix `Stream`;
//! the package `zerodds.<lowercase first segment>.v1`
//! ([`default_service`], [`default_package`]). Method paths follow the
//! gRPC scheme `/<package>.<TopicSlug>Stream/<Method>`.

use alloc::string::String;
use alloc::vec::Vec;

use crate::descriptor::{FieldDesc, FieldType, FileDesc, MessageDesc, MethodDesc, ServiceDesc};

/// The package of a service whose topic name yields no segment (an empty
/// name, or one of separators only).
pub const DEFAULT_PACKAGE: &str = "zerodds.topic.v1";

/// Spec §5.1: the default gRPC service of a DDS topic —
/// `Chat::Message` ⇒ `ChatMessageStream`.
#[must_use]
pub fn default_service(dds_topic: &str) -> String {
    format!("{}Stream", topic_to_pascal(dds_topic))
}

/// Spec §5.1: the default package of a DDS topic,
/// `zerodds.<lowercase first segment>.v1` — `Chat::Message` ⇒
/// `zerodds.chat.v1`. The first segment is the text before the first `::`;
/// a character in it that a protobuf identifier does not take (anything but
/// an ASCII letter, digit or `_`) becomes `_`, and a segment that begins
/// with a digit gets a leading `_` (Protocol Buffers Language
/// Specification, "Identifiers").
#[must_use]
pub fn default_package(dds_topic: &str) -> String {
    let first = dds_topic.split("::").next().unwrap_or("");
    let mut segment: String = first
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if segment.trim_matches('_').is_empty() {
        return DEFAULT_PACKAGE.into();
    }
    if segment.starts_with(|c: char| c.is_ascii_digit()) {
        segment.insert(0, '_');
    }
    format!("zerodds.{segment}.v1")
}

/// Service definition for a single DDS topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicService {
    /// gRPC package (e.g. `zerodds.chat.v1`).
    pub package: String,
    /// Service name (e.g. `TradeStream`).
    pub service: String,
    /// Original DDS topic name.
    pub dds_topic: String,
    /// DDS type name.
    pub dds_type: String,
}

impl TopicService {
    /// The service definition of a DDS topic with the defaults of spec
    /// §5.1 ([`default_service`], [`default_package`]).
    #[must_use]
    pub fn from_topic(dds_topic: &str, dds_type: &str) -> Self {
        Self {
            package: default_package(dds_topic),
            service: default_service(dds_topic),
            dds_topic: dds_topic.into(),
            dds_type: dds_type.into(),
        }
    }

    /// The fully-qualified service name, `<package>.<service>`.
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.package, self.service)
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

    /// gRPC path of `method`.
    #[must_use]
    pub fn method_path(&self, method: MethodKind) -> String {
        format!("/{}.{}/{}", self.package, self.service, method.name())
    }

    /// Is `path` one of the four method paths?
    #[must_use]
    pub fn matches_path(&self, path: &str) -> Option<MethodKind> {
        MethodKind::ALL
            .into_iter()
            .find(|m| path == self.method_path(*m))
    }
}

/// Which method a gRPC path addresses (spec §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    /// Client stream: stream Sample → PublishAck.
    Publish,
    /// Server stream: SubscribeReq → stream Sample.
    Subscribe,
    /// Unary: Sample → PublishAck.
    PublishOne,
    /// Unary: CatalogReq → CatalogResp.
    Catalog,
}

impl MethodKind {
    /// The four methods, in the order of the service definition.
    pub const ALL: [Self; 4] = [
        Self::Publish,
        Self::Subscribe,
        Self::PublishOne,
        Self::Catalog,
    ];

    /// The method name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Publish => "Publish",
            Self::Subscribe => "Subscribe",
            Self::PublishOne => "PublishOne",
            Self::Catalog => "Catalog",
        }
    }
}

/// Sanitize a DDS topic name to PascalCase (spec §5.1 steps 1 and 2): every
/// character that is not an ASCII letter or digit (`::` among them) is a
/// word boundary; each word begins with its upper case.
/// `Chat::Message` ⇒ `ChatMessage`, `dds.demo.Trade` ⇒ `DdsDemoTrade`,
/// `chat-room/foo_bar` ⇒ `ChatRoomFooBar`. A name that begins with a digit
/// gets the prefix `Topic` (a protobuf identifier begins with a letter).
#[must_use]
pub fn topic_to_pascal(topic: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in topic.chars() {
        if !c.is_ascii_alphanumeric() {
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
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "Topic");
    }
    out
}

/// The `.proto` definition of the service: the text of spec §4.2 with the
/// service's package and name.
#[must_use]
pub fn render_proto(svc: &TopicService) -> String {
    format!(
        r#"syntax = "proto3";
package {package};

service {service} {{
  rpc Publish(stream Sample) returns (PublishAck);
  rpc Subscribe(SubscribeReq) returns (stream Sample);
  rpc PublishOne(Sample) returns (PublishAck);
  rpc Catalog(CatalogReq) returns (CatalogResp);
}}

message Sample {{
  bytes  cdr_payload   = 1;       // CDR-Bytes including Encap-Header
  uint32 flags         = 2;       // dispose-bit etc.
  bytes  key_hash      = 3;
  uint64 source_ts_ns  = 4;
  string type_name     = 5;
  string topic_name    = 6;
}}

message PublishAck {{
  uint64 sequence_number = 1;
  uint64 acked_at_ns     = 2;
}}

message SubscribeReq {{
  string  partition_filter = 1;     // optional partition expression
  bool    include_dispose  = 2;
  uint32  initial_burst    = 3;     // 0 = no burst
}}

message CatalogReq {{}}
message CatalogResp {{
  repeated TopicEntry topics = 1;
}}
message TopicEntry {{
  string dds_name      = 1;
  string dds_type      = 2;
  string grpc_service  = 3;
  string idl_url       = 4;
}}
"#,
        package = svc.package,
        service = svc.service
    )
}

/// The name of the `.proto` file of `package` the reflection service serves
/// (§4.6): the package as a path, `zerodds.chat.v1` ⇒
/// `zerodds/chat/v1/bridge.proto`.
#[must_use]
pub fn proto_file_name(package: &str) -> String {
    format!("{}/bridge.proto", package.replace('.', "/"))
}

/// The file descriptor of `package` (§4.2, §4.6): the six messages of §4.2
/// and, for each name in `services`, a service with the four methods.
#[must_use]
pub fn proto_file(package: &str, services: &[&str]) -> FileDesc {
    use FieldType::{Bool, Bytes, String as Str, Uint32, Uint64};
    let msg = |name, fields| MessageDesc {
        name,
        fields,
        oneofs: Vec::new(),
    };
    FileDesc {
        name: proto_file_name(package),
        package: package.into(),
        messages: alloc::vec![
            msg(
                "Sample",
                alloc::vec![
                    FieldDesc::one("cdr_payload", 1, Bytes),
                    FieldDesc::one("flags", 2, Uint32),
                    FieldDesc::one("key_hash", 3, Bytes),
                    FieldDesc::one("source_ts_ns", 4, Uint64),
                    FieldDesc::one("type_name", 5, Str),
                    FieldDesc::one("topic_name", 6, Str),
                ],
            ),
            msg(
                "PublishAck",
                alloc::vec![
                    FieldDesc::one("sequence_number", 1, Uint64),
                    FieldDesc::one("acked_at_ns", 2, Uint64),
                ],
            ),
            msg(
                "SubscribeReq",
                alloc::vec![
                    FieldDesc::one("partition_filter", 1, Str),
                    FieldDesc::one("include_dispose", 2, Bool),
                    FieldDesc::one("initial_burst", 3, Uint32),
                ],
            ),
            msg("CatalogReq", Vec::new()),
            msg(
                "CatalogResp",
                alloc::vec![FieldDesc::msg("topics", 1, "TopicEntry").repeated()],
            ),
            msg(
                "TopicEntry",
                alloc::vec![
                    FieldDesc::one("dds_name", 1, Str),
                    FieldDesc::one("dds_type", 2, Str),
                    FieldDesc::one("grpc_service", 3, Str),
                    FieldDesc::one("idl_url", 4, Str),
                ],
            ),
        ],
        services: services
            .iter()
            .map(|name| ServiceDesc {
                name: (*name).into(),
                methods: MethodKind::ALL
                    .into_iter()
                    .map(|m| {
                        let (input, output, client_streaming, server_streaming) = match m {
                            MethodKind::Publish => ("Sample", "PublishAck", true, false),
                            MethodKind::Subscribe => ("SubscribeReq", "Sample", false, true),
                            MethodKind::PublishOne => ("Sample", "PublishAck", false, false),
                            MethodKind::Catalog => ("CatalogReq", "CatalogResp", false, false),
                        };
                        MethodDesc {
                            name: m.name(),
                            input,
                            output,
                            client_streaming,
                            server_streaming,
                        }
                    })
                    .collect(),
            })
            .collect(),
    }
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

    /// Register a topic service; a second service of the same
    /// fully-qualified name is not added.
    pub fn register(&mut self, svc: TopicService) {
        if !self
            .services
            .iter()
            .any(|s| s.service == svc.service && s.package == svc.package)
        {
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

    /// Look up by gRPC path (e.g. `/zerodds.trade.v1.TradeStream/Publish`).
    #[must_use]
    pub fn find_by_path(&self, path: &str) -> Option<(&TopicService, MethodKind)> {
        for s in &self.services {
            if let Some(k) = s.matches_path(path) {
                return Some((s, k));
            }
        }
        None
    }

    /// The file descriptors of the services (§4.6): one per package, with
    /// the messages of §4.2 and the package's services, in the order the
    /// packages first appear.
    #[must_use]
    pub fn proto_files(&self) -> Vec<FileDesc> {
        let mut packages: Vec<&str> = Vec::new();
        for s in &self.services {
            if !packages.contains(&s.package.as_str()) {
                packages.push(&s.package);
            }
        }
        packages
            .into_iter()
            .map(|p| {
                let names: Vec<&str> = self
                    .services
                    .iter()
                    .filter(|s| s.package == p)
                    .map(|s| s.service.as_str())
                    .collect();
                proto_file(p, &names)
            })
            .collect()
    }

    /// All fully-qualified service names (`<package>.<service>`).
    /// Spec §4.6 — reflection ListServices response.
    #[must_use]
    pub fn fully_qualified_service_names(&self) -> Vec<String> {
        self.services.iter().map(TopicService::full_name).collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn grpcbridge_4_2_pascal_simple() {
        assert_eq!(topic_to_pascal("Trade"), "Trade");
    }

    #[test]
    fn grpcbridge_4_2_pascal_dotted() {
        assert_eq!(topic_to_pascal("dds.demo.Trade"), "DdsDemoTrade");
    }

    #[test]
    fn grpcbridge_4_2_pascal_mixed_separators() {
        assert_eq!(topic_to_pascal("chat-room/foo_bar"), "ChatRoomFooBar");
    }

    #[test]
    fn grpcbridge_4_2_pascal_empty_falls_back() {
        assert_eq!(topic_to_pascal(""), "Topic");
    }

    /// §5.1: `Chat::Message` ⇒ `ChatMessageStream` in `zerodds.chat.v1`.
    #[test]
    fn grpcbridge_5_1_chat_message_becomes_chat_message_stream_in_zerodds_chat_v1() {
        assert_eq!(topic_to_pascal("Chat::Message"), "ChatMessage");
        assert_eq!(default_service("Chat::Message"), "ChatMessageStream");
        assert_eq!(default_package("Chat::Message"), "zerodds.chat.v1");
        let s = TopicService::from_topic("Chat::Message", "Chat::Message");
        assert_eq!(s.full_name(), "zerodds.chat.v1.ChatMessageStream");
        assert_eq!(
            default_service("Sensor::Imu::Reading"),
            "SensorImuReadingStream"
        );
        assert_eq!(default_package("Sensor::Imu::Reading"), "zerodds.sensor.v1");
    }

    /// §5.1: a name without `::` is its own first segment; what a protobuf
    /// identifier does not take is replaced.
    #[test]
    fn grpcbridge_5_1_a_single_segment_names_its_own_package() {
        assert_eq!(default_package("Trade"), "zerodds.trade.v1");
        assert_eq!(default_package("rt/chatter"), "zerodds.rt_chatter.v1");
        assert_eq!(
            default_package("dds.demo.Trade"),
            "zerodds.dds_demo_trade.v1"
        );
        assert_eq!(default_package("9lives::X"), "zerodds._9lives.v1");
        assert_eq!(default_package("::X"), DEFAULT_PACKAGE);
        assert_eq!(default_service("rt/chatter"), "RtChatterStream");
        assert_eq!(default_service("9lives"), "Topic9livesStream");
    }

    #[test]
    fn grpcbridge_4_2_topic_service_paths() {
        let s = TopicService::from_topic("Trade", "Trade");
        assert_eq!(s.publish_path(), "/zerodds.trade.v1.TradeStream/Publish");
        assert_eq!(
            s.subscribe_path(),
            "/zerodds.trade.v1.TradeStream/Subscribe"
        );
    }

    #[test]
    fn grpcbridge_4_2_matches_path_publish() {
        let s = TopicService::from_topic("Trade", "Trade");
        assert_eq!(
            s.matches_path("/zerodds.trade.v1.TradeStream/Publish"),
            Some(MethodKind::Publish)
        );
        assert_eq!(
            s.matches_path("/zerodds.trade.v1.TradeStream/Subscribe"),
            Some(MethodKind::Subscribe)
        );
        assert_eq!(s.matches_path("/wrong"), None);
    }

    #[test]
    fn grpcbridge_4_2_render_proto_contains_methods() {
        let s = TopicService::from_topic("Trade", "Trade");
        let proto = render_proto(&s);
        assert!(proto.contains("service TradeStream"));
        for m in MethodKind::ALL {
            assert!(proto.contains(&format!("rpc {}(", m.name())), "{m:?}");
        }
        assert_eq!(
            s.matches_path("/zerodds.trade.v1.TradeStream/PublishOne"),
            Some(MethodKind::PublishOne)
        );
        assert_eq!(
            s.matches_path("/zerodds.trade.v1.TradeStream/Catalog"),
            Some(MethodKind::Catalog)
        );
        assert_eq!(s.matches_path("/zerodds.trade.v1.TradeStream/Other"), None);
    }

    /// §4.2: the generated definition is the text of the spec, with the
    /// package and the service of the mapping.
    #[test]
    fn grpcbridge_4_2_render_proto_is_the_definition_of_the_spec() {
        const SPEC: &str = r#"syntax = "proto3";
package zerodds.chat.v1;

service ChatMessageStream {
  rpc Publish(stream Sample) returns (PublishAck);
  rpc Subscribe(SubscribeReq) returns (stream Sample);
  rpc PublishOne(Sample) returns (PublishAck);
  rpc Catalog(CatalogReq) returns (CatalogResp);
}

message Sample {
  bytes  cdr_payload   = 1;       // CDR-Bytes including Encap-Header
  uint32 flags         = 2;       // dispose-bit etc.
  bytes  key_hash      = 3;
  uint64 source_ts_ns  = 4;
  string type_name     = 5;
  string topic_name    = 6;
}

message PublishAck {
  uint64 sequence_number = 1;
  uint64 acked_at_ns     = 2;
}

message SubscribeReq {
  string  partition_filter = 1;     // optional partition expression
  bool    include_dispose  = 2;
  uint32  initial_burst    = 3;     // 0 = no burst
}

message CatalogReq {}
message CatalogResp {
  repeated TopicEntry topics = 1;
}
message TopicEntry {
  string dds_name      = 1;
  string dds_type      = 2;
  string grpc_service  = 3;
  string idl_url       = 4;
}
"#;
        let svc = TopicService {
            package: "zerodds.chat.v1".into(),
            service: "ChatMessageStream".into(),
            dds_topic: "Chat::Message".into(),
            dds_type: "Chat::Message".into(),
        };
        assert_eq!(render_proto(&svc), SPEC);
    }

    /// §4.2, §10: the `.proto` the crate ships for client stubs
    /// (`proto/zerodds/chat/v1/bridge.proto`, the file the grpc-go,
    /// grpc-java and tonic clients of `tests/cross_vendor.rs` generate
    /// from) is, below its comment header, what `render_proto` writes for
    /// `Chat::Message` with the defaults of §5.1, under the name the
    /// reflection service gives the file.
    #[test]
    fn grpcbridge_4_2_the_shipped_proto_is_what_render_proto_writes() {
        const SHIPPED: &str = include_str!("../proto/zerodds/chat/v1/bridge.proto");
        let body = SHIPPED
            .split_once("\n\n")
            .map(|(header, body)| {
                assert!(header.lines().all(|l| l.starts_with("//")), "{header}");
                body
            })
            .expect("a comment header, a blank line, the definition");
        let svc = TopicService::from_topic("Chat::Message", "Chat::Message");
        assert_eq!(body, render_proto(&svc));
        assert_eq!(
            proto_file_name(&svc.package),
            "zerodds/chat/v1/bridge.proto"
        );
    }

    /// §4.6: the descriptor of a package declares the six messages of
    /// §4.2 with their field numbers and types, and each service with the
    /// four methods and their streaming.
    #[test]
    fn grpcbridge_4_6_the_descriptor_declares_the_messages_of_4_2() {
        let f = proto_file("zerodds.chat.v1", &["ChatMessageStream", "ChatOtherStream"]);
        assert_eq!(f.name, "zerodds/chat/v1/bridge.proto");
        let names: Vec<&str> = f.messages.iter().map(|m| m.name).collect();
        assert_eq!(
            names,
            [
                "Sample",
                "PublishAck",
                "SubscribeReq",
                "CatalogReq",
                "CatalogResp",
                "TopicEntry"
            ]
        );
        let sample = &f.messages[0];
        let numbered: Vec<(&str, u32, FieldType)> = sample
            .fields
            .iter()
            .map(|x| (x.name, x.number, x.ty))
            .collect();
        assert_eq!(
            numbered,
            [
                ("cdr_payload", 1, FieldType::Bytes),
                ("flags", 2, FieldType::Uint32),
                ("key_hash", 3, FieldType::Bytes),
                ("source_ts_ns", 4, FieldType::Uint64),
                ("type_name", 5, FieldType::String),
                ("topic_name", 6, FieldType::String),
            ]
        );
        let topics = &f.messages[4].fields[0];
        assert!(topics.repeated);
        assert_eq!(topics.message, Some("TopicEntry"));
        assert_eq!(f.services.len(), 2);
        let m = &f.services[0].methods;
        assert_eq!(
            m.iter()
                .map(|x| (
                    x.name,
                    x.input,
                    x.output,
                    x.client_streaming,
                    x.server_streaming
                ))
                .collect::<Vec<_>>(),
            [
                ("Publish", "Sample", "PublishAck", true, false),
                ("Subscribe", "SubscribeReq", "Sample", false, true),
                ("PublishOne", "Sample", "PublishAck", false, false),
                ("Catalog", "CatalogReq", "CatalogResp", false, false),
            ]
        );
        assert!(f.declares("zerodds.chat.v1.ChatOtherStream.Subscribe"));
        assert!(f.declares("zerodds.chat.v1.TopicEntry.idl_url"));
        let mut cat = ServiceCatalog::new();
        cat.register(TopicService::from_topic("Chat::Message", "Chat::Message"));
        cat.register(TopicService::from_topic("Chat::Other", "Chat::Other"));
        cat.register(TopicService::from_topic("Trade", "Trade"));
        let files = cat.proto_files();
        assert_eq!(files.len(), 2, "one file per package");
        assert_eq!(files[0].services.len(), 2);
        assert_eq!(files[1].package, "zerodds.trade.v1");
    }

    #[test]
    fn grpcbridge_4_2_catalog_registration_dedup() {
        let mut cat = ServiceCatalog::new();
        cat.register(TopicService::from_topic("Trade", "Trade"));
        cat.register(TopicService::from_topic("Trade", "Trade"));
        assert_eq!(cat.len(), 1);
    }

    #[test]
    fn grpcbridge_4_2_catalog_find_by_path() {
        let mut cat = ServiceCatalog::new();
        cat.register(TopicService::from_topic("Trade", "Trade"));
        let (svc, kind) = cat
            .find_by_path("/zerodds.trade.v1.TradeStream/Subscribe")
            .expect("found");
        assert_eq!(svc.service, "TradeStream");
        assert_eq!(kind, MethodKind::Subscribe);
    }

    #[test]
    fn grpcbridge_4_2_catalog_fully_qualified_names() {
        let mut cat = ServiceCatalog::new();
        cat.register(TopicService::from_topic("Trade", "Trade"));
        cat.register(TopicService::from_topic("Quote", "Quote"));
        let names = cat.fully_qualified_service_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"zerodds.trade.v1.TradeStream".into()));
        assert!(names.contains(&"zerodds.quote.v1.QuoteStream".into()));
    }
}
