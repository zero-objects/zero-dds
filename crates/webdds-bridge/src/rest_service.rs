// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! REST-PSM request dispatch — Spec §8.3: HTTP method + URI
//! (`zerodds_web::rest::parse_route`) to a [`crate::gateway::Gateway`]
//! or [`crate::metadata::MetadataStore`] call, and the XML response
//! (`zerodds_web::status::http_status_for` + Tab 6 element names) back.
//!
//! Authorization is not decided here: the caller (`daemon::server`)
//! checks `zerodds_bridge_security::Acl` before calling [`dispatch`] at
//! all, and passes the already-decided `permit: bool` in — this daemon
//! reuses the bridge fleet's common auth/ACL substrate
//! (`zerodds_bridge_security`) rather than the Spec §7.3
//! `AccessController`/`Permissions` engine `zerodds_web::access_control`
//! also models; see the crate README "Security" section for why (both
//! implement the same Spec §7.3 decision shape — permit-by-default is
//! never used).
//!
//! **Update (`PUT`) on a DCPS entity changes its QoS**
//! (`DomainParticipant`/`Topic`/`Publisher`/`Subscriber`/`DataWriter`/
//! `DataReader::set_qos`, via `zerodds_dcps::Entity` —
//! `crate::qos_update` turns the PUT body's inline `<qos>` container
//! into the concrete Qos value, merged onto the entity's own current
//! QoS — see that module's doc for why a merge and not the more literal
//! "replace the whole representation"). `UpdateWaitset` instead
//! replaces the condition set, per Spec §7.4.3.5. None of the six read
//! a `<qos_profile>name</qos_profile>` *reference* — only an inline
//! `<qos>` — and `update_participant`/`update_publisher`/
//! `update_subscriber` do not sync contained entities the way Spec
//! §7.4.3.2/§7.4.4.8/§7.4.4.12 also describe (their `create_*`
//! counterparts do not read contained entities out of their own body
//! either) — see the crate README "Open points" for both.
//! `UpdateQosLibrary`/`UpdateQosProfile` are metadata and are fully
//! implemented against [`crate::metadata::MetadataStore`].

use std::sync::Mutex;

use zerodds_web::bridge::{BackendError, BackendResult, DdsBackend};
use zerodds_web::representation::{
    CONTENT_TYPE_DDS_WEB_XML, ContentType, RepresentationKind, element_name,
};
use zerodds_web::rest::{RestMethod, RestRoute, parse_route};
use zerodds_web::status::{ReturnStatus, http_status_for};
use zerodds_xml_wire::emitter::XmlEmitter;

use crate::gateway::Gateway;
use crate::http1::{Request, Response};
use crate::metadata::{MetadataError, MetadataStore, StoredDocument};
use crate::xmlfields;

/// Dispatches one HTTP request against `gateway`/`metadata`.
///
/// # Panics
/// Never — every error path is converted to an HTTP response.
#[must_use]
pub fn dispatch(
    gateway: &Mutex<Gateway>,
    metadata: &MetadataStore,
    req: &Request,
    permit: bool,
) -> Response {
    if !permit {
        return fault_response(
            RestMethod::Get,
            ReturnStatus::PermissionsError,
            "denied by ACL",
        );
    }
    if let Some(resp) = check_representation_headers(req) {
        return resp;
    }
    let method = match req.method.as_str() {
        "GET" => RestMethod::Get,
        "POST" => RestMethod::Post,
        "PUT" => RestMethod::Put,
        "DELETE" => RestMethod::Delete,
        "HEAD" => RestMethod::Head,
        _ => {
            return Response::empty(405, "Method Not Allowed")
                .with_header("Allow", "GET, POST, PUT, DELETE, HEAD");
        }
    };
    let path = req.path();
    let route = match parse_route(method, path) {
        Ok(r) => r,
        Err(_) => return Response::empty(404, "Not Found"),
    };
    let query = req.target.split_once('?').map(|(_, q)| q).unwrap_or("");
    let head_only = matches!(req.method.as_str(), "HEAD");
    // `DataWriter::write` is a `POST` that creates no new resource — the
    // generic method->status rule (`POST` => `201 Created`) is for a
    // *create* operation, and reusing `BackendResult::Deleted` for "no
    // body to return" would wrongly pick up that 201 through `method`
    // alone. Its success status is `200 OK`, computed as a `GET`'s
    // would be, without changing what `parse_route` matched on.
    let effective_method = if head_only || matches!(route, RestRoute::DataWriterWrite { .. }) {
        RestMethod::Get
    } else {
        method
    };

    let outcome = match &route {
        // A long-poll waits without the gateway, which every other
        // request needs.
        RestRoute::GetWaitset { app, waitset } => Outcome::Backend(
            crate::gateway::waitset_timeout(query_param(query, "timeout").as_deref()).and_then(
                |timeout| {
                    let handle = gateway
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .waitset_handle(app, waitset);
                    handle.and_then(|h| h.wait(timeout))
                },
            ),
        ),
        _ => {
            let mut gw = gateway
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            route_to_outcome(&mut gw, metadata, &route, &req.body, query)
        }
    };
    let mut resp = outcome_to_response(effective_method, outcome);
    if head_only {
        resp.body.clear();
    }
    resp
}

