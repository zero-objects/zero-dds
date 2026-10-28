// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Go endpoint ping-pong: raw generated codegen over UDP, plus the full stack
//! (generated types + endpoint SDK, sync `Client` and async `AsyncReader`) with
//! XRCE framing. Gated on `go`.
//!
//! The generated codec calls `runtime/go` (module `zdwire`) rather than
//! carrying its own marshaller, so every app here requires two modules: the
//! wire runtime and — for the framing cases — `endpoints/go`
//! (module `zeroddsendpoint`).

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{
    Language, endpoints_dir, runtime_dir, scratch_dir, source, tool_available,
};
use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong, xcdr2_reader, xcdr2_writer};

fn go_available() -> bool {
    tool_available("go", "version")
}

/// The generated `Ping`/`Pong` module, as `package main` with the imports the
/// app adds. The generator emits bare declarations; the package clause and the
/// import block are this harness's to write.
fn gen_module(extra_imports: &str) -> String {
    format!(
        "package main\n\nimport (\n\t\"zdwire\"\n{extra_imports})\n\n{}",
        source(Language::Go)
    )
}

/// The XCDR2 little-endian writer the peer decodes. The `PlCdr1Framing` fields
/// govern PL-CDR1 only and never fire under plain XCDR2; they are spelled out
/// because `NewWriter` takes the struct positionally and a zero value would be
/// a different framing silently.
const GO_WRITER: &str = "\tzd_framing := zdwire.PlCdr1Framing{\n\
     \t\tPidFlags:                  \"sentinel-must-understand\",\n\
     \t\tHighestShortID:            16128,\n\
     \t\tPadDeclaredLength:         true,\n\
     \t\tPadDeclaredOptionalLength: true,\n\
     \t\tMemberMustUnderstand:      true,\n\
     \t\tPadAfterValue:             true,\n\
     \t}\n\
     \tzd_w := zdwire.NewWriter(zdwire.LittleEndian, 4, zd_framing, false)\n\
     \tZd_enc_Ping(zd_w, Ping{Seq: 1, Msg: \"hello from app\"})\n\
     \tsample := zd_w.Finish()\n";

/// Decodes a `Pong` body with the generated decoder and prints the line the
/// peer asserts on.
const GO_PRINT_PONG: &str = "\tzd_r := zdwire.NewReader(body, zdwire.LittleEndian, 4, \
     \"sentinel-must-understand\", 16128)\n\
     \tpong := Zd_dec_Pong(zd_r, zdwire.TryConstructDiscard)\n\
     \tfmt.Printf(\"PONG seq=%d reply=%s\\n\", pong.Seq, pong.Reply)\n";

/// The `go.mod` for an app that needs the wire runtime, and optionally the
/// endpoint SDK.
fn go_mod(with_endpoint: bool) -> String {
    let mut out = format!(
        "module pingpongapp\n\ngo 1.21\n\nrequire zdwire v0.0.0\n\nreplace zdwire => {}\n",
        runtime_dir("go").display()
    );
    if with_endpoint {
        out.push_str(&format!(
            "\nrequire zeroddsendpoint v0.0.0\n\nreplace zeroddsendpoint => {}\n",
            endpoints_dir("go").display()
        ));
    }
    out
}

/// Writes `main.go` + `go.mod` into a fresh directory and runs `go run .`.
fn spawn_go(label: &str, src: &str, with_endpoint: bool, args: &[String]) -> Child {
    let dir = scratch_dir(label);
    std::fs::write(dir.join("main.go"), src).expect("write main.go");
    std::fs::write(dir.join("go.mod"), go_mod(with_endpoint)).expect("write go.mod");
    Command::new("go")
        .arg("run")
        .arg(".")
        .args(args)
        // Pure Go, no cgo: `net` links the system resolver through cgo by
        // default on macOS, and a cgo-linked Go binary aborts before `main`
        // on this host (`dyld: missing LC_UUID load command`). Nothing here
        // needs cgo — the app resolves a numeric loopback address — so the
        // pure-Go resolver is both sufficient and the portable choice.
        .env("CGO_ENABLED", "0")
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn go")
}

// ---- 1) raw codegen over a plain UDP socket ----

const GO_RAW_MAIN: &str = r#"
func main() {
	conn, err := net.Dial("udp", "127.0.0.1:"+os.Args[1])
	if err != nil {
		panic(err)
	}
	defer conn.Close()
__WRITER__
	if _, err := conn.Write(sample); err != nil {
		panic(err)
	}
	buf := make([]byte, 4096)
	_ = conn.SetReadDeadline(time.Now().Add(30 * time.Second))
	n, err := conn.Read(buf)
	if err != nil {
		panic(err)
	}
	body := buf[:n]
__PRINT__
}
"#;

fn build_go_raw(port: u16) -> Child {
    let mut src = gen_module("\t\"fmt\"\n\t\"net\"\n\t\"os\"\n\t\"time\"\n");
    src.push_str(
        &GO_RAW_MAIN
            .replace("__WRITER__", GO_WRITER)
            .replace("__PRINT__", GO_PRINT_PONG),
    );
    spawn_go("go_raw", &src, false, &[port.to_string()])
}

#[test]
fn go_raw_udp() {
    if !go_available() {
        eprintln!("SKIP go_raw_udp: `go` not on PATH");
        return;
    }
    // The raw app sends a bare XCDR2 Ping (no XRCE frame), so the peer's
    // `xrce_unframe` would reject it — the raw case runs its own tiny peer.
    use std::net::UdpSocket;
    use std::time::Duration;
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = build_go_raw(port);
    sock.set_read_timeout(Some(Duration::from_secs(90)))
        .expect("timeout");
    let mut buf = [0u8; 4096];
    let (n, app) = sock.recv_from(&mut buf).expect("recv");
    let mut r = xcdr2_reader(&buf[..n]);
    let seq = r.read_u32().expect("seq");
    let msg = r.read_string().expect("msg");
    assert_eq!((seq, msg.as_str()), (1, "hello from app"));
    let mut w = xcdr2_writer();
    w.write_u32(seq);
    w.write_string(&format!("pong:{msg}"));
    sock.send_to(&w.finish(), app).expect("send");
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
__WRITER__
	var body []byte
	if mode == "async" {
		reader := zerodds.NewAsyncReader(tr)
		defer reader.Close()
		if err := zerodds.NewAsyncWriter(tr, zerodds.XrceObjectID(0x001, uint16(zerodds.ObjkDataWriter))).Write(sample); err != nil {
			panic(err)
		}
		select {
		case body = <-reader.Samples:
		case <-time.After(30 * time.Second):
			panic("async: no pong")
		}
	} else {
		client := zerodds.NewClient(tr, zerodds.XrceObjectID(0x001, uint16(zerodds.ObjkDataWriter)))
		if err := client.Write(sample); err != nil {
			panic(err)
		}
		var ok bool
		body, ok, err = client.Receive(30 * time.Second)
		if err != nil {
			panic(err)
		}
		if !ok {
			panic("sync: no pong")
		}
	}
__PRINT__
}
"#;

fn build_go_endpoint(mode: &str, port: u16) -> Child {
    let mut src =
        gen_module("\t\"fmt\"\n\t\"net\"\n\t\"os\"\n\t\"time\"\n\n\tzerodds \"zeroddsendpoint\"\n");
    src.push_str(
        &GO_ENDPOINT_MAIN
            .replace("__WRITER__", GO_WRITER)
            .replace("__PRINT__", GO_PRINT_PONG),
    );
    spawn_go(
        &format!("go_ep_{mode}"),
        &src,
        true,
        &[mode.to_string(), port.to_string()],
    )
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
