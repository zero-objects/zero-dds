// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Go reliable-stream endpoint: the pure-Go reliable state machine + frame
//! codec (`endpoints/go/reliable.go`) driven by its async-decoupled writer
//! against the shared Rust reliable peer, which injects loss: the peer
//! drops, the app retransmits on ACKNACK, all samples arrive gap-free. The
//! samples are the `Ping` of the codec the Go generator writes from the IDL
//! (`codegen::source`), so the run checks IDL, generated codec and runtime
//! together; the in-process runnable example, which the endpoints-gate column of
//! Go does not run; and the producer-latency comparison. The unit and
//! byte-golden suite runs in that column (`go test`). Gated on `go`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, endpoints_dir, runtime_dir, scratch_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

/// A `go` command with cgo off.
///
/// `net` links the system resolver through cgo by default on macOS, and a
/// cgo-linked Go binary aborts before `main` on this host (`dyld: missing
/// LC_UUID load command`). Nothing here needs cgo — the apps resolve numeric
/// loopback addresses — so the pure-Go resolver is both sufficient and the
/// portable choice. `tests/go.rs` sets the same variable for the same reason.
fn go_command() -> Command {
    let mut cmd = Command::new("go");
    cmd.env("CGO_ENABLED", "0");
    cmd
}

fn go_available() -> bool {
    go_command().arg("version").output().is_ok()
}

fn endpoints_go() -> PathBuf {
    PathBuf::from(format!("{}/../../endpoints/go", env!("CARGO_MANIFEST_DIR")))
}