enum Outcome {
    Backend(Result<BackendResult, BackendError>),
    Meta(Result<MetaResult, MetadataError>),
}

enum MetaResult {
    Created(String),
    Deleted,
    List(Vec<String>),
}

fn route_to_outcome(
    gw: &mut Gateway,
    metadata: &MetadataStore,
    route: &RestRoute,
    body: &[u8],
    query: &str,
) -> Outcome {
    match route {
        RestRoute::CreateApplication => Outcome::Backend(
            gw.create_application(&body_text_field(body, "name").unwrap_or_default()),
        ),
        RestRoute::DeleteApplication { app } => Outcome::Backend(gw.delete_application(app)),
        RestRoute::GetApplications => Outcome::Backend(gw.list_applications("*")),

        RestRoute::CreateType => {
            let name = body_text_field(body, "name").unwrap_or_default();
            Outcome::Meta(
                metadata
                    .types
                    .create(
                        &name,
                        StoredDocument {
                            content_type: CONTENT_TYPE_DDS_WEB_XML.to_string(),
                            body: body.to_vec(),
                        },
                    )
                    .map(|()| MetaResult::Created(format!("types/{name}"))),
            )
        }
        RestRoute::DeleteType { type_name } => Outcome::Meta(
            metadata
                .types
                .delete(type_name)
                .map(|()| MetaResult::Deleted),
        ),
        RestRoute::GetTypes => Outcome::Meta(Ok(MetaResult::List(metadata.types.names()))),

        RestRoute::CreateQosLibrary => {
            let name = body_text_field(body, "name").unwrap_or_default();
            Outcome::Meta(
                metadata
                    .qos_libraries
                    .create(
                        &name,
                        StoredDocument {
                            content_type: CONTENT_TYPE_DDS_WEB_XML.to_string(),
                            body: body.to_vec(),
                        },
                    )
                    .map(|()| MetaResult::Created(format!("qos_libraries/{name}"))),
            )
        }
        RestRoute::UpdateQosLibrary { qos_lib } => Outcome::Meta(
            metadata
                .qos_libraries
                .update(
                    qos_lib,
                    StoredDocument {
                        content_type: CONTENT_TYPE_DDS_WEB_XML.to_string(),
                        body: body.to_vec(),
                    },
                )
                .map(|()| MetaResult::Deleted),
        ),
        RestRoute::DeleteQosLibrary { qos_lib } => Outcome::Meta(
            metadata
                .qos_libraries
                .delete(qos_lib)
                .map(|()| MetaResult::Deleted),
        ),
        RestRoute::GetQosLibraries => {
            Outcome::Meta(Ok(MetaResult::List(metadata.qos_libraries.names())))
        }

        RestRoute::CreateQosProfile { qos_lib } => {
            let name = body_text_field(body, "name").unwrap_or_default();
            let key = MetadataStore::profile_key(qos_lib, &name);
            Outcome::Meta(
                metadata
                    .qos_profiles
                    .create(
                        &key,
                        StoredDocument {
                            content_type: CONTENT_TYPE_DDS_WEB_XML.to_string(),
                            body: body.to_vec(),
                        },
                    )
                    .map(|()| {
                        MetaResult::Created(format!("qos_libraries/{qos_lib}/qos_profiles/{name}"))
                    }),
            )
        }
        RestRoute::UpdateQosProfile { qos_lib, profile } => {
            let key = MetadataStore::profile_key(qos_lib, profile);
            Outcome::Meta(
                metadata
                    .qos_profiles
                    .update(
                        &key,
                        StoredDocument {
                            content_type: CONTENT_TYPE_DDS_WEB_XML.to_string(),
                            body: body.to_vec(),
                        },
                    )
                    .map(|()| MetaResult::Deleted),
            )
        }
        RestRoute::DeleteQosProfile { qos_lib, profile } => {
            let key = MetadataStore::profile_key(qos_lib, profile);
            Outcome::Meta(
                metadata
                    .qos_profiles
                    .delete(&key)
                    .map(|()| MetaResult::Deleted),
            )
        }
        RestRoute::GetQosProfiles { qos_lib } => {
            let prefix = format!("{qos_lib}/");
            let names = metadata
                .qos_profiles
                .names()
                .into_iter()
                .filter(|n| n.starts_with(&prefix))
                .collect();
            Outcome::Meta(Ok(MetaResult::List(names)))
        }

        RestRoute::CreateParticipant { app } => Outcome::Backend(
            crate::gateway::domain_id_of(body_text_field(body, "domain_id").as_deref())
                .and_then(|domain_id| gw.create_participant(app, domain_id, body)),
        ),
        RestRoute::UpdateParticipant { app, participant } => {
            Outcome::Backend(gw.update_participant(app, participant, body))
        }
        RestRoute::DeleteParticipant { app, participant } => {
            Outcome::Backend(gw.delete_participant(app, participant))
        }
        RestRoute::GetParticipants { app } => Outcome::Backend(gw.list_participants(app)),

        RestRoute::CreateTopic { app, participant } => {
            let name = body_text_field(body, "name").unwrap_or_default();
            let type_name = body_text_field(body, "type_name").unwrap_or_default();
            Outcome::Backend(gw.create_topic(app, participant, &name, &type_name))
        }
        RestRoute::UpdateTopic {
            app,
            participant,
            topic,
        } => Outcome::Backend(gw.update_topic(app, participant, topic, body)),
        RestRoute::DeleteTopic {
            app,
            participant,
            topic,
        } => Outcome::Backend(gw.delete_topic(app, participant, topic)),
        RestRoute::GetTopics { app, participant } => {
            Outcome::Backend(gw.list_topics(app, participant))
        }

        RestRoute::CreatePublisher { app, participant } => {
            Outcome::Backend(gw.create_publisher(app, participant, body))
        }
        RestRoute::UpdatePublisher {
            app,
            participant,
            publisher,
        } => Outcome::Backend(gw.update_publisher(app, participant, publisher, body)),
        RestRoute::DeletePublisher {
            app,
            participant,
            publisher,
        } => Outcome::Backend(gw.delete_publisher(app, participant, publisher)),
        RestRoute::GetPublishers { app, participant } => {
            Outcome::Backend(gw.list_publishers(app, participant))
        }

        RestRoute::CreateSubscriber { app, participant } => {
            Outcome::Backend(gw.create_subscriber(app, participant, body))
        }
        RestRoute::UpdateSubscriber {
            app,
            participant,
            subscriber,
        } => Outcome::Backend(gw.update_subscriber(app, participant, subscriber, body)),
        RestRoute::DeleteSubscriber {
            app,
            participant,
            subscriber,
        } => Outcome::Backend(gw.delete_subscriber(app, participant, subscriber)),
        RestRoute::GetSubscribers { app, participant } => {
            Outcome::Backend(gw.list_subscribers(app, participant))
        }

        RestRoute::CreateDataWriter {
            app,
            participant,
            publisher,
        } => {
            let topic = body_text_field(body, "topic_name").unwrap_or_default();
            Outcome::Backend(gw.create_data_writer(app, participant, publisher, &topic))
        }
        RestRoute::UpdateDataWriter {
            app,
            participant,
            publisher,
            data_writer,
        } => {
            Outcome::Backend(gw.update_data_writer(app, participant, publisher, data_writer, body))
        }
        RestRoute::DeleteDataWriter {
            app,
            participant,
            publisher,
            data_writer,
        } => Outcome::Backend(gw.delete_data_writer(app, participant, publisher, data_writer)),
        RestRoute::GetDataWriters {
            app,
            participant,
            publisher,
        } => Outcome::Backend(gw.list_data_writers(app, participant, publisher)),

        RestRoute::CreateDataReader {
            app,
            participant,
            subscriber,
        } => {
            let topic = body_text_field(body, "topic_name").unwrap_or_default();
            Outcome::Backend(gw.create_data_reader(app, participant, subscriber, &topic))
        }
        RestRoute::UpdateDataReader {
            app,
            participant,
            subscriber,
            data_reader,
        } => {
            Outcome::Backend(gw.update_data_reader(app, participant, subscriber, data_reader, body))
        }
        RestRoute::DeleteDataReader {
            app,
            participant,
            subscriber,
            data_reader,
        } => Outcome::Backend(gw.delete_data_reader(app, participant, subscriber, data_reader)),
        RestRoute::GetDataReaders {
            app,
            participant,
            subscriber,
        } => Outcome::Backend(gw.list_data_readers(app, participant, subscriber)),

        RestRoute::DataWriterWrite {
            app,
            participant,
            publisher,
            data_writer,
        } => Outcome::Backend(gw.write_sample(app, participant, publisher, data_writer, body)),
        RestRoute::DataReaderRead {
            app,
            participant,
            subscriber,
            data_reader,
        } => {
            let selector = query_param(query, "query");
            Outcome::Backend(gw.read_samples(
                app,
                participant,
                subscriber,
                data_reader,
                selector.as_deref(),
            ))
        }

        RestRoute::CreateWaitset { app } => {
            let name = body_text_field(body, "name").unwrap_or_default();
            let readers = match readers_list(body) {
                Ok(r) => r,
                Err(e) => return Outcome::Backend(Err(e)),
            };
            Outcome::Backend(gw.create_waitset(app, &name, &readers))
        }
        RestRoute::GetWaitset { app, waitset } => Outcome::Backend(
            // `dispatch` waits outside the gateway; this is the same wait
            // for a caller that already holds it.
            crate::gateway::waitset_timeout(query_param(query, "timeout").as_deref())
                .and_then(|timeout| gw.wait_waitset(app, waitset, timeout)),
        ),
        RestRoute::UpdateWaitset { app, waitset } => {
            let readers = match readers_list(body) {
                Ok(r) => r,
                Err(e) => return Outcome::Backend(Err(e)),
            };
            Outcome::Backend(gw.update_waitset(app, waitset, &readers))
        }
        RestRoute::DeleteWaitset { app, waitset } => {
            Outcome::Backend(gw.delete_waitset(app, waitset))
        }
        RestRoute::GetWaitsets { app } => Outcome::Backend(gw.list_waitsets(app)),

        RestRoute::RegisterType { app, participant } => {
            let type_name = body_text_field(body, "type_name").unwrap_or_default();
            let key = format!("{app}/{participant}/{type_name}");
            Outcome::Meta(metadata.types.create(&key, StoredDocument { content_type: CONTENT_TYPE_DDS_WEB_XML.to_string(), body: body.to_vec() }).map(|()| MetaResult::Created(format!("applications/{app}/domain_participants/{participant}/registered_types/{type_name}"))))
        }
        RestRoute::UnregisterType {
            app,
            participant,
            type_name,
        } => {
            let key = format!("{app}/{participant}/{type_name}");
            Outcome::Meta(metadata.types.delete(&key).map(|()| MetaResult::Deleted))
        }
        RestRoute::GetRegisteredTypes { app, participant } => {
            let prefix = format!("{app}/{participant}/");
            let names = metadata
                .types
                .names()
                .into_iter()
                .filter_map(|n| n.strip_prefix(&prefix).map(str::to_string))
                .collect();
            Outcome::Meta(Ok(MetaResult::List(names)))
        }

        RestRoute::Unknown { .. } => {
            Outcome::Backend(Err(BackendError::NotFound("no such route".to_string())))
        }
    }
}

