// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Portable interceptor routing (§16.4): real OTS (`TransactionService` id 0)
//! and CSIv2 (`SecurityAttributeService` id 15) service contexts are injected/extracted
//! through the interceptor pipeline — the spec-clean architecture instead of
//! hardcoded SC wiring in the transport. Plus `forward_reference` (§16.4.5).

use std::sync::{Arc, Mutex};

use zerodds_cdr::Endianness;
use zerodds_corba_ccm::orb_extensions::{
    ClientInterceptionPoint, ClientRequestInterceptor, InterceptorRegistry, RequestInfo,
    ServerInterceptionPoint, ServerRequestInterceptor, ServiceContextInjector,
};
use zerodds_corba_cos_transactions::{Otid, PropagationContext, TRANSACTION_SERVICE_CONTEXT_ID};
use zerodds_corba_csiv2::gssup::GssupCredentialToken;

const SAS_ID: u32 = 15; // SecurityAttributeService

#[test]
fn ots_and_csiv2_service_contexts_route_through_pi() {
    // --- Client: OTS PropagationContext + CSIv2 GSSUP as interceptors ---
    let otid = Otid::new(7, vec![1, 2, 3, 4]);
    let ots_bytes = PropagationContext::flat(30, otid.clone())
        .to_service_context_data(Endianness::Big)
        .unwrap();
    let gss = GssupCredentialToken::new("alice".into(), "secret".into(), b"target".to_vec())
        .to_gss_token(Endianness::Big)
        .unwrap();

    let mut client_reg = InterceptorRegistry::new();
    client_reg.add_client(Arc::new(ServiceContextInjector::new(
        "OTSInterceptor",
        TRANSACTION_SERVICE_CONTEXT_ID,
        ots_bytes.clone(),
    )));
    client_reg.add_client(Arc::new(ServiceContextInjector::new(
        "CSIv2Interceptor",
        SAS_ID,
        gss.clone(),
    )));

    let mut info = RequestInfo::new(1, "transfer");
    client_reg.run_client(ClientInterceptionPoint::SendRequest, &mut info);

    // Both SCs are now in the request — injected via PI, not hardcoded.
    let sc0 = info.get_request_service_context(0).expect("OTS SC missing");
    assert_eq!(sc0, ots_bytes.as_slice());
    let pc = PropagationContext::from_service_context_data(sc0).unwrap();
    assert_eq!(pc.current.otid, otid);
    assert_eq!(
        info.get_request_service_context(SAS_ID).unwrap(),
        gss.as_slice()
    );

    // --- Server: interceptor evaluates the SCs (SC list = as decoded from the wire) ---
    struct ServerExtractor {
        otid: Mutex<Option<Otid>>,
        user: Mutex<Option<String>>,
    }
    impl ServerRequestInterceptor for ServerExtractor {
        fn name(&self) -> &str {
            "extractor"
        }
        fn receive_request_service_contexts(&self, info: &mut RequestInfo) {
            if let Some(b) = info.get_request_service_context(0) {
                if let Ok(pc) = PropagationContext::from_service_context_data(b) {
                    *self.otid.lock().unwrap() = Some(pc.current.otid);
                }
            }
            if let Some(b) = info.get_request_service_context(SAS_ID) {
                if let Ok(tok) = GssupCredentialToken::from_gss_token(b) {
                    *self.user.lock().unwrap() = Some(tok.username);
                }
            }
        }
    }
    let extractor = Arc::new(ServerExtractor {
        otid: Mutex::new(None),
        user: Mutex::new(None),
    });
    let mut server_reg = InterceptorRegistry::new();
    server_reg.add_server(extractor.clone());

    let mut server_info = RequestInfo::new(1, "transfer");
    for (id, data) in info.request_service_contexts() {
        server_info.add_request_service_context(*id, data.clone());
    }
    server_reg.run_server(
        ServerInterceptionPoint::ReceiveRequestServiceContexts,
        &mut server_info,
    );

    assert_eq!(*extractor.otid.lock().unwrap(), Some(otid));
    assert_eq!(extractor.user.lock().unwrap().as_deref(), Some("alice"));
}

#[test]
fn forward_reference_via_receive_other() {
    struct Redirector;
    impl ClientRequestInterceptor for Redirector {
        fn name(&self) -> &str {
            "redirector"
        }
        fn receive_other(&self, info: &mut RequestInfo) {
            info.set_forward_reference(vec![0xCA, 0xFE]); // LOCATION_FORWARD-IOR
        }
    }
    let mut reg = InterceptorRegistry::new();
    reg.add_client(Arc::new(Redirector));
    let mut info = RequestInfo::new(2, "op");
    reg.run_client(ClientInterceptionPoint::ReceiveOther, &mut info);
    assert_eq!(info.forward_reference(), Some(&[0xCA, 0xFE][..]));
}

#[test]
fn reply_service_context_injection() {
    // Server adds a reply service context (e.g. an OTS commit acknowledgment).
    struct ReplyTagger;
    impl ServerRequestInterceptor for ReplyTagger {
        fn name(&self) -> &str {
            "reply-tagger"
        }
        fn send_reply(&self, info: &mut RequestInfo) {
            info.add_reply_service_context(0, vec![0x01]);
        }
    }
    let mut reg = InterceptorRegistry::new();
    reg.add_server(Arc::new(ReplyTagger));
    let mut info = RequestInfo::new(3, "op");
    reg.run_server(ServerInterceptionPoint::SendReply, &mut info);
    assert_eq!(info.get_reply_service_context(0), Some(&[0x01][..]));
}
