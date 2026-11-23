// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 2 §7.6: the client side of object references with
//! several profiles and addresses, and Object URLs through the ORB.
//!
//! - §7.6.3: every IIOP profile of a reference is tried in the order of
//!   the IOR; a reference with no usable profile raises `TRANSIENT` minor
//!   2 (no address reachable) or `IMP_LIMIT` minor 1 (no IIOP profile).
//! - §7.6.6.2: the `TAG_ALTERNATE_IIOP_ADDRESS` addresses of a profile
//!   are tried after its own address.
//! - §9.4.2.1: `ReferenceAddr` names the profile the request went over.
//! - §7.6.9: the case of a stringified IOR is not significant.
//! - §7.6.10: `corbaloc` with several IIOP 1.0 addresses, an IPv6 host,
//!   and the `http` and `ftp` schemes of Table 7.1 through
//!   `ORB::string_to_object`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;
use zerodds_corba_interop::string_argument;

use zerodds_corba_ccm::orb_core::{Orb, ThreadingMode};
use zerodds_corba_giop::{
    AddressingDisposition, Message, Reply, ReplyStatusType, ServiceContextList, TargetAddress,
    Version,
};
use zerodds_corba_iiop::{Acceptor, Connection, IiopProfileBody, IiopVersion, TaggedComponent};
use zerodds_corba_interop::runtime::{
    AmiClient, CorbaServer, IiopCorbaConnection, LocateOutcome, object_reference,
    object_reference_from_ior, stringify_object_reference,
};
use zerodds_corba_interop::{decode_string_body, encode_string_body};
use zerodds_corba_ior::{AlternateIiopAddress, ComponentId, StructuredComponent};
use zerodds_corba_rt::RawProfile;
use zerodds_corba_rust::{
    CorbaConnection, CorbaException, ObjectReference, SkeletonResult, SystemExceptionKind,
};
use zerodds_wire::{Endianness, Writer};

/// A server on `bind` whose object `key` answers every call with `text`.
fn answering(bind: &str, key: &'static [u8], text: &'static str) -> (CorbaServer, Acceptor, u16) {
    let server = CorbaServer::new();
    server.register(key, move |_op, _body, _e| {
        SkeletonResult::Reply(encode_string_body(text))
    });
    let acceptor = server.serve(bind.parse().unwrap()).unwrap();
    let port = acceptor.listen_addr().port();
    (server, acceptor, port)
}

/// A port of 127.0.0.1 nothing listens on.
fn dead_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn call(target: &ObjectReference) -> Result<String, CorbaException> {
    IiopCorbaConnection::new()
        .invoke(target, "ping", Endianness::Big, &string_argument("x"))
        .map(|(body, _)| decode_string_body(&body).unwrap())
}

fn iiop_profile(port: u16, key: &[u8], alternates: &[(&str, u16)]) -> Vec<u8> {
    let mut body = IiopProfileBody::new(IiopVersion::V1_2, "127.0.0.1".into(), port, key.to_vec());
    for (host, port) in alternates {
        let c = StructuredComponent::AlternateIiopAddress(AlternateIiopAddress {
            host: (*host).to_string(),
            port: *port,
        });
        body.components.push(TaggedComponent {
            tag: ComponentId::AlternateIiopAddress.as_u32(),
            component_data: c.encode_encapsulation(Endianness::Big).unwrap(),
        });
    }
    body.encode_encapsulation(Endianness::Big).unwrap()
}

fn system(r: Result<String, CorbaException>) -> zerodds_corba_rust::SystemException {
    match r {
        Err(CorbaException::System(e)) => e,
        other => panic!("not a system exception: {other:?}"),
    }
}

