// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Go endpoint ping-pong: raw generated codegen over UDP, plus the full stack
//! (generated types + endpoint SDK, sync `Client` and async `AsyncReader`) with
//! XRCE framing. Gated on `go`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::Path;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_go::{GoGenOptions, generate_go_module};

fn go_available() -> bool {
    Command::new("go").arg("version").output().is_ok()
}

/// Generated Ping/Pong module (package main) with the imports the app needs.
fn gen_module(extra_imports: &str) -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    let src = generate_go_module(
        &ast,
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.replacen(
        "import \"math\"\n",
        &format!("import (\n\t\"math\"\n{extra_imports})\n"),
        1,
    )
}

// ---- 1) raw codegen over a plain UDP socket ----

const GO_RAW_MAIN: &str = r#"
func main() {
	conn, err := net.Dial("udp", "127.0.0.1:"+os.Args[1])
	if err != nil {
		panic(err)
	}
	defer conn.Close()
	if _, err := conn.Write((Ping{Seq: 1, Msg: "hello from app"}).MarshalXCDR(Little)); err != nil {
		panic(err)
	}
	buf := make([]byte, 4096)
	_ = conn.SetReadDeadline(time.Now().Add(10 * time.Second))
	n, err := conn.Read(buf)
	if err != nil {
		panic(err)
	}
	pong := UnmarshalXCDRPong(buf[:n], Little)
	fmt.Printf("PONG seq=%d reply=%s\n", pong.Seq, pong.Reply)
}
"#;

fn build_go_raw(port: u16) -> Child {
    let mut src = gen_module("\t\"fmt\"\n\t\"net\"\n\t\"os\"\n\t\"time\"\n");
    // The raw app sends a bare XCDR2 sample (no XRCE frame), so the peer's
    // xrce_unframe would reject it — the raw case uses its own tiny harness.
    src.push_str(GO_RAW_MAIN);
    let dir = std::env::temp_dir().join(format!("pp_go_raw_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.go"), &src).expect("write");
    Command::new("go")
        .arg("run")
        .arg(dir.join("main.go"))
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn go")
}

#[test]
fn go_raw_udp() {
    if !go_available() {
        eprintln!("SKIP go_raw_udp: `go` not on PATH");
        return;
    }
    // The raw app sends a bare XCDR2 Ping (no XRCE frame); run a minimal peer.
    use std::net::UdpSocket;
    use std::time::Duration;
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = build_go_raw(port);
    sock.set_read_timeout(Some(Duration::from_secs(30)))
        .expect("timeout");
    let mut buf = [0u8; 4096];
    let (n, app) = sock.recv_from(&mut buf).expect("recv");
    let mut r = BufferReader::new(&buf[..n], Endianness::Little).xcdr2();
    let seq = r.read_u32().expect("seq");
    let msg = r.read_string().expect("msg");
    assert_eq!((seq, msg.as_str()), (1, "hello from app"));
    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    w.write_u32(seq).expect("s");
    w.write_string(&format!("pong:{msg}")).expect("r");
    sock.send_to(&w.into_bytes(), app).expect("send");
    let out = child.wait_with_output().expect("wait");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("PONG seq=1 reply=pong:hello from app"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---- 2/3) full stack: generated types + endpoint SDK (sync/async) over XRCE ----

const GO_ENDPOINT_MAIN: &str = r#"
type udpTransport struct {
	conn *net.UDPConn
	peer *net.UDPAddr
}

func (u *udpTransport) Deliver(frame []byte) error {
	_, err := u.conn.WriteToUDP(frame, u.peer)
	return err
}

func (u *udpTransport) Receive(buf []byte) (int, bool, error) {
	_ = u.conn.SetReadDeadline(time.Now().Add(10 * time.Millisecond))
	n, _, err := u.conn.ReadFromUDP(buf)
	if err != nil {
		if ne, ok := err.(net.Error); ok && ne.Timeout() {
			return 0, true, nil
		}
		return 0, false, err
	}
	return n, false, nil
}

func main() {
	mode := os.Args[1]
	peer, err := net.ResolveUDPAddr("udp", "127.0.0.1:"+os.Args[2])
	if err != nil {
		panic(err)
	}
	conn, err := net.ListenUDP("udp", nil)
	if err != nil {
		panic(err)
	}
	defer conn.Close()
	tr := &udpTransport{conn: conn, peer: peer}
	sample := (Ping{Seq: 1, Msg: "hello from app"}).MarshalXCDR(Little)
	var body []byte
	if mode == "async" {
		reader := zerodds.NewAsyncReader(tr)
		defer reader.Close()
		if err := zerodds.NewAsyncWriter(tr).Write(sample); err != nil {
			panic(err)
		}
		select {
		case body = <-reader.Samples:
		case <-time.After(10 * time.Second):
			panic("async: no pong")
		}
	} else {
		client := zerodds.NewClient(tr)
		if err := client.Write(sample); err != nil {
			panic(err)
		}
		var ok bool
		body, ok, err = client.Receive(10 * time.Second)
		if err != nil {
			panic(err)
		}
		if !ok {
			panic("sync: no pong")
		}
	}
	pong := UnmarshalXCDRPong(body, Little)
	fmt.Printf("PONG seq=%d reply=%s\n", pong.Seq, pong.Reply)
}
"#;

fn build_go_endpoint(mode: &str, port: u16) -> Child {
    let mut src =
        gen_module("\t\"fmt\"\n\t\"net\"\n\t\"os\"\n\t\"time\"\n\n\tzerodds \"zeroddsendpoint\"\n");
    src.push_str(GO_ENDPOINT_MAIN);
    let endpoints_go = format!("{}/../../endpoints/go", env!("CARGO_MANIFEST_DIR"));
    let dir = std::env::temp_dir().join(format!("pp_go_ep_{mode}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.go"), &src).expect("write");
    std::fs::write(
        dir.join("go.mod"),
        format!(
            "module pingpongapp\n\ngo 1.19\n\nrequire zeroddsendpoint v0.0.0\n\nreplace zeroddsendpoint => {}\n",
            Path::new(&endpoints_go).canonicalize().expect("endpoints/go path").display()
        ),
    )
    .expect("go.mod");
    Command::new("go")
        .arg("run")
        .arg(".")
        .arg(mode)
        .arg(port.to_string())
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn go")
}

fn endpoint_case(mode: &str) {
    if !go_available() {
        eprintln!("SKIP go endpoint {mode}: `go` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_go_endpoint(mode, peer.port);
    ping_pong(&peer, child, &format!("go/{mode}"));
}

#[test]
fn go_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn go_endpoint_async() {
    endpoint_case("async");
}
