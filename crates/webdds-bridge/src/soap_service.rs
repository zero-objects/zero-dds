// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SOAP-PSM request dispatch — Spec §8.4 (SIMPLE-WSDL-SOAP), sibling of
//! [`crate::rest_service`] over the same [`crate::gateway::Gateway`].
//!
//! A SOAP request's body element name (`<CreateApplication>`,
//! `<WriteSample>`, ...) is one of [`crate::wsdl_bridge::builtin_operations`]
//! — the DDS-WEB object-model operations, the same set REST exposes.
//! Parameters travel as the body element's fields
//! (`crate::xmlfields`), and a `WriteSample`/`ReadSample`'s `data`
//! field carries the sample itself, XML per `crate::dynxml` — the same
//! representation REST uses, so a client that already builds REST XML
//! bodies reuses that code for SOAP.
//!
//! An operation name that names a config-named IDL `interface`'s own
//! operation (its WSDL is real, from `wsdl_bridge::operations_of`) is
//! dispatched as a DDS-RPC call through [`crate::rpc_dispatch`] — "WSDL
//! and runtime from the same model": both read
//! `zerodds_lang_rules::adapter::interface`. A name that is neither a
//! builtin nor a known business operation gets a `env:Receiver` fault
//! naming both possibilities it did not match.

use std::sync::Mutex;

use zerodds_soap::envelope::{Envelope, build_envelope, parse_envelope};
use zerodds_soap::fault::{Fault, FaultCode};
use zerodds_web::bridge::{BackendError, BackendResult, DdsBackend};
use zerodds_xml_wire::emitter::XmlEmitter;

use crate::gateway::Gateway;
use crate::rpc_dispatch::RpcDispatcher;
use crate::wsdl_bridge::builtin_operations;
use crate::xmlfields::{self, FlatFields};

/// Dispatches one SOAP request body (already extracted from its
/// envelope) and returns the response envelope's XML — either a
/// `<OperationResponse>` success body or an `env:Fault`.
///
/// `rpc` is one [`RpcDispatcher`] per config-named `interfaces:` IDL
/// file (empty where the daemon configured none — no business
/// operation is reachable, but nothing about the object-model
/// operations changes); the first one that recognizes the operation
/// name handles the call.
#[must_use]
pub fn dispatch_envelope(
    gateway: &Mutex<Gateway>,
    rpc: &[RpcDispatcher],
    raw_envelope: &str,
    permit: bool,
) -> String {
    if !permit {
        return fault_envelope(Fault::new(FaultCode::Sender, "denied by ACL"));
    }
    let envelope = match parse_envelope(raw_envelope) {
        Ok(e) => e,
        Err(e) => {
            return fault_envelope(Fault::new(
                FaultCode::Sender,
                &format!("malformed envelope: {e}"),
            ));
        }
    };
    let (op_name, fields) = match operation_of(&envelope) {
        Ok(v) => v,
        Err(e) => return fault_envelope(Fault::new(FaultCode::Sender, &e)),
    };
    if !builtin_operations().iter().any(|o| o.name == op_name) {
        if let Some(dispatcher) = rpc.iter().find(|d| d.knows_operation(&op_name)) {
            let body_xml = envelope.body_xml.trim();
            return match dispatcher.dispatch(&op_name, body_xml) {
                Ok(reply_xml) => success_envelope(&op_name, &reply_xml),
                Err(e) => fault_envelope(Fault::new(FaultCode::Receiver, &e.to_string())),
            };
        }
        return fault_envelope(Fault::new(
            FaultCode::Receiver,
            &format!(
                "operation `{op_name}` is neither one of the DDS-WEB object-model operations \
                 this gateway dispatches over SOAP, nor an operation of a configured \
                 `interfaces:` IDL file"
            ),
        ));
    }
    let result = if op_name == "GetWaitset" {
        // A long-poll waits without the gateway, which every other
        // request needs.
        crate::gateway::waitset_timeout(fields.get("timeout"))
            .and_then(|timeout| {
                let handle = gateway
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .waitset_handle(
                        fields.get("app").unwrap_or_default(),
                        fields.get("waitset").unwrap_or_default(),
                    );
                handle.and_then(|h| h.wait(timeout))
            })
            .map(render_result)
            .map_err(|e| format!("{e:?}"))
    } else {
        let mut gw = gateway
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        dispatch_operation(
            &mut gw,
            &op_name,
            &fields,
            envelope.body_xml.trim().as_bytes(),
        )
    };
    match result {
        Ok(body_xml) => success_envelope(&op_name, &body_xml),
        Err(e) => fault_envelope(Fault::new(FaultCode::Receiver, &e)),
    }
}