fn outcome_to_response(method: RestMethod, outcome: Outcome) -> Response {
    match outcome {
        Outcome::Backend(Ok(result)) => backend_success(method, result),
        Outcome::Backend(Err(e)) => fault_response(method, backend_status(&e), &e_message(&e)),
        Outcome::Meta(Ok(result)) => meta_success(method, result),
        Outcome::Meta(Err(e)) => fault_response(method, meta_status(&e), &e.to_string()),
    }
}

fn backend_success(method: RestMethod, result: BackendResult) -> Response {
    match result {
        BackendResult::Created(path) => Response::empty(
            http_status_for(ReturnStatus::Ok, RestMethod::Post),
            "Created",
        )
        .with_header("Location", &path),
        BackendResult::Deleted => Response::empty(
            http_status_for(ReturnStatus::Ok, method),
            status_reason(method),
        ),
        BackendResult::List(names) => list_body(method, &names),
        BackendResult::Body { content_type, body } => Response::new(
            http_status_for(ReturnStatus::Ok, RestMethod::Get),
            "OK",
            &content_type,
            body,
        ),
    }
}

fn meta_success(method: RestMethod, result: MetaResult) -> Response {
    match result {
        MetaResult::Created(path) => Response::empty(
            http_status_for(ReturnStatus::Ok, RestMethod::Post),
            "Created",
        )
        .with_header("Location", &path),
        MetaResult::Deleted => Response::empty(
            http_status_for(ReturnStatus::Ok, method),
            status_reason(method),
        ),
        MetaResult::List(names) => list_body(method, &names),
    }
}

