// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]
//! SSLIOP interop server: a ZeroDDS CORBA server that offers Echo over **TLS**
//! (rustls) and prints its reference as a stringified IOR with a
//! `TAG_SSL_SEC_TRANS` component. A foreign ORB (omniORB with SSL transport)
//! or the ZeroDDS `ssliop_client` uses it to select the TLS path.
//!
//! Usage: `ssliop_server <cert.pem> <key.pem>` (binds 127.0.0.1:0).

use std::sync::Arc;

use zerodds_corba_interop::runtime::{CorbaServer, stringify_object_ref_ssl};
use zerodds_corba_rust::CorbaException;

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));
use corba_gen::{Echo, dispatch_echo};

struct EchoImpl;
impl Echo for EchoImpl {
    fn ping(&self, msg: String) -> Result<String, CorbaException> {
        Ok(msg)
    }
}

fn main() {
    let cert_path = std::env::args().nth(1).expect("cert.pem argument missing");
    let key_path = std::env::args().nth(2).expect("key.pem argument missing");
    let cert = std::fs::read(&cert_path).expect("cert.pem lesen");
    let key = std::fs::read(&key_path).expect("key.pem lesen");
    let server_cfg =
        zerodds_corba_iiop::tls::load_server_config(&cert, &key).expect("TLS-ServerConfig");

    let server = CorbaServer::new();
    let echo = Arc::new(EchoImpl);
    server.register(b"Echo", move |op, body, e| {
        dispatch_echo(&*echo, op, body, e)
    });

    let acceptor = server
        .serve_tls("127.0.0.1:0".parse().unwrap(), server_cfg)
        .expect("serve_tls");
    let addr = acceptor.listen_addr();
    // SSLIOP IOR: IIOP ProfileBody port 0 (no cleartext), SSL port = the bound
    // TLS port in the TAG_SSL_SEC_TRANS component.
    println!(
        "SSLIOP_IOR={}",
        stringify_object_ref_ssl("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), b"Echo")
    );
    println!("LISTENING={addr}");
    use std::io::Write;
    std::io::stdout().flush().unwrap();

    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