#[test]
fn corba_p2_7_6_6_2_client_falls_back_to_an_alternate_address() {
    let (_server, _acceptor, live) = answering("127.0.0.1:0", b"Echo", "from alternate");
    let reference = ObjectReference::from_profiles(
        "IDL:Echo:1.0".into(),
        vec![RawProfile {
            tag: 0,
            data: iiop_profile(dead_port(), b"Echo", &[("127.0.0.1", live)]),
        }],
    );
    assert_eq!(call(&reference).unwrap(), "from alternate");
    // The LocateRequest goes the same way.
    assert!(matches!(
        IiopCorbaConnection::new().locate(&reference).unwrap(),
        LocateOutcome::Here(_)
    ));
    // The AMI client too.
    let mut ami = AmiClient::connect(&reference).unwrap();
    let id = ami.send_poll("ping", &string_argument("x")).unwrap();
    assert!(matches!(ami.get_reply(id), Ok(Ok(_))));
}

#[test]
fn corba_p2_7_6_3_client_falls_back_to_the_next_profile() {
    let (_server, _acceptor, live) = answering("127.0.0.1:0", b"Echo", "from second profile");
    let reference = ObjectReference::from_profiles(
        "IDL:Echo:1.0".into(),
        vec![
            // TAG_MULTIPLE_COMPONENTS, no IIOP: skipped.
            RawProfile {
                tag: 1,
                data: vec![0, 0, 0, 0, 0, 0, 0, 0],
            },
            RawProfile {
                tag: 0,
                data: iiop_profile(dead_port(), b"Echo", &[]),
            },
            RawProfile {
                tag: 0,
                data: iiop_profile(live, b"Echo", &[]),
            },
        ],
    );
    assert_eq!(call(&reference).unwrap(), "from second profile");
}

/// A GIOP peer that asks for `ReferenceAddr` and then answers with the
/// `selected_profile_index` and the profile count it was sent.
fn reference_addr_peer() -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut conn = Connection::from_stream(stream).unwrap();
        while let Ok((msg, e)) = conn.read_message_with_endianness() {
            let Message::Request(req) = msg else { continue };
            let (status, body) = match &req.target {
                TargetAddress::Key(_) => {
                    let mut w = Writer::new(e);
                    AddressingDisposition::ReferenceAddr.encode(&mut w);
                    (ReplyStatusType::NeedsAddressingMode, w.finish())
                }
                TargetAddress::Reference(info) => (
                    ReplyStatusType::NoException,
                    encode_string_body(&format!(
                        "{}/{}",
                        info.selected_profile_index,
                        info.profiles.len()
                    )),
                ),
                TargetAddress::Profile(_) => {
                    (ReplyStatusType::NoException, encode_string_body("profile"))
                }
            };
            let reply = Message::Reply(Reply {
                request_id: req.request_id,
                reply_status: status,
                service_context: ServiceContextList::default(),
                body,
            });
            conn.write_message(Version::V1_2, e, false, &reply).unwrap();
        }
    });
    (port, handle)
}

/// Part 2 §9.4.2.1: `ReferenceAddr` carries the whole IOR and the index
/// of the profile the request went over.
#[test]
fn corba_p2_9_4_2_1_reference_addr_names_the_profile_the_request_used() {
    let (port, _peer) = reference_addr_peer();
    let reference = ObjectReference::from_profiles(
        "IDL:Echo:1.0".into(),
        vec![
            RawProfile {
                tag: 1,
                data: vec![0, 0, 0, 0, 0, 0, 0, 0],
            },
            RawProfile {
                tag: 0,
                data: iiop_profile(dead_port(), b"Echo", &[]),
            },
            RawProfile {
                tag: 0,
                data: iiop_profile(port, b"Echo", &[]),
            },
        ],
    );
    assert_eq!(call(&reference).unwrap(), "2/3");
}

#[test]
fn corba_p2_7_6_3_no_usable_profile_is_transient_2() {
    let reference = ObjectReference::from_profiles(
        "IDL:Echo:1.0".into(),
        vec![
            RawProfile {
                tag: 0,
                data: iiop_profile(dead_port(), b"Echo", &[("127.0.0.1", dead_port())]),
            },
            RawProfile {
                tag: 0,
                data: iiop_profile(dead_port(), b"Echo", &[]),
            },
        ],
    );
    let e = system(call(&reference));
    assert_eq!(e.kind, SystemExceptionKind::Transient);
    assert_eq!(e.omg_minor(), Some(2));
    let Err(CorbaException::System(e)) = AmiClient::connect(&reference) else {
        panic!("the AMI client connected")
    };
    assert_eq!(
        (e.kind, e.omg_minor()),
        (SystemExceptionKind::Transient, Some(2))
    );
}