/// Extracts the operation name (the SOAP body's single root element
/// tag) and its fields.
fn operation_of(envelope: &Envelope) -> Result<(String, FlatFields), String> {
    let body = envelope.body_xml.trim();
    let name = body
        .strip_prefix('<')
        .and_then(|s| {
            s.split(|c: char| c == '>' || c.is_whitespace() || c == '/')
                .next()
        })
        .ok_or_else(|| "empty SOAP body".to_string())?
        .to_string();
    let fields = xmlfields::parse(body).map_err(|e| e.to_string())?;
    Ok((name, fields))
}

fn dispatch_operation(
    gw: &mut Gateway,
    op: &str,
    f: &FlatFields,
    raw_body: &[u8],
) -> Result<String, String> {
    let s = |k: &str| f.get(k).unwrap_or_default().to_string();
    let result: Result<BackendResult, BackendError> = match op {
        "CreateApplication" => gw.create_application(&s("name")),
        "DeleteApplication" => gw.delete_application(&s("app")),
        "GetApplications" => gw.list_applications("*"),
        "CreateParticipant" => crate::gateway::domain_id_of(f.get("domain_id"))
            .and_then(|domain_id| gw.create_participant(&s("app"), domain_id, raw_body)),
        "UpdateParticipant" => gw.update_participant(&s("app"), &s("participant"), raw_body),
        "DeleteParticipant" => gw.delete_participant(&s("app"), &s("participant")),
        "GetParticipants" => gw.list_participants(&s("app")),
        "CreateTopic" => gw.create_topic(&s("app"), &s("participant"), &s("name"), &s("type_name")),
        "UpdateTopic" => gw.update_topic(&s("app"), &s("participant"), &s("topic"), raw_body),
        "DeleteTopic" => gw.delete_topic(&s("app"), &s("participant"), &s("topic")),
        "GetTopics" => gw.list_topics(&s("app"), &s("participant")),
        "CreatePublisher" => gw.create_publisher(&s("app"), &s("participant"), raw_body),
        "UpdatePublisher" => {
            gw.update_publisher(&s("app"), &s("participant"), &s("publisher"), raw_body)
        }
        "DeletePublisher" => gw.delete_publisher(&s("app"), &s("participant"), &s("publisher")),
        "GetPublishers" => gw.list_publishers(&s("app"), &s("participant")),
        "CreateSubscriber" => gw.create_subscriber(&s("app"), &s("participant"), raw_body),
        "UpdateSubscriber" => {
            gw.update_subscriber(&s("app"), &s("participant"), &s("subscriber"), raw_body)
        }
        "DeleteSubscriber" => gw.delete_subscriber(&s("app"), &s("participant"), &s("subscriber")),
        "GetSubscribers" => gw.list_subscribers(&s("app"), &s("participant")),
        "CreateDataWriter" => gw.create_data_writer(
            &s("app"),
            &s("participant"),
            &s("publisher"),
            &s("topic_name"),
        ),
        "UpdateDataWriter" => gw.update_data_writer(
            &s("app"),
            &s("participant"),
            &s("publisher"),
            &s("writer"),
            raw_body,
        ),
        "DeleteDataWriter" => {
            gw.delete_data_writer(&s("app"), &s("participant"), &s("publisher"), &s("writer"))
        }
        "GetDataWriters" => gw.list_data_writers(&s("app"), &s("participant"), &s("publisher")),
        "CreateDataReader" => gw.create_data_reader(
            &s("app"),
            &s("participant"),
            &s("subscriber"),
            &s("topic_name"),
        ),
        "UpdateDataReader" => gw.update_data_reader(
            &s("app"),
            &s("participant"),
            &s("subscriber"),
            &s("reader"),
            raw_body,
        ),
        "DeleteDataReader" => {
            gw.delete_data_reader(&s("app"), &s("participant"), &s("subscriber"), &s("reader"))
        }
        "GetDataReaders" => gw.list_data_readers(&s("app"), &s("participant"), &s("subscriber")),
        "WriteSample" => {
            // `<data>` holds the sample as REST writes it — elements, so
            // the subtree itself — or that XML as escaped text.
            let data = match f.get("data") {
                Some(text) => text.to_string(),
                None => xmlfields::child_xml(&String::from_utf8_lossy(raw_body), "data")
                    .map_err(|e| e.to_string())?
                    .unwrap_or_default(),
            };
            gw.write_sample(
                &s("app"),
                &s("participant"),
                &s("publisher"),
                &s("writer"),
                data.as_bytes(),
            )
        }
        "ReadSample" => gw.read_samples(
            &s("app"),
            &s("participant"),
            &s("subscriber"),
            &s("reader"),
            f.get("query"),
        ),
        "CreateWaitset" => gw.create_waitset(&s("app"), &s("name"), &f.list("readers")),
        "UpdateWaitset" => gw.update_waitset(&s("app"), &s("waitset"), &f.list("readers")),
        // `dispatch_envelope` waits outside the gateway; this is the same
        // wait for a caller that already holds it.
        "GetWaitset" => crate::gateway::waitset_timeout(f.get("timeout"))
            .and_then(|timeout| gw.wait_waitset(&s("app"), &s("waitset"), timeout)),
        "DeleteWaitset" => gw.delete_waitset(&s("app"), &s("waitset")),
        "GetWaitsets" => gw.list_waitsets(&s("app")),
        other => return Err(format!("unhandled builtin operation `{other}`")),
    };
    result.map(render_result).map_err(|e| format!("{e:?}"))
}

