// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Live CSIv2 SAS handshake cross-ORB (CSIv2 §16/§24): a ZeroDDS client sends a
//! GSSUP credential to a **JacORB GssUpServer** (SecurePOA with
//! `EstablishTrustInClient`), whose real `SASTargetInterceptor`/`ListGssUpContext`
//! validates it (user table `{jay/test}`). Proves the full SAS flow against a
//! foreign ORB — and directly the `sequence<octet>` fix (JacORB reads the username as
//! byte[]; `"jay\0"` would NOT match).
//!
//! The test builds and starts JacORB's own `demo/sas` server
//! (`tests/support/jacorb.rs`); it runs in the integration job
//! `integration-bridges-vendor-corba`.

use std::path::PathBuf;

use zerodds_corba_interop::runtime::{IiopCorbaConnection, object_reference_from_ior};
use zerodds_corba_rust::CorbaConnection;
use zerodds_wire::Endianness;

#[path = "support/jacorb.rs"]
mod jacorb;

/// JacORB's `demo/sas`: `GssUpServer` behind the SAS interceptor, with
/// `ListGssUpContext` and its user `jay` / `test`.
fn gssup_server() -> (jacorb::Server, String) {
    let demo = jacorb::jacorb_home().join("demo/sas/src/main");
    let java: Vec<PathBuf> = ["GssUpServer.java", "ListGssUpContext.java"]
        .iter()
        .map(|f| demo.join("java/org/jacorb/demo/sas").join(f))
        .collect();
    jacorb::start(&jacorb::Spec {
        name: "jacorb-sas",
        idl: vec![demo.join("idl/server.idl")],
        java,
        main: "org.jacorb.demo.sas.GssUpServer",
        props: &[
            "jacorb.security.sas.contextClass=org.jacorb.demo.sas.ListGssUpContext",
            "org.omg.PortableInterceptor.ORBInitializerClass.SAS=\
             org.jacorb.security.sas.SASInitializer",
            "jacorb.implname=StandardImplName",
        ],
        ior_to_file: true,
        stdout_prefix: "",
    })
}

#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (integration-bridges-vendor-corba)"]
fn corbarust_13_a_jacorb_target_accepts_a_gssup_credential_and_refuses_a_wrong_one() {
    let (_server, ior) = gssup_server();
    let oref = object_reference_from_ior(ior.trim()).expect("parse IOR");

    // Valid credential (JacORB auth_data = {jay/test}).
    let conn = IiopCorbaConnection::new().with_csiv2_credentials("jay", "test");
    // printSAS() has a void/empty body. Success = the JacORB TSS accepted the GSSUP.
    let r = conn.invoke(&oref, "printSAS", Endianness::Big, &[]);
    assert!(
        r.is_ok(),
        "JacORB TSS rejected a valid ZeroDDS GSSUP: {r:?}"
    );

    // Negative: wrong password → JacORB must throw NO_PERMISSION.
    let bad = IiopCorbaConnection::new().with_csiv2_credentials("jay", "wrong");
    let rb = bad.invoke(&oref, "printSAS", Endianness::Big, &[]);
    assert!(
        rb.is_err(),
        "an invalid credential should have been rejected"
    );
}