#[test]
fn corba_p2_7_6_3_a_reference_without_an_iiop_profile_is_imp_limit_1() {
    for profiles in [
        vec![RawProfile {
            tag: 3,
            data: vec![0, 1],
        }],
        // An IIOP profile that does not decode.
        vec![RawProfile {
            tag: 0,
            data: vec![0, 1],
        }],
    ] {
        let reference = ObjectReference::from_profiles("IDL:Echo:1.0".into(), profiles);
        let e = system(call(&reference));
        assert_eq!(e.kind, SystemExceptionKind::ImpLimit);
        assert_eq!(e.omg_minor(), Some(1));
    }
}

/// Part 2 §7.6.10.1: a `corbaloc` with two IIOP 1.0 addresses; the
/// first is dead, the client reaches the object at the second.
#[test]
fn corba_p2_7_6_10_corbaloc_with_two_iiop_1_0_addresses_reaches_the_second() {
    let (_server, _acceptor, live) = answering("127.0.0.1:0", b"Dev/Echo", "second address");
    let orb = Orb::init(ThreadingMode::SingleThreaded);
    let obj = orb
        .string_to_object(&format!(
            "corbaloc::127.0.0.1:{},:127.0.0.1:{live}/Dev/Echo",
            dead_port()
        ))
        .unwrap();
    assert_eq!(call(obj.reference().unwrap()).unwrap(), "second address");
}

/// Part 2 §7.6.10.3: `"[" IPv6_address "]"`, with and without a port.
#[test]
fn corba_p2_7_6_10_an_ipv6_corbaloc_reaches_the_object() {
    let (_server, _acceptor, live) = answering("[::1]:0", b"Echo", "over IPv6");
    let orb = Orb::init(ThreadingMode::SingleThreaded);
    let obj = orb
        .string_to_object(&format!("corbaloc:iiop:1.2@[::1]:{live}/Echo"))
        .unwrap();
    assert_eq!(call(obj.reference().unwrap()).unwrap(), "over IPv6");
}

/// Part 2 §7.6.9: "The case for characters in a stringified IOR is not
/// significant": `object_reference_from_ior` reads the prefix and the hex
/// digits in either case and calls the object; text without the `IOR:`
/// scheme is BAD_PARAM minor 7 (§7.6.10), hex that does not decode minor
/// 9.
#[test]
fn corba_p2_7_6_9_object_reference_from_ior_ignores_the_prefix_case() {
    let (_server, _acceptor, port) = answering("127.0.0.1:0", b"Echo", "any case");
    let target = object_reference("IDL:Echo:1.0", "127.0.0.1", port, b"Echo");
    let ior = stringify_object_reference(&target).unwrap();
    let hex = &ior["IOR:".len()..];
    for text in [
        format!("ior:{hex}"),
        format!("Ior:{hex}"),
        format!("iOR:{}", hex.to_uppercase()),
    ] {
        let reference = object_reference_from_ior(&text).unwrap();
        assert_eq!(reference, target, "{text}");
        assert_eq!(call(&reference).unwrap(), "any case");
    }
    let minor = |text: &str| match object_reference_from_ior(text) {
        Err(CorbaException::System(e)) => {
            assert_eq!(e.kind, SystemExceptionKind::BadParam);
            e.omg_minor()
        }
        other => panic!("{text}: {other:?}"),
    };
    assert_eq!(minor(&format!("corbaloc::127.0.0.1:{port}/Echo")), Some(7));
    assert_eq!(minor("x"), Some(7));
    assert_eq!(minor("ior:0"), Some(9));
    assert_eq!(minor("ior:zz"), Some(9));
}

/// One HTTP exchange: reads the request head, checks it, writes `answer`.
fn http_once(listener: &TcpListener, path: &str, answer: &str) {
    let (stream, _) = listener.accept().unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut head = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        head.push_str(&line);
    }
    assert!(
        head.starts_with(&format!("GET {path} HTTP/1.1\r\n")),
        "{head}"
    );
    assert!(head.contains("\r\nHost: 127.0.0.1:"), "{head}");
    let mut stream = stream;
    stream.write_all(answer.as_bytes()).unwrap();
}

