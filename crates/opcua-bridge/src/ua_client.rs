// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Gateway 1.0 §8 (OPC UA → DDS): browses and subscribes a configured
//! foreign OPC-UA server as a client
//! ([`zerodds_opcua_server::client::Client`] over
//! `zerodds_opcua_server::client::tcp::TcpTransport`) and writes the
//! monitored nodes' values into DDS.

use std::sync::Arc;

use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_server::client::{Client, TcpTransport};
use zerodds_types::dynamic::{DynamicData, DynamicType};

use crate::dds_bridge::DdsBridge;
use crate::dynamic_value::{self, FlatField};

/// A client-side operation failed.
#[derive(Debug)]
pub enum UaClientError {
    /// The `opc.tcp` connect/handshake failed.
    Connect(String),
    /// A service call failed.
    Service(String),
    /// [`crate::dynamic_value`] refused the shape.
    Convert(dynamic_value::ConvertError),
    /// The DDS write failed.
    Dds(String),
    /// The type could not be encoded ([`SerializedTypeSupport`]).
    Codec(String),
}

impl core::fmt::Display for UaClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Connect(m) => write!(f, "connect: {m}"),
            Self::Service(m) => write!(f, "service call: {m}"),
            Self::Convert(e) => write!(f, "{e}"),
            Self::Dds(m) => write!(f, "DDS: {m}"),
            Self::Codec(m) => write!(f, "codec: {m}"),
        }
    }
}

impl std::error::Error for UaClientError {}

impl From<dynamic_value::ConvertError> for UaClientError {
    fn from(e: dynamic_value::ConvertError) -> Self {
        Self::Convert(e)
    }
}

/// One monitored node's target: where in a DDS topic's type its value
/// lands. `field_path` is `""` for a scalar topic (the whole sample), or a
/// [`crate::dynamic_value::flatten`]-style dotted path into a struct topic
/// (e.g. writing one field of a multi-field topic from one external node).
pub struct NodeTarget {
    /// The node to monitor on the remote server.
    pub node_id: NodeId,
    /// The DDS topic its value is written to.
    pub dds_topic: String,
    /// The DDS type name (for `DcpsRuntime` registration).
    pub dds_type: String,
    /// The DDS type, resolved.
    pub dyn_type: DynamicType,
    /// The type's [`SerializedTypeSupport`].
    pub type_support: Arc<SerializedTypeSupport>,
    /// Whether the type is keyed.
    pub keyed: bool,
    /// Where in the type this node's value lands.
    pub field_path: String,
}

/// The `host:port` a `TcpStream::connect` needs, out of an `opc.tcp://host:
/// port[/path]` endpoint URL (Part 6 §7.1: the endpoint URL is the
/// application-level identity the Hello message carries; the socket only
/// wants the authority).
fn tcp_addr_of(endpoint_url: &str) -> &str {
    let authority = endpoint_url
        .strip_prefix("opc.tcp://")
        .unwrap_or(endpoint_url);
    authority.split('/').next().unwrap_or(authority)
}

struct BoundTarget {
    dds_topic: String,
    dyn_type: DynamicType,
    type_support: Arc<SerializedTypeSupport>,
    field_path: String,
    last: DynamicData,
}

/// One connected foreign OPC-UA server: its subscription and the DDS topics
/// its monitored nodes write into.
pub struct ClientJob {
    endpoint_url: String,
    client: Client,
    transport: TcpTransport,
    subscription_id: u32,
    targets: Vec<BoundTarget>,
}

impl ClientJob {
    /// Connects to `endpoint_url`, creates one subscription and one
    /// monitored item per `targets` entry (in order — the item's
    /// `client_handle` is its index), and registers a DDS writer per
    /// distinct topic on `dds_bridge`.
    ///
    /// # Errors
    /// [`UaClientError`] — see its variants.
    pub fn connect(
        dds_bridge: &mut DdsBridge,
        endpoint_url: &str,
        targets: Vec<NodeTarget>,
    ) -> Result<Self, UaClientError> {
        let addr = tcp_addr_of(endpoint_url);
        let mut transport =
            TcpTransport::connect(addr).map_err(|e| UaClientError::Connect(e.to_string()))?;
        let mut client = Client::new();
        client
            .connect(&mut transport, endpoint_url)
            .map_err(|e| UaClientError::Connect(e.to_string()))?;
        let subscription_id = client
            .create_subscription(&mut transport)
            .map_err(|e| UaClientError::Service(e.to_string()))?;

        let items: Vec<(NodeId, u32)> = targets
            .iter()
            .enumerate()
            .map(|(index, target)| {
                (
                    target.node_id.clone(),
                    u32::try_from(index).unwrap_or(u32::MAX),
                )
            })
            .collect();
        client
            .create_monitored_items(&mut transport, subscription_id, &items)
            .map_err(|e| UaClientError::Service(e.to_string()))?;

        let mut bound = Vec::with_capacity(targets.len());
        for target in targets {
            dds_bridge
                .writer(
                    &target.dds_topic,
                    &target.dds_type,
                    target.keyed,
                    false,
                    Some(Arc::clone(&target.type_support)),
                )
                .map_err(|e| UaClientError::Dds(e.to_string()))?;
            let last = dynamic_value::default_data(&target.dyn_type).map_err(|e| {
                UaClientError::Convert(dynamic_value::ConvertError::ShapeMismatch(e.to_string()))
            })?;
            bound.push(BoundTarget {
                dds_topic: target.dds_topic,
                dyn_type: target.dyn_type,
                type_support: target.type_support,
                field_path: target.field_path,
                last,
            });
        }

        Ok(Self {
            endpoint_url: endpoint_url.to_string(),
            client,
            transport,
            subscription_id,
            targets: bound,
        })
    }