fn list_body(method: RestMethod, names: &[String]) -> Response {
    let mut emitter = XmlEmitter::new();
    emitter.declaration();
    let list_tag = "list";
    let _ = emitter.start_element(list_tag, &[]);
    for name in names {
        let _ = emitter.start_element("item", &[]);
        emitter.text(name);
        let _ = emitter.end_element();
    }
    let _ = emitter.end_element();
    Response::new(
        http_status_for(ReturnStatus::Ok, method),
        "OK",
        CONTENT_TYPE_DDS_WEB_XML,
        emitter.finish().into_bytes(),
    )
}

fn status_reason(method: RestMethod) -> &'static str {
    match method {
        RestMethod::Delete | RestMethod::Put => "No Content",
        _ => "OK",
    }
}

fn fault_response(method: RestMethod, status: ReturnStatus, message: &str) -> Response {
    let mut emitter = XmlEmitter::new();
    emitter.declaration();
    let _ = emitter.start_element("fault", &[]);
    let _ = emitter.start_element("message", &[]);
    emitter.text(message);
    let _ = emitter.end_element();
    let _ = emitter.end_element();
    Response::new(
        http_status_for(status, method),
        "Error",
        CONTENT_TYPE_DDS_WEB_XML,
        emitter.finish().into_bytes(),
    )
}