/// Part 2 §7.6.10.7, Table 7.1: an `http` URL returns an IOR (with
/// `Content-Length`), a URL (chunked), or redirects; a `404` is BAD_PARAM
/// minor 10. A redirect to an absolute path, a relative path or a network
/// path is resolved against the request URL (RFC 9110 §10.2.2, RFC 3986
/// §5.2); a redirect to a `file` URL is refused with minor 10.
#[test]
fn corba_p2_7_6_10_7_an_http_url_yields_the_ior() {
    let orb = Orb::init(ThreadingMode::SingleThreaded);
    let target = object_reference("IDL:Echo:1.0", "echo.example", 4242, b"Echo");
    let ior = zerodds_corba_interop::runtime::stringify_object_reference(&target).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = format!("{ior}\n");
    let answers = vec![
        (
            "/refs/echo.ior".to_string(),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        ),
        (
            "/loc".to_string(),
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\
             9\r\ncorbaloc:\r\n17\r\n:echo.example:4242/Echo\r\n0\r\n\r\n"
                .to_string(),
        ),
        (
            "/old".to_string(),
            "HTTP/1.1 302 Found\r\nLocation: /refs/echo.ior\r\nContent-Length: 0\r\n\r\n"
                .to_string(),
        ),
        (
            "/refs/echo.ior".to_string(),
            format!("HTTP/1.0 200 OK\r\n\r\n{body}"),
        ),
        (
            "/missing".to_string(),
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string(),
        ),
        (
            "/refs/old".to_string(),
            "HTTP/1.1 301 Moved Permanently\r\nLocation: echo.ior\r\n\r\n".to_string(),
        ),
        (
            "/refs/echo.ior".to_string(),
            format!("HTTP/1.0 200 OK\r\n\r\n{body}"),
        ),
        (
            "/moved".to_string(),
            format!(
                "HTTP/1.1 307 Temporary Redirect\r\n\
                 Location: //127.0.0.1:{port}/x/../refs/./echo.ior\r\nContent-Length: 0\r\n\r\n"
            ),
        ),
        (
            "/refs/echo.ior".to_string(),
            format!("HTTP/1.0 200 OK\r\n\r\n{body}"),
        ),
        (
            "/elsewhere".to_string(),
            "HTTP/1.1 302 Found\r\nLocation: file:///etc/hostname\r\n\r\n".to_string(),
        ),
    ];
    let server = std::thread::spawn(move || {
        for (path, answer) in answers {
            http_once(&listener, &path, &answer);
        }
    });
    let base = format!("http://127.0.0.1:{port}");
    let obj = orb
        .string_to_object(&format!("{base}/refs/echo.ior"))
        .unwrap();
    assert_eq!(obj.reference(), Some(&target));
    let obj = orb.string_to_object(&format!("{base}/loc")).unwrap();
    let p = IiopProfileBody::decode_encapsulation(&obj.reference().unwrap().iiop_profile).unwrap();
    assert_eq!(
        (p.host.as_str(), p.port, p.object_key.as_slice()),
        ("echo.example", 4242, &b"Echo"[..])
    );
    let obj = orb
        .string_to_object(&format!("HTTP://127.0.0.1:{port}/old"))
        .unwrap();
    assert_eq!(obj.reference(), Some(&target));
    let e = orb
        .string_to_object(&format!("{base}/missing"))
        .unwrap_err();
    assert_eq!(e.omg_minor(), Some(10));
    let obj = orb.string_to_object(&format!("{base}/refs/old")).unwrap();
    assert_eq!(obj.reference(), Some(&target));
    let obj = orb.string_to_object(&format!("{base}/moved")).unwrap();
    assert_eq!(obj.reference(), Some(&target));
    let e = orb
        .string_to_object(&format!("{base}/elsewhere"))
        .unwrap_err();
    assert_eq!(e.omg_minor(), Some(10));
    server.join().unwrap();
}