    /// The remote endpoint URL this job is connected to.
    #[must_use]
    pub fn endpoint_url(&self) -> &str {
        &self.endpoint_url
    }

    /// Sends one Publish, applies every notification to its target's DDS
    /// topic, and returns the number of DDS writes made.
    ///
    /// # Errors
    /// [`UaClientError`] on a service, convert, codec or DDS failure.
    pub fn tick(&mut self, dds_bridge: &DdsBridge) -> Result<usize, UaClientError> {
        let outcome = self
            .client
            .publish_acknowledging(&mut self.transport, &[])
            .map_err(|e| UaClientError::Service(e.to_string()))?;
        if outcome.subscription_id != self.subscription_id {
            return Ok(0);
        }
        let mut written = 0;
        for notification in outcome.notifications {
            let Some(target) = self.targets.get_mut(notification.client_handle as usize) else {
                continue; // a notification for a handle this job never registered — ignored, not a write target.
            };
            let Some(value) = notification.value.value else {
                continue; // Part 4 §7.28: a DataValue with no Value (only a StatusCode) has nothing to write.
            };
            let fields = [FlatField {
                path: target.field_path.clone(),
                value,
            }];
            let data = dynamic_value::unflatten(&target.dyn_type, &fields, &target.last)?;
            let sample = target
                .type_support
                .from_dynamic_data(data.clone())
                .map_err(UaClientError::Codec)?;
            let body = target
                .type_support
                .write_body(&sample, 2, false)
                .map_err(UaClientError::Codec)?;
            dds_bridge
                .write(&target.dds_topic, &body)
                .map_err(|e| UaClientError::Dds(e.to_string()))?;
            target.last = data;
            written += 1;
        }
        Ok(written)
    }

    /// Closes the session (Part 4 §5.7.4). Errors are the caller's to log —
    /// a failed close does not leave DDS-side state behind, unlike a failed
    /// DDS write.
    ///
    /// # Errors
    /// [`UaClientError::Service`].
    pub fn close(&mut self) -> Result<(), UaClientError> {
        self.client
            .close_session(&mut self.transport, false)
            .map_err(|e| UaClientError::Service(e.to_string()))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;
    use zerodds_dcps::runtime::RuntimeConfig;
    use zerodds_opcua_gateway::data_value::{DataValue, Variant, VariantValue};
    use zerodds_opcua_server::address_space::AddressSpace;
    use zerodds_opcua_server::server::Server;

    fn ast_of(idl: &str) -> Arc<zerodds_ast::Ast> {
        Arc::new(
            zerodds_wire_probe::encode::model_from_idl(idl, "ua_client_test.idl").expect("idl"),
        )
    }

    #[test]
    fn client_job_writes_a_monitored_scalar_node_into_dds() {
        let ast = ast_of("@final struct Temp { double v; };");
        let dyn_type = DynamicType::find(&ast, "Temp").expect("type");
        let type_support = crate::idl::type_support_for(&[ast], "Temp").expect("support");

        let mut address_space = AddressSpace::new();
        let node_id = NodeId::numeric(1, 100);
        address_space.set_value(
            node_id.clone(),
            DataValue::new_value(Variant::scalar(VariantValue::Double(21.5)), 0, 0),
        );
        let mut server = Server::new("opc.tcp://127.0.0.1:0", address_space);

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let accepted = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            zerodds_opcua_server::client::serve_connection(&mut server, stream)
        });

        let lease = zerodds_test_domains::lease_domain();
        let mut bridge = DdsBridge::start(
            lease.id(),
            "opcua-bridge-client-test",
            RuntimeConfig::default(),
        )
        .expect("start");
        let reader = bridge
            .reader(
                "Temp",
                "Temp",
                false,
                false,
                Some(Arc::clone(&type_support)),
            )
            .expect("reader");

        let target = NodeTarget {
            node_id,
            dds_topic: "Temp".to_string(),
            dds_type: "Temp".to_string(),
            dyn_type,
            type_support,
            keyed: false,
            field_path: "v".to_string(),
        };
        let mut job =
            ClientJob::connect(&mut bridge, &std::format!("opc.tcp://{addr}"), vec![target])
                .expect("connect");
        let written = job.tick(&bridge).expect("tick");
        assert_eq!(written, 1);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut got = Vec::new();
        while got.is_empty() && std::time::Instant::now() < deadline {
            got = reader.poll();
            if got.is_empty() {
                thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        assert_eq!(got.len(), 1);
        let target = job.targets.first().expect("one target");
        let decoded = target
            .type_support
            .read_body(&got[0].body, got[0].representation, got[0].big_endian)
            .expect("decode");
        let (fields, skipped) =
            dynamic_value::flatten(&target.dyn_type, decoded.data()).expect("flatten");
        assert!(skipped.is_empty());
        assert_eq!(fields[0].value, Variant::scalar(VariantValue::Double(21.5)));

        job.close().expect("close");
        drop(job);
        let _ = accepted.join();
    }
}