fn backend_status(e: &BackendError) -> ReturnStatus {
    match e {
        BackendError::NotFound(_) => ReturnStatus::InvalidObject,
        BackendError::Conflict(_) => ReturnStatus::ObjectAlreadyExists,
        BackendError::BadRequest(_) => ReturnStatus::InvalidInput,
        BackendError::Forbidden => ReturnStatus::PermissionsError,
        BackendError::Internal(_) => ReturnStatus::GenericServiceError,
    }
}

fn meta_status(e: &MetadataError) -> ReturnStatus {
    match e {
        MetadataError::Conflict(_) => ReturnStatus::ObjectAlreadyExists,
        MetadataError::NotFound(_) => ReturnStatus::InvalidObject,
    }
}

fn e_message(e: &BackendError) -> String {
    match e {
        BackendError::NotFound(m)
        | BackendError::Conflict(m)
        | BackendError::BadRequest(m)
        | BackendError::Internal(m) => m.clone(),
        BackendError::Forbidden => "forbidden".to_string(),
    }
}

/// The `readers` list of a `create_waitset`/`update_waitset` body
/// (`<waitset><readers><item>...</item>...</readers></waitset>`, or no
/// `<readers>` at all — a legitimate empty list, not an error).
///
/// A malformed body is refused rather than treated as "no readers":
/// for `update_waitset` in particular, that silent default would
/// detach every condition an existing WaitSet already holds on nothing
/// more than a client's typo — silently destructive, not the harmless
/// "brand-new resource named `readers=[]`" a `create_waitset` typo
/// gives.
///
/// # Errors
/// [`BackendError::BadRequest`] if `body` is not UTF-8 or not
/// well-formed XML.
fn readers_list(body: &[u8]) -> Result<Vec<String>, BackendError> {
    let text = std::str::from_utf8(body)
        .map_err(|e| BackendError::BadRequest(format!("body is not UTF-8: {e}")))?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    xmlfields::parse(text)
        .map(|f| f.list("readers"))
        .map_err(|e| BackendError::BadRequest(e.to_string()))
}