/// A minimal FTP server (RFC 959, RFC 2428 `EPSV`) for one session that
/// serves `content` as `dir/name`; it records the commands it got.
fn ftp_server(
    dir: &'static str,
    name: &'static str,
    content: String,
) -> (u16, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut out = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);
        let mut commands = Vec::new();
        let mut cwd_ok = false;
        let mut data: Option<TcpListener> = None;
        // A multi-line greeting (RFC 959 §4.2).
        out.write_all(b"220-zerodds test server\r\n220 ready\r\n")
            .unwrap();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap() == 0 {
                break;
            }
            let line = line.trim_end().to_string();
            commands.push(line.clone());
            let (cmd, arg) = line.split_once(' ').unwrap_or((line.as_str(), ""));
            let answer = match cmd {
                "USER" => "331 password please".to_string(),
                "PASS" => "230 logged in".to_string(),
                "TYPE" => "200 type set".to_string(),
                "CWD" if arg == dir => {
                    cwd_ok = true;
                    "250 ok".to_string()
                }
                "CWD" => "550 no such directory".to_string(),
                "EPSV" => {
                    let l = TcpListener::bind("127.0.0.1:0").unwrap();
                    let p = l.local_addr().unwrap().port();
                    data = Some(l);
                    format!("229 Entering Extended Passive Mode (|||{p}|)")
                }
                "RETR" if cwd_ok && arg == name => {
                    out.write_all(b"150 opening\r\n").unwrap();
                    let (mut d, _) = data.take().unwrap().accept().unwrap();
                    d.write_all(content.as_bytes()).unwrap();
                    drop(d);
                    "226 done".to_string()
                }
                "RETR" => "550 no such file".to_string(),
                "QUIT" => {
                    out.write_all(b"221 bye\r\n").unwrap();
                    break;
                }
                _ => "502 not implemented".to_string(),
            };
            out.write_all(format!("{answer}\r\n").as_bytes()).unwrap();
        }
        commands
    });
    (port, handle)
}

/// Part 2 §7.6.10.7, Table 7.1: an `ftp` URL names a file holding the
/// IOR; the client logs in, changes to the directory and retrieves the
/// file in passive mode.
#[test]
fn corba_p2_7_6_10_7_an_ftp_url_yields_the_ior() {
    let orb = Orb::init(ThreadingMode::SingleThreaded);
    let target = object_reference("IDL:Echo:1.0", "echo.example", 4242, b"Echo");
    let ior = zerodds_corba_interop::runtime::stringify_object_reference(&target).unwrap();
    let (port, server) = ftp_server("refs", "echo.ior", format!("{ior}\r\n"));
    let obj = orb
        .string_to_object(&format!("ftp://127.0.0.1:{port}/refs/echo.ior"))
        .unwrap();
    assert_eq!(obj.reference(), Some(&target));
    let commands = server.join().unwrap();
    assert_eq!(
        commands,
        [
            "USER anonymous",
            "PASS anonymous@",
            "TYPE I",
            "CWD refs",
            "EPSV",
            "RETR echo.ior",
            "QUIT"
        ]
    );
    // A file the server does not have: BAD_PARAM minor 10.
    let (port, server) = ftp_server("refs", "echo.ior", String::new());
    let e = orb
        .string_to_object(&format!("ftp://u:pw@127.0.0.1:{port}/refs/other.ior"))
        .unwrap_err();
    assert_eq!(e.omg_minor(), Some(10));
    let commands = server.join().unwrap();
    assert_eq!(&commands[..2], ["USER u", "PASS pw"]);
    // RFC 1738 §3.2.2: `;type=a` reads the file with `TYPE A`.
    let (port, server) = ftp_server("refs", "echo.ior", format!("{ior}\r\n"));
    let obj = orb
        .string_to_object(&format!("ftp://127.0.0.1:{port}/refs/echo.ior;type=a"))
        .unwrap();
    assert_eq!(obj.reference(), Some(&target));
    let commands = server.join().unwrap();
    assert_eq!(
        &commands[2..],
        ["TYPE A", "CWD refs", "EPSV", "RETR echo.ior", "QUIT"]
    );
}
