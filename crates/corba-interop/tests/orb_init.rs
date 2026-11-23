// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 1 §8.5 over IIOP: an ORB from `orb_init` resolves an
//! initial reference given as a `corbaname:` URL by calling `resolve`
//! on a naming context served over IIOP, and the resulting reference
//! reaches the named object.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_corba_ccm::orb_core::ResolveError;
use zerodds_corba_cosnaming::wire::{encode_object_reply, read_name};
use zerodds_corba_cosnaming::{Name, NameComponent};
use zerodds_corba_interop::decode_string_body;
use zerodds_corba_interop::runtime::{
    CorbaServer, IiopCorbaConnection, object_reference, orb_init, stringify_object_reference,
};
use zerodds_corba_interop::string_argument;
use zerodds_corba_ior::from_stringified;
use zerodds_corba_rust::{CorbaConnection, CorbaException, SkeletonResult};
use zerodds_wire::Endianness;

/// One server with an `Echo` object and a naming context that knows
/// `apps/echo.obj` under the key `NameService`.
fn serve() -> (zerodds_corba_iiop::Acceptor, String) {
    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| match op {
        "ping" => SkeletonResult::Reply(body.bytes().to_vec()),
        _ => SkeletonResult::BadOperation,
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let host = addr.ip().to_string();
    let echo = object_reference("IDL:Echo:1.0", &host, addr.port(), b"Echo");
    let echo_ior = from_stringified(&stringify_object_reference(&echo).unwrap()).unwrap();
    let known: Name = vec![
        NameComponent::new("apps"),
        NameComponent::with_kind("echo", "obj"),
    ];
    server.register(b"NameService", move |op, body, e| {
        assert_eq!(op, "resolve");
        let name = read_name(&mut body.reader(e)).unwrap();
        if name == known {
            SkeletonResult::Reply(encode_object_reply(&echo_ior, e).unwrap())
        } else {
            let mut w = zerodds_wire::Writer::new(e);
            w.write_string("IDL:omg.org/CosNaming/NamingContext/NotFound:1.0");
            w.write_u32(0); // missing_node
            w.write_u32(0); // rest_of_name: empty
            SkeletonResult::Exception(CorbaException::UserException {
                repository_id: "IDL:omg.org/CosNaming/NamingContext/NotFound:1.0".into(),
                body: zerodds_corba_rust::AlignedBody::new(&w.finish(), 0),
                endianness: e,
            })
        }
    });
    (acceptor, format!("{host}:{}", addr.port()))
}

#[test]
fn corba_p1_8_5_3_2_an_init_ref_corbaname_resolves_over_iiop() {
    let (acceptor, hostport) = serve();
    let mut argv = vec![
        "-ORBInitRef".to_string(),
        format!("EchoService=corbaname::{hostport}/NameService#apps/echo.obj"),
    ];
    let orb = orb_init(&mut argv, "interop-8-5-3-2").unwrap();
    assert!(argv.is_empty());
    let echo = orb.resolve_initial_references("EchoService").unwrap();
    let echo = echo.reference().expect("a remote reference").clone();
    assert_eq!(echo.type_id, "IDL:Echo:1.0");

    let conn = IiopCorbaConnection::new();
    let (reply, _) = conn
        .invoke(
            &echo,
            "ping",
            Endianness::Big,
            &string_argument("via corbaname"),
        )
        .unwrap();
    assert_eq!(decode_string_body(&reply).unwrap(), "via corbaname");

    // A name the context does not know: BAD_PARAM minor 10 out of
    // string_to_object (Annex A.6).
    let e = orb
        .string_to_object(&format!("corbaname::{hostport}/NameService#apps/missing"))
        .unwrap_err();
    assert_eq!(e.omg_minor(), Some(10));

    // -ORBDefaultInitRef appends "/<id>": corbaloc to the Echo key.
    let mut argv = vec![
        "-ORBDefaultInitRef".to_string(),
        format!("corbaloc:iiop:1.2@{hostport}"),
    ];
    let orb2 = orb_init(&mut argv, "interop-8-5-3-3").unwrap();
    let by_default = orb2.resolve_initial_references("Echo").unwrap();
    let (reply, _) = conn
        .invoke(
            by_default.reference().unwrap(),
            "ping",
            Endianness::Big,
            &string_argument("via default"),
        )
        .unwrap();
    assert_eq!(decode_string_body(&reply).unwrap(), "via default");

    // An id nothing configures and no default base: InvalidName.
    assert_eq!(
        orb.resolve_initial_references("NoSuchService").unwrap_err(),
        ResolveError::InvalidName
    );
    orb.destroy().unwrap();
    orb2.destroy().unwrap();
    acceptor.shutdown();
}