/// Spec §8.3.5 Tab 7: `Accept` is Required, valid value
/// `application/dds-web+xml` — the *only* representation the REST
/// platform's §8.2 defines (checked against the spec text: its four
/// `8.2.*` subsections and Tab 7/Tab 8 name no other format, no JSON
/// among them). `Content-Type` is Optional, same valid value, checked
/// only when present.
///
/// # Errors (as a `Response`, not a `Result`, so a caller with no
/// `RestMethod` yet can still short-circuit)
/// * `400` if `Accept` is missing (a Required header absent is a
///   malformed request, not merely "cannot accept").
/// * `406` if `Accept` is present but names no acceptable media range
///   for `application/dds-web+xml` (`application/dds-web+xml`,
///   `application/*` or `*/*`).
/// * `415` if `Content-Type` is present and is not
///   `application/dds-web+xml` (ignoring a `; charset=...` parameter).
fn check_representation_headers(req: &Request) -> Option<Response> {
    let Some(accept) = req.header("accept") else {
        return Some(text_response(
            400,
            "Bad Request",
            "Accept header is required (Spec DDS-WEB 1.0 SS8.3.5 Tab 7)",
        ));
    };
    if !accept
        .split(',')
        .any(|range| media_range_matches(range.split(';').next().unwrap_or("").trim()))
    {
        return Some(text_response(
            406,
            "Not Acceptable",
            &format!(
                "Accept: {accept} names no representation this daemon serves \
                 (only {CONTENT_TYPE_DDS_WEB_XML})"
            ),
        ));
    }
    if let Some(content_type) = req.header("content-type") {
        let bare = content_type.split(';').next().unwrap_or("").trim();
        if !bare.is_empty() && bare != CONTENT_TYPE_DDS_WEB_XML {
            return Some(text_response(
                415,
                "Unsupported Media Type",
                &format!("Content-Type: {content_type} is not {CONTENT_TYPE_DDS_WEB_XML}"),
            ));
        }
    }
    None
}

fn text_response(status: u16, reason: &'static str, message: &str) -> Response {
    Response::new(status, reason, "text/plain", message.as_bytes().to_vec())
}

/// `true` if `range` (one comma-separated `Accept` token, parameters
/// already stripped) admits `application/dds-web+xml`.
fn media_range_matches(range: &str) -> bool {
    matches!(range, "*/*" | "application/*" | CONTENT_TYPE_DDS_WEB_XML)
}

fn body_text_field(body: &[u8], tag: &str) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    xmlfields::parse(text).ok()?.get(tag).map(str::to_string)
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