fn render_result(result: BackendResult) -> String {
    let mut emitter = XmlEmitter::new();
    match result {
        BackendResult::Created(path) => {
            let _ = emitter.start_element("path", &[]);
            emitter.text(&path);
            let _ = emitter.end_element();
        }
        BackendResult::Deleted => {}
        BackendResult::List(items) => {
            for item in items {
                let _ = emitter.start_element("item", &[]);
                emitter.text(&item);
                let _ = emitter.end_element();
            }
        }
        BackendResult::Body { body, .. } => {
            emitter.text(&String::from_utf8_lossy(&body));
        }
    }
    emitter.finish()
}

fn success_envelope(op_name: &str, body_xml: &str) -> String {
    let response_tag = format!("{op_name}Response");
    let body = format!("<{response_tag}>{body_xml}</{response_tag}>");
    build_envelope(&Envelope {
        header_xml: None,
        body_xml: body,
    })
    .unwrap_or_else(|e| {
        // `build_envelope` only fails on an invalid tag name, which a
        // fixed, code-generated tag never is — but a raw `format!`
        // string is still surfaced instead of unwrapped, per the
        // project's no-silent-drop rule.
        fault_envelope(Fault::new(
            FaultCode::Receiver,
            &format!("could not build response envelope: {e}"),
        ))
    })
}

fn fault_envelope(fault: Fault) -> String {
    build_envelope(&Envelope {
        header_xml: None,
        body_xml: fault.to_xml(),
    })
    .unwrap_or_else(|_| {
        format!(
            "<soap:Envelope xmlns:soap=\"{}\"><soap:Body>{}</soap:Body></soap:Envelope>",
            zerodds_soap::envelope::SOAP_12_NS,
            fault.to_xml()
        )
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn empty_gateway() -> Mutex<Gateway> {
        Mutex::new(Gateway::new(
            BTreeMap::new(),
            std::sync::Arc::new(crate::metadata::MetadataStore::new()),
        ))
    }

    fn envelope(body: &str) -> String {
        build_envelope(&Envelope {
            header_xml: None,
            body_xml: body.to_string(),
        })
        .unwrap()
    }

    #[test]
    fn denied_by_acl_yields_sender_fault() {
        let gw = empty_gateway();
        let resp = dispatch_envelope(
            &gw,
            &[],
            &envelope("<CreateApplication><name>a</name></CreateApplication>"),
            false,
        );
        assert!(resp.contains("env:Sender"));
    }

    #[test]
    fn create_application_round_trips() {
        let gw = empty_gateway();
        let resp = dispatch_envelope(
            &gw,
            &[],
            &envelope("<CreateApplication><name>app1</name></CreateApplication>"),
            true,
        );
        assert!(resp.contains("CreateApplicationResponse"));
        assert!(resp.contains("applications/app1"));
    }

    #[test]
    fn unknown_operation_yields_receiver_fault_naming_the_boundary() {
        let gw = empty_gateway();
        let resp = dispatch_envelope(
            &gw,
            &[],
            &envelope("<SomeBusinessOp><x>1</x></SomeBusinessOp>"),
            true,
        );
        assert!(resp.contains("env:Receiver"));
        assert!(resp.contains("configured"));
    }

    #[test]
    fn malformed_envelope_yields_sender_fault() {
        let gw = empty_gateway();
        let resp = dispatch_envelope(&gw, &[], "<not-soap/>", true);
        assert!(resp.contains("env:Sender"));
    }

    #[test]
    fn full_lifecycle_over_soap() {
        let gw = empty_gateway();
        let _ = dispatch_envelope(
            &gw,
            &[],
            &envelope("<CreateApplication><name>app1</name></CreateApplication>"),
            true,
        );
        let resp = dispatch_envelope(
            &gw,
            &[],
            &envelope(
                "<CreateParticipant><app>app1</app><domain_id>0</domain_id></CreateParticipant>",
            ),
            true,
        );
        assert!(resp.contains("CreateParticipantResponse"));
        assert!(resp.contains("domain_participants/p1"));
    }
}
