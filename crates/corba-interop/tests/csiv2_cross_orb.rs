// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]
//! Live CSIv2 SAS handshake cross-ORB (CSIv2 §16/§24): a ZeroDDS client sends a
//! GSSUP credential to a **JacORB GssUpServer** (SecurePOA with
//! `EstablishTrustInClient`), whose real `SASTargetInterceptor`/`ListGssUpContext`
//! validates it (user table `{jay/test}`). Proves the full SAS flow against a
//! foreign ORB — and directly the `sequence<octet>` fix (JacORB reads the username as
//! byte[]; `"jay\0"` would NOT match).
//!
//! Runs only with a live JacORB GssUpServer whose IOR is provided via `SECURE_IOR`
//! (the Linux test host). Ignored by default.

use zerodds_cdr::Endianness;
use zerodds_corba_interop::runtime::{IiopCorbaConnection, object_reference_from_ior};
use zerodds_corba_rust::CorbaConnection;

#[test]
#[ignore = "needs live JacORB GssUpServer via SECURE_IOR env (codepit)"]
fn zerodds_gssup_accepted_by_jacorb_tss() {
    let ior = std::env::var("SECURE_IOR").expect("SECURE_IOR env (JacORB GssUpServer-IOR)");
    let oref = object_reference_from_ior(ior.trim()).expect("parse IOR");

    // Valid credential (JacORB auth_data = {jay/test}).
    let conn = IiopCorbaConnection::new().with_csiv2_credentials("jay", "test");
    // printSAS() has a void/empty body. Success = the JacORB TSS accepted the GSSUP.
    let r = conn.invoke(&oref, "printSAS", Endianness::Big, &[]);
    assert!(
        r.is_ok(),
        "JacORB TSS rejected a valid ZeroDDS GSSUP: {r:?}"
    );
    eprintln!("cross-ORB CSIv2 OK: JacORB TSS accepted ZeroDDS GSSUP (jay/test)");

    // Negative: wrong password → JacORB must throw NO_PERMISSION.
    let bad = IiopCorbaConnection::new().with_csiv2_credentials("jay", "wrong");
    let rb = bad.invoke(&oref, "printSAS", Endianness::Big, &[]);
    assert!(
        rb.is_err(),
        "an invalid credential should have been rejected"
    );
    eprintln!("cross-ORB CSIv2 negative OK: invalid credential rejected");
}