/// Unused import guard (`ContentType`/`RepresentationKind`/`element_name`
/// are re-exported for other modules to keep the wire tag registry in
/// one place; `rest_service` itself only needs the generic `list`/`item`
/// tags today).
#[allow(dead_code)]
fn _keep_representation_imports(_: ContentType, _: RepresentationKind) -> &'static str {
    element_name(ContentType::Application, RepresentationKind::Object)
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

    fn accept_header() -> Vec<(String, String)> {
        vec![("Accept".to_string(), CONTENT_TYPE_DDS_WEB_XML.to_string())]
    }

    fn get(target: &str) -> Request {
        Request {
            method: "GET".to_string(),
            target: target.to_string(),
            version: "HTTP/1.1".to_string(),
            headers: accept_header(),
            body: Vec::new(),
        }
    }

    fn post(target: &str, body: &str) -> Request {
        Request {
            method: "POST".to_string(),
            target: target.to_string(),
            version: "HTTP/1.1".to_string(),
            headers: accept_header(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn put(target: &str, body: &str) -> Request {
        Request {
            method: "PUT".to_string(),
            target: target.to_string(),
            version: "HTTP/1.1".to_string(),
            headers: accept_header(),
            body: body.as_bytes().to_vec(),
        }
    }

    #[test]
    fn denied_by_acl_yields_403() {
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        let resp = dispatch(&gw, &meta, &get("/dds/rest1/applications"), false);
        assert_eq!(resp.status, 403);
    }

    #[test]
    fn missing_accept_header_is_400() {
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        let req = Request {
            method: "GET".to_string(),
            target: "/dds/rest1/applications".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let resp = dispatch(&gw, &meta, &req, true);
        assert_eq!(resp.status, 400);
    }

    #[test]
    fn accept_header_naming_an_unserved_representation_is_406() {
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        let req = Request {
            method: "GET".to_string(),
            target: "/dds/rest1/applications".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: vec![("Accept".to_string(), "application/json".to_string())],
            body: Vec::new(),
        };
        let resp = dispatch(&gw, &meta, &req, true);
        assert_eq!(resp.status, 406);
    }

    #[test]
    fn accept_header_wildcards_are_acceptable() {
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        for accept in ["*/*", "application/*", "application/dds-web+xml; q=0.9"] {
            let req = Request {
                method: "GET".to_string(),
                target: "/dds/rest1/applications".to_string(),
                version: "HTTP/1.1".to_string(),
                headers: vec![("Accept".to_string(), accept.to_string())],
                body: Vec::new(),
            };
            let resp = dispatch(&gw, &meta, &req, true);
            assert_eq!(resp.status, 200, "Accept: {accept} should be acceptable");
        }
    }

    #[test]
    fn content_type_naming_an_unserved_representation_is_415() {
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        let req = Request {
            method: "POST".to_string(),
            target: "/dds/rest1/applications".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: vec![
                ("Accept".to_string(), CONTENT_TYPE_DDS_WEB_XML.to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: b"<application><name>a</name></application>".to_vec(),
        };
        let resp = dispatch(&gw, &meta, &req, true);
        assert_eq!(resp.status, 415);
    }

    #[test]
    fn unknown_route_is_404() {
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        let resp = dispatch(&gw, &meta, &get("/not/a/route"), true);
        assert_eq!(resp.status, 404);
    }

    #[test]
    fn create_application_then_list_round_trips() {
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/applications",
                "<application><name>app1</name></application>",
            ),
            true,
        );
        assert_eq!(resp.status, 201);
        assert_eq!(
            resp.headers
                .iter()
                .find(|(k, _)| k == "Location")
                .map(|(_, v)| v.as_str()),
            Some("applications/app1")
        );

        let resp = dispatch(&gw, &meta, &get("/dds/rest1/applications"), true);
        assert_eq!(resp.status, 200);
        let body = String::from_utf8(resp.body).unwrap();
        assert!(body.contains("app1"));
    }

    #[test]
    fn update_waitset_with_a_malformed_body_is_400_not_a_silent_empty_readers_list() {
        // Regression: `readers_list` used to swallow a parse failure
        // into `Vec::new()`, which for `update_waitset` would silently
        // detach every condition an existing WaitSet already holds.
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        let resp = dispatch(
            &gw,
            &meta,
            &put("/dds/rest1/applications/a/waitsets/ws1", "<not-closed"),
            true,
        );
        assert_eq!(
            resp.status, 422,
            "expected 422 Unprocessable Entity, got {resp:?}"
        );
    }

    #[test]
    fn qos_library_crud() {
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/qos_libraries",
                "<qos_library><name>Lib</name></qos_library>",
            ),
            true,
        );
        assert_eq!(resp.status, 201);
        let resp = dispatch(&gw, &meta, &get("/dds/rest1/qos_libraries"), true);
        assert!(String::from_utf8(resp.body).unwrap().contains("Lib"));
    }

    #[test]
    fn update_topic_on_an_unknown_topic_is_404_not_500() {
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        let resp = dispatch(
            &gw,
            &meta,
            &put(
                "/dds/rest1/applications/a/domain_participants/p/topics/T",
                "",
            ),
            true,
        );
        assert_eq!(resp.status, 404);
    }

    #[test]
    fn update_topic_changes_its_qos_and_returns_204() {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "zd-webdds-rest-update-topic-{n}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let idl_path = dir.join("t.idl");
        std::fs::write(&idl_path, "module M { @final struct S { long x; }; };").unwrap();
        let mut types = BTreeMap::new();
        types.insert(
            "M::S".to_string(),
            crate::gateway::TypeSource::Idl(idl_path),
        );
        let gw = Mutex::new(Gateway::new(
            types,
            std::sync::Arc::new(crate::metadata::MetadataStore::new()),
        ));
        let meta = MetadataStore::new();

        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/applications",
                "<application><name>a</name></application>",
            ),
            true,
        );
        assert_eq!(resp.status, 201);
        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/applications/a/domain_participants",
                "<domain_participant><domain_id>0</domain_id></domain_participant>",
            ),
            true,
        );
        assert_eq!(resp.status, 201);
        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/applications/a/domain_participants/p1/topics",
                "<topic><name>T</name><type_name>M::S</type_name></topic>",
            ),
            true,
        );
        assert_eq!(resp.status, 201);

        // TOPIC_DATA is the one Topic policy Changeable=YES post-enable
        // (Spec §2.2.3) — every other policy on an already-enabled
        // topic legitimately refuses (`ImmutablePolicy`), which is
        // correct DDS behavior, not a gateway bug.
        let resp = dispatch(
            &gw,
            &meta,
            &put(
                "/dds/rest1/applications/a/domain_participants/p1/topics/T",
                "<topic><qos><topic_data><value>aGk=</value></topic_data></qos></topic>",
            ),
            true,
        );
        assert_eq!(resp.status, 204, "expected 204 No Content, got {resp:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn head_returns_no_body() {
        let gw = empty_gateway();
        let meta = MetadataStore::new();
        let req = Request {
            method: "HEAD".to_string(),
            target: "/dds/rest1/applications".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: accept_header(),
            body: Vec::new(),
        };
        let resp = dispatch(&gw, &meta, &req, true);
        assert!(resp.body.is_empty());
    }

    #[test]
    fn write_sample_success_is_200_not_201() {
        // `DataWriter::write` is a `POST` that creates no new resource;
        // the generic method->status rule (`POST` => `201`) is for
        // *create* operations, and this route must not pick that up
        // through the request method alone (regression for the bug the
        // crate's own daemon_e2e test caught: a write returning 201 with
        // an empty body and no `Location` header).
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("zd-webdds-rest-write-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let idl_path = dir.join("t.idl");
        std::fs::write(&idl_path, "module M { @final struct S { long x; }; };").unwrap();
        let mut types = std::collections::BTreeMap::new();
        types.insert(
            "M::S".to_string(),
            crate::gateway::TypeSource::Idl(idl_path),
        );
        let gw = Mutex::new(Gateway::new(
            types,
            std::sync::Arc::new(crate::metadata::MetadataStore::new()),
        ));
        let meta = MetadataStore::new();

        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/applications",
                "<application><name>a</name></application>",
            ),
            true,
        );
        assert_eq!(resp.status, 201);
        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/applications/a/domain_participants",
                "<domain_participant><domain_id>0</domain_id></domain_participant>",
            ),
            true,
        );
        assert_eq!(resp.status, 201);
        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/applications/a/domain_participants/p1/topics",
                "<topic><name>T</name><type_name>M::S</type_name></topic>",
            ),
            true,
        );
        assert_eq!(resp.status, 201);
        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/applications/a/domain_participants/p1/publishers",
                "",
            ),
            true,
        );
        assert_eq!(resp.status, 201);
        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/applications/a/domain_participants/p1/publishers/pub1/data_writers",
                "<data_writer><topic_name>T</topic_name></data_writer>",
            ),
            true,
        );
        assert_eq!(resp.status, 201);

        let resp = dispatch(
            &gw,
            &meta,
            &post(
                "/dds/rest1/applications/a/domain_participants/p1/publishers/pub1/data_writers/dw1",
                "<data><x>7</x></data>",
            ),
            true,
        );
        assert_eq!(
            resp.status, 200,
            "expected 200 OK for a successful write, got {resp:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