/// Builds `./<pkg>` (a `main` package under `endpoints/go`) with `go build`
/// into a uniquely-tagged temp binary. The module (`zeroddsendpoint`) already
/// lives in place, so no scaffolding/copying is needed — just build in-tree.
fn go_build(pkg: &str, tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zd_go_reliable_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let bin = dir.join(pkg.trim_start_matches("./"));
    let out = go_command()
        .arg("build")
        .arg("-o")
        .arg(&bin)
        .arg(format!("./{pkg}"))
        .current_dir(endpoints_go())
        .output()
        .expect("spawn go build");
    assert!(
        out.status.success(),
        "go build ./{pkg} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    bin
}

/// The reliable app: the `Ping` codec `codegen::source(Language::Go)` writes
/// from the IDL, and a sender over the SDK's async-decoupled
/// `ReliableAsyncWriter` (`endpoints/go/reliable.go`) that enqueues the
/// encoded `Ping { Seq: i, Msg: "reliable" }` for each sample.
const GO_RELIABLE_MAIN: &str = r#"
type udpTransport struct {
	conn *net.UDPConn
}

func (u *udpTransport) Deliver(frame []byte) error {
	_, err := u.conn.Write(frame)
	return err
}

func (u *udpTransport) Receive(buf []byte) (int, bool, error) {
	_ = u.conn.SetReadDeadline(time.Now().Add(20 * time.Millisecond))
	n, err := u.conn.Read(buf)
	if err != nil {
		if ne, ok := err.(net.Error); ok && ne.Timeout() {
			return 0, true, nil
		}
		return 0, false, err
	}
	return n, false, nil
}

func main() {
	port, _ := strconv.Atoi(os.Args[1])
	n, _ := strconv.Atoi(os.Args[2])
	addr, err := net.ResolveUDPAddr("udp", fmt.Sprintf("127.0.0.1:%d", port))
	if err != nil {
		panic(err)
	}
	conn, err := net.DialUDP("udp", nil, addr)
	if err != nil {
		panic(err)
	}
	defer conn.Close()
	w := zerodds.NewReliableAsyncWriter(&udpTransport{conn: conn}, 0,
		zerodds.XrceObjectID(0x001, uint16(zerodds.ObjkDataWriter)))
	// XCDR2 little-endian; the PL_CDR1 fields never fire under plain XCDR2.
	zd_framing := zdwire.PlCdr1Framing{
		PidFlags:                  "sentinel-must-understand",
		HighestShortID:            16128,
		PadDeclaredLength:         true,
		PadDeclaredOptionalLength: true,
		MemberMustUnderstand:      true,
		PadAfterValue:             true,
	}
	for i := 0; i < n; i++ {
		zd_w := zdwire.NewWriter(zdwire.LittleEndian, 4, zd_framing, false)
		Zd_enc_Ping(zd_w, Ping{Seq: int32(i), Msg: "reliable"})
		w.Enqueue(zd_w.Finish())
	}
	// Returns once every sample was submitted and acknowledged.
	w.Close()
	fmt.Printf("SENT count=%d\n", n)
}
"#;

/// Writes the generated module and the reliable app into a scratch module
/// with `go.mod` naming `runtime/go` and `endpoints/go`, and builds it.
fn build_reliable_app(tag: &str) -> PathBuf {
    let dir = scratch_dir(&format!("go_reliable_{tag}"));
    let src = format!(
        "package main\n\nimport (\n\t\"fmt\"\n\t\"net\"\n\t\"os\"\n\t\"strconv\"\n\t\"time\"\n\n\
         \t\"zdwire\"\n\tzerodds \"zeroddsendpoint\"\n)\n\n{}\n{GO_RELIABLE_MAIN}",
        source(Language::Go)
    );
    std::fs::write(dir.join("main.go"), src).expect("write main.go");
    std::fs::write(
        dir.join("go.mod"),
        format!(
            "module reliableapp\n\ngo 1.21\n\nrequire zdwire v0.0.0\n\nreplace zdwire => {}\n\n\
             require zeroddsendpoint v0.0.0\n\nreplace zeroddsendpoint => {}\n",
            runtime_dir("go").display(),
            endpoints_dir("go").display()
        ),
    )
    .expect("write go.mod");
    let bin = dir.join("reliable_app");
    let out = go_command()
        .arg("build")
        .arg("-o")
        .arg(&bin)
        .arg(".")
        .current_dir(&dir)
        .output()
        .expect("spawn go build");
    assert!(
        out.status.success(),
        "go build of the reliable app failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    bin
}

fn spawn_reliable_app(bin: &std::path::Path, port: u16, n: usize) -> Child {
    Command::new(bin)
        .arg(port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn go reliable app")
}

fn run_loss(drop_every: Option<usize>, tag: &str, label: &str) {
    if !go_available() {
        eprintln!("SKIP {label}: `go` not on PATH");
        return;
    }
    let n = RELIABLE_SAMPLES;
    let bin = build_reliable_app(tag);
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = spawn_reliable_app(&bin, peer.port, n);
    let delivered = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &delivered, n);
}

/// reliable-endpoint 1.0 §5 point 3: the Go app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Go toolchain; runs in integration-bindings"]
fn reliable_5_3_go_samples_arrive_in_order_despite_dropped_datagrams() {
    // Peer drops every 3rd distinct sample once → the app must retransmit.
    run_loss(Some(3), "loss", "go/loss");
}

/// reliable-endpoint 1.0 §5 point 5: the Go reliable example runs and
/// reports its sequence without gaps; the endpoints-gate column of Go
/// builds it without running it.
#[test]
#[ignore = "needs the Go toolchain; runs in integration-bindings"]
fn reliable_5_5_go_example_recovers_its_sequence_without_gaps() {
    if !go_available() {
        eprintln!("SKIP go reliable example: `go` not on PATH");
        return;
    }
    let out = go_command()
        .arg("run")
        .arg("./example_reliable")
        .current_dir(endpoints_go())
        .output()
        .expect("go run example_reliable");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("RELIABLE OK"),
        "go example_reliable did not report RELIABLE OK:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("sequence 0..11 verified in order"),
        "go example_reliable did not verify the recovered sequence:\nstdout: {stdout}"
    );
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Go producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the Go toolchain; runs in integration-bindings"]
fn reliable_5_4_go_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !go_available() {
        eprintln!("SKIP go reliable latency bench: `go` not on PATH");
        return;
    }
    let bin = go_build("reliable_bench", "bench");
    // No live peer needed (an arbitrary loopback port): only local dispatch
    // cost (channel enqueue vs. inline syscall) is under measurement.
    let out = Command::new(&bin).arg("9").output().expect("run bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find(|l| l.starts_with("BENCH "))
        .unwrap_or_else(|| {
            panic!(
                "no BENCH line:\nstdout: {stdout}\nstderr: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
    let decoupled = parse_kv(line, "decoupled_ns=");
    let inline_send = parse_kv(line, "inline_ns=");
    eprintln!(
        "go async-writer producer latency: decoupled={decoupled} ns, inline(send)={inline_send} ns"
    );
    assert!(
        decoupled < inline_send,
        "the decoupled enqueue ({decoupled} ns) must cost less than the inline send ({inline_send} ns)"
    );
}

fn parse_kv(line: &str, key: &str) -> u64 {
    line.split_whitespace()
        .find_map(|tok| tok.strip_prefix(key))
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("missing {key} in `{line}`"))
}
