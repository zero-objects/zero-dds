// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-vendor interop tests for the WebSocket bridge.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §10 + §12.3; §5.2 (the served IDL
//! through Cyclone DDS `idlc`).
//!
//! Spec §10 on the DDS side: `wsbridge_10_*` runs a Cyclone DDS participant
//! (`tests/interop/cyclone_reading.c`, built by the test with `idlc` and the
//! C compiler) and a Fast DDS participant (`tests/interop/fastdds_reading.cpp`,
//! built with the C++ compiler) and RTI Connext DDS 7.7.0 (the `shape_main`
//! the OMG `dds-rtps` suite releases, `ShapeType`) against the daemon. They
//! need `cyclonedds-dev`, `libfastdds-dev` and that executable, so they are
//! `#[ignore]`d behind the feature `cross-vendor-tests`; the weekly job
//! `integration-bridges-vendor` provisions all three and runs them:
//! ```bash
//! cargo test -p zerodds-websocket-bridge --features cross-vendor-tests \
//!     --test cross_vendor -- --ignored
//! ```

#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

#[cfg(feature = "cross-vendor-tests")]
mod cyclone {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use zerodds_websocket_bridge::codec::{decode, encode};
    use zerodds_websocket_bridge::daemon::config::{DaemonConfig, TopicConfig};
    use zerodds_websocket_bridge::daemon::server;
    use zerodds_websocket_bridge::frame::{Frame, Opcode};

    fn interop_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/interop")
    }

    /// The Cyclone DDS participant, built from its sources with `idlc` and
    /// the C compiler.
    fn cyclone_reading() -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ws-cyclone-reading");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for file in ["reading.idl", "cyclone_reading.c"] {
            std::fs::copy(interop_dir().join(file), dir.join(file)).unwrap();
        }
        let idlc = Command::new("idlc")
            .arg("reading.idl")
            .current_dir(&dir)
            .status()
            .expect("idlc; install cyclonedds-dev");
        assert!(idlc.success(), "idlc failed");
        let cc = Command::new("cc")
            .args([
                "-o",
                "cyclone_reading",
                "cyclone_reading.c",
                "reading.c",
                "-I.",
                "-lddsc",
            ])
            .current_dir(&dir)
            .status()
            .expect("cc");
        assert!(cc.success(), "the Cyclone DDS participant does not build");
        dir.join("cyclone_reading")
    }

    /// The Fast DDS participant of the same command line, built from its
    /// source with the C++ compiler against `libfastdds-dev`.
    fn fastdds_reading() -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ws-fastdds-reading");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(
            interop_dir().join("fastdds_reading.cpp"),
            dir.join("fastdds_reading.cpp"),
        )
        .unwrap();
        let cxx = Command::new("c++")
            .args([
                "-std=c++17",
                "-o",
                "fastdds_reading",
                "fastdds_reading.cpp",
                "-lfastdds",
                "-lfastcdr",
            ])
            .current_dir(&dir)
            .status()
            .expect("c++; install g++ and libfastdds-dev");
        assert!(cxx.success(), "the Fast DDS participant does not build");
        dir.join("fastdds_reading")
    }

    /// The lines a child writes to stdout, read on a thread.
    fn lines_of(child: &mut Child) -> std::sync::mpsc::Receiver<String> {
        let out = child.stdout.take().expect("piped stdout");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = tx.send(line);
            }
        });
        rx
    }

    /// A WebSocket client of the daemon: one connection, masked frames.
    struct Client {
        stream: TcpStream,
        acc: Vec<u8>,
    }

    impl Client {
        /// The opening handshake on `path`, with `protocol` as
        /// `Sec-WebSocket-Protocol` where given.
        fn open(addr: &str, path: &str, protocol: Option<&str>) -> Self {
            let mut stream = TcpStream::connect(addr).expect("connect");
            stream
                .set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            let protocol = protocol
                .map(|p| format!("Sec-WebSocket-Protocol: {p}\r\n"))
                .unwrap_or_default();
            stream
                .write_all(
                    format!(
                        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\n\
                         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                         Sec-WebSocket-Version: 13\r\n{protocol}\r\n"
                    )
                    .as_bytes(),
                )
                .unwrap();
            let mut acc = Vec::new();
            let mut buf = [0u8; 4096];
            let deadline = Instant::now() + Duration::from_secs(3);
            let end = loop {
                if let Some(i) = acc.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
                assert!(Instant::now() < deadline, "no handshake response");
                match stream.read(&mut buf) {
                    Ok(0) => panic!("the daemon closed the handshake"),
                    Ok(n) => acc.extend_from_slice(&buf[..n]),
                    Err(_) => {}
                }
            };
            let head = String::from_utf8_lossy(&acc[..end]).into_owned();
            assert!(head.starts_with("HTTP/1.1 101"), "{head}");
            Self {
                stream,
                acc: acc[end..].to_vec(),
            }
        }

        fn send(&mut self, frame: Frame) {
            let wire = encode(&frame.with_mask([0x5a, 0x17, 0xc3, 0x08])).unwrap();
            self.stream.write_all(&wire).unwrap();
        }

        /// The next data frame within `within`; Pings are answered.
        fn next(&mut self, within: Duration) -> Option<Frame> {
            let deadline = Instant::now() + within;
            let mut buf = [0u8; 4096];
            loop {
                if let Ok((frame, used)) = decode(&self.acc) {
                    self.acc.drain(..used);
                    if frame.opcode == Opcode::Ping {
                        let mut pong = Frame::binary(frame.payload.clone());
                        pong.opcode = Opcode::Pong;
                        self.send(pong);
                        continue;
                    }
                    return Some(frame);
                }
                if Instant::now() >= deadline {
                    return None;
                }
                match self.stream.read(&mut buf) {
                    Ok(0) => return None,
                    Ok(n) => self.acc.extend_from_slice(&buf[..n]),
                    Err(_) => {}
                }
            }
        }
    }

    /// The daemon with the topic `SensorReading` of type `Sensor::Reading`
    /// (`tests/interop/reading.idl`) at `/topics/sensor`, RELIABLE, both
    /// directions.
    fn daemon_config() -> (DaemonConfig, zerodds_test_domains::DomainLease) {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.listen = "127.0.0.1:0".to_string();
        let lease = zerodds_test_domains::lease_domain();
        cfg.domain = lease.id();
        cfg.topics.push(TopicConfig {
            name: "SensorReading".to_string(),
            type_name: "Sensor::Reading".to_string(),
            direction: "bidir".to_string(),
            ws_path: "/topics/sensor".to_string(),
            reliability: "reliable".to_string(),
            durability: "volatile".to_string(),
            history_depth: 10,
            idl_file: interop_dir().join("reading.idl").display().to_string(),
            ..TopicConfig::default()
        });
        (cfg, lease)
    }

    /// The `data` of a JSON sample frame of `SensorReading`, if `frame` is
    /// one.
    fn json_data(frame: &Frame) -> Option<serde_json::Value> {
        if frame.opcode != Opcode::Text {
            return None;
        }
        let v: serde_json::Value = serde_json::from_slice(&frame.payload).ok()?;
        (v.get("topic")? == "SensorReading").then(|| v.get("data").cloned())?
    }

    /// Spec §10: the daemon is a plain RTPS peer of Cyclone DDS. A JSON
    /// `publish` of a WebSocket client reaches a Cyclone DDS reader as that
    /// sample; a sample a Cyclone DDS writer writes reaches a JSON-mode
    /// client as JSON and a binary-mode client as a ZDB1 frame; the binary
    /// frame, sent back by the client, reaches the Cyclone DDS reader again.
    #[test]
    #[ignore = "needs Cyclone DDS (cyclonedds-dev, idlc) and a C compiler; runs in integration-bridges-vendor"]
    fn wsbridge_10_a_ws_publish_reaches_cyclone_and_a_cyclone_sample_a_ws_client() {
        ws_against(&cyclone_reading(), "Cyclone DDS");
    }

    /// Spec §5.2 with a foreign IDL compiler: the IDL `/schema/<slug>`
    /// serves for a topic whose `idl_file` includes another file compiles
    /// with Cyclone DDS `idlc` in a directory without that file, and `idlc`
    /// writes the C type of the topic.
    #[test]
    #[ignore = "needs Cyclone DDS (cyclonedds-dev, idlc); runs in integration-bridges-vendor"]
    fn wsbridge_5_2_cyclone_idlc_compiles_the_served_idl() {
        let src = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ws-schema-src");
        let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ws-schema-idlc");
        for d in [&src, &out] {
            let _ = std::fs::remove_dir_all(d);
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(
            src.join("geo.idl"),
            "#ifndef GEO_IDL\n#define GEO_IDL\nmodule Geo { struct Point { double x; double y; }; };\n#endif\n",
        )
        .unwrap();
        std::fs::write(
            src.join("track.idl"),
            "#include \"geo.idl\"\nmodule Nav { struct Track { @key long id; sequence<Geo::Point> points; }; };\n",
        )
        .unwrap();
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.listen = "127.0.0.1:0".to_string();
        let lease = zerodds_test_domains::lease_domain();
        cfg.domain = lease.id();
        cfg.topics.push(TopicConfig {
            name: "Nav::Track".to_string(),
            type_name: "Nav::Track".to_string(),
            direction: "out".to_string(),
            idl_file: src.join("track.idl").display().to_string(),
            ..TopicConfig::default()
        });
        let mut handle = server::start(cfg).expect("daemon start");
        let mut s = TcpStream::connect(&handle.local_addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        s.write_all(b"GET /schema/nav/track.idl HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).ok();
        handle.shutdown();
        assert!(resp.starts_with("HTTP/1.1 200 OK\r\n"), "{resp}");
        let body = &resp[resp.find("\r\n\r\n").unwrap() + 4..];
        assert!(!body.contains("#include"), "{body}");
        std::fs::write(out.join("track.idl"), body).unwrap();
        let idlc = Command::new("idlc")
            .arg("track.idl")
            .current_dir(&out)
            .output()
            .expect("idlc; install cyclonedds-dev");
        assert!(
            idlc.status.success(),
            "idlc refused the served IDL: {}\n{body}",
            String::from_utf8_lossy(&idlc.stderr)
        );
        let header = std::fs::read_to_string(out.join("track.h")).expect("track.h");
        assert!(header.contains("Nav_Track"), "{header}");
        assert!(header.contains("Geo_Point"), "{header}");
    }

    /// Spec §10 with Fast DDS 3 (the type built with its DynamicTypes API,
    /// XCDR2): the same exchanges as with Cyclone DDS.
    #[test]
    #[ignore = "needs Fast DDS (libfastdds-dev) and a C++ compiler; runs in integration-bridges-vendor"]
    fn wsbridge_10_fastdds_a_ws_publish_reaches_fastdds_and_a_fastdds_sample_a_ws_client() {
        ws_against(&fastdds_reading(), "Fast DDS");
    }

    /// The exchanges of §10 against the foreign participant `peer` (the
    /// command line of `cyclone_reading.c`), named `vendor` in the messages.
    fn ws_against(peer: &std::path::Path, vendor: &str) {
        let (cfg, lease) = daemon_config();
        let domain = lease.id().to_string();
        let mut handle = server::start(cfg).expect("daemon start");
        let addr = handle.local_addr.clone();

        // WebSocket → DDS: a JSON publish, taken by the foreign reader.
        let mut sub = Command::new(peer)
            .args(["sub", &domain, "40"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("the foreign subscriber");
        let lines = lines_of(&mut sub);
        assert_eq!(
            lines.recv_timeout(Duration::from_secs(10)).expect("ready"),
            "{\"event\":\"ready\"}"
        );
        let mut json = Client::open(&addr, "/topics/sensor", Some("zerodds-ws-bridge-1.0+json"));
        let mut binary = Client::open(&addr, "/topics/sensor", None);
        let value = serde_json::json!({ "id": 9, "value": 4.5, "unit": "W" });
        let publish =
            serde_json::json!({ "op": "publish", "topic": "SensorReading", "data": value });
        let deadline = Instant::now() + Duration::from_secs(20);
        let taken = loop {
            // Until the daemon's writer and the foreign reader have matched.
            json.send(Frame::text(publish.to_string()));
            if let Ok(line) = lines.recv_timeout(Duration::from_secs(1)) {
                break line;
            }
            if Instant::now() >= deadline {
                // What the daemon saw: the readers its writer matched and the
                // frames it sent the client (an error frame names a refusal).
                let writer = *handle.user_writers.get("SensorReading").unwrap();
                let matched = handle.runtime.user_writer_matched_reader_guids(writer);
                let mut frames = Vec::new();
                while let Some(f) = json.next(Duration::from_millis(100)) {
                    frames.push(String::from_utf8_lossy(&f.payload).into_owned());
                }
                panic!(
                    "no sample reached {vendor}; matched readers {matched:02x?}, frames {frames:?}"
                );
            }
        };
        let taken: serde_json::Value = serde_json::from_str(&taken).expect("a JSON line");
        assert_eq!(taken, value);

        // DDS → WebSocket: the foreign writer writes, both clients get the sample.
        let written = Command::new(peer)
            .args(["pub", &domain, "10", "5.5", "Hz"])
            .output()
            .expect("the foreign publisher");
        assert!(
            written.status.success(),
            "{}",
            String::from_utf8_lossy(&written.stderr)
        );
        let want = serde_json::json!({ "id": 10, "value": 5.5, "unit": "Hz" });
        let until = Instant::now() + Duration::from_secs(10);
        let mut got_json = false;
        while !got_json && Instant::now() < until {
            if let Some(frame) = json.next(Duration::from_millis(200)) {
                got_json = json_data(&frame).as_ref() == Some(&want);
            }
        }
        assert!(
            got_json,
            "the JSON client did not get the sample of {vendor}"
        );
        // The binary client: a ZDB1 frame whose CDR body carries the unit.
        let until = Instant::now() + Duration::from_secs(10);
        let zdb1 = loop {
            assert!(
                Instant::now() < until,
                "the binary client did not get the sample of {vendor}"
            );
            let Some(frame) = binary.next(Duration::from_millis(200)) else {
                continue;
            };
            if frame.opcode == Opcode::Binary
                && frame.payload.starts_with(b"ZDB1")
                && frame.payload.windows(3).any(|w| w == b"Hz\0")
            {
                break frame.payload;
            }
        };
        assert_eq!(zdb1[4], 0, "an alive sample has no flag");

        // The binary frame back into DDS: the foreign reader takes the sample again.
        while lines.try_recv().is_ok() {}
        binary.send(Frame::binary(zdb1));
        let deadline = Instant::now() + Duration::from_secs(10);
        let again = loop {
            let line = lines
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|_| panic!("the binary frame did not reach {vendor}"));
            let v: serde_json::Value = serde_json::from_str(&line).expect("a JSON line");
            if v == want {
                break v;
            }
        };
        assert_eq!(again, want);
        let _ = sub.kill();
        let _ = sub.wait();
        handle.shutdown();
    }

    /// The `shape_main` of RTI Connext DDS 7.7.0 that the OMG `dds-rtps`
    /// suite releases (`tests/interop/omg/vendors.tsv`); the provision tool
    /// `connext-shape-main` of `tests/integration/provision.sh` puts it at
    /// the default path, `CONNEXT_SHAPE_MAIN` names another.
    fn connext_shape_main() -> PathBuf {
        let exe = std::env::var("CONNEXT_SHAPE_MAIN")
            .unwrap_or_else(|_| "/opt/omg-connext/connext_dds-7.7.0_shape_main_linux".to_string());
        assert!(
            std::path::Path::new(&exe).is_file(),
            "no {exe}; provision connext-shape-main"
        );
        PathBuf::from(exe)
    }

    /// `shape_main` with `args`, its stdout line-buffered (it buffers a
    /// pipe in full otherwise) and read on a thread.
    fn shape_main(
        exe: &std::path::Path,
        args: &[&str],
    ) -> (Child, std::sync::mpsc::Receiver<String>) {
        let mut child = Command::new("stdbuf")
            .arg("-oL")
            .arg(exe)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("stdbuf and shape_main");
        let lines = lines_of(&mut child);
        (child, lines)
    }

    /// The sample line of `shape_main` (`Square     BLUE       010 020 [30]`)
    /// as color, x, y and shapesize; `None` for any other line.
    fn shape_line(line: &str) -> Option<(String, i64, i64, i64)> {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() != 5 || f[0] != "Square" {
            return None;
        }
        let size = f[4].strip_prefix('[')?.strip_suffix(']')?;
        Some((
            f[1].to_string(),
            f[2].parse().ok()?,
            f[3].parse().ok()?,
            size.parse().ok()?,
        ))
    }

    /// Spec §10 with RTI Connext DDS 7.7.0, the `shape_main` of the OMG
    /// `dds-rtps` suite (`ShapeType` of `interop/shapes/omg-dds-rtps.idl`,
    /// `@appendable`, XCDR2 with `-x 2`): a JSON `publish` of a WebSocket
    /// client reaches the Connext reader as that shape; a shape the Connext
    /// writer writes reaches a JSON-mode client as JSON and a binary-mode
    /// client as a ZDB1 frame; the binary frame, sent back, reaches the
    /// Connext reader again.
    #[test]
    #[ignore = "needs the Connext shape_main of the OMG dds-rtps suite; runs in integration-bridges-vendor"]
    fn wsbridge_10_rti_a_ws_publish_reaches_connext_and_a_connext_sample_a_ws_client() {
        let exe = connext_shape_main();
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.listen = "127.0.0.1:0".to_string();
        let lease = zerodds_test_domains::lease_domain();
        cfg.domain = lease.id();
        cfg.topics.push(TopicConfig {
            name: "Square".to_string(),
            type_name: "ShapeType".to_string(),
            direction: "bidir".to_string(),
            ws_path: "/topics/square".to_string(),
            reliability: "reliable".to_string(),
            durability: "volatile".to_string(),
            history_depth: 10,
            idl_file: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../interop/shapes/omg-dds-rtps.idl")
                .display()
                .to_string(),
            ..TopicConfig::default()
        });
        let domain = lease.id().to_string();
        let mut handle = server::start(cfg).expect("daemon start");
        let addr = handle.local_addr.clone();

        // WebSocket → DDS: a JSON publish, taken by the Connext reader.
        let (mut sub, lines) = shape_main(&exe, &["-S", "-t", "Square", "-d", &domain, "-x", "2"]);
        let ready = Instant::now() + Duration::from_secs(10);
        loop {
            let line = lines
                .recv_timeout(ready.saturating_duration_since(Instant::now()))
                .expect("Create reader for topic");
            if line.starts_with("Create reader for topic") {
                break;
            }
        }
        let mut json = Client::open(&addr, "/topics/square", Some("zerodds-ws-bridge-1.0+json"));
        let mut binary = Client::open(&addr, "/topics/square", None);
        let shape = serde_json::json!({
            "color": "BLUE", "x": 10, "y": 20, "shapesize": 30, "additional_payload_size": []
        });
        let publish = serde_json::json!({ "op": "publish", "topic": "Square", "data": shape });
        let deadline = Instant::now() + Duration::from_secs(20);
        let taken = 'taken: loop {
            json.send(Frame::text(publish.to_string()));
            let tick = Instant::now() + Duration::from_secs(1);
            while let Ok(line) = lines.recv_timeout(tick.saturating_duration_since(Instant::now()))
            {
                if let Some(s) = shape_line(&line) {
                    break 'taken s;
                }
            }
            if Instant::now() >= deadline {
                let writer = *handle.user_writers.get("Square").unwrap();
                let matched = handle.runtime.user_writer_matched_reader_guids(writer);
                let mut frames = Vec::new();
                while let Some(f) = json.next(Duration::from_millis(100)) {
                    frames.push(String::from_utf8_lossy(&f.payload).into_owned());
                }
                panic!(
                    "no shape reached Connext; matched readers {matched:02x?}, frames {frames:?}"
                );
            }
        };
        assert_eq!(taken, ("BLUE".to_string(), 10, 20, 30));

        // DDS → WebSocket: the Connext writer writes RED shapes of size 30.
        let (mut publisher, _) = shape_main(
            &exe,
            &[
                "-P", "-t", "Square", "-c", "RED", "-z", "30", "-d", &domain, "-x", "2",
            ],
        );
        let until = Instant::now() + Duration::from_secs(15);
        let got = loop {
            assert!(
                Instant::now() < until,
                "the JSON client did not get a shape of Connext"
            );
            let Some(frame) = json.next(Duration::from_millis(200)) else {
                continue;
            };
            if frame.opcode != Opcode::Text {
                continue;
            }
            let Ok(v) = serde_json::from_slice::<serde_json::Value>(&frame.payload) else {
                continue;
            };
            if v.get("topic").and_then(|t| t.as_str()) == Some("Square")
                && v["data"]["color"] == "RED"
            {
                break v["data"].clone();
            }
        };
        assert_eq!(got["shapesize"], 30, "{got}");
        assert!(got["x"].is_i64() && got["y"].is_i64(), "{got}");
        assert_eq!(
            got["additional_payload_size"],
            serde_json::json!([]),
            "{got}"
        );
        // The binary client: a ZDB1 frame of an `@appendable` XCDR2 body.
        let until = Instant::now() + Duration::from_secs(10);
        let zdb1 = loop {
            assert!(
                Instant::now() < until,
                "the binary client did not get a shape of Connext"
            );
            let Some(frame) = binary.next(Duration::from_millis(200)) else {
                continue;
            };
            if frame.opcode == Opcode::Binary
                && frame.payload.starts_with(b"ZDB1")
                && frame.payload.windows(4).any(|w| w == b"RED\0")
            {
                break frame.payload;
            }
        };
        assert_eq!(zdb1[4], 0, "an alive sample has no flag");
        // D_CDR2_BE or D_CDR2_LE (DDS-XTypes 1.3 Table 60).
        assert!(
            zdb1[5] == 0 && (zdb1[6] == 0x08 || zdb1[6] == 0x09),
            "{:02x?}",
            &zdb1[..9]
        );
        // DHEADER, the length of "RED\0", its four octets, then x and y.
        let int_at = |at: usize| {
            let b = [zdb1[at], zdb1[at + 1], zdb1[at + 2], zdb1[at + 3]];
            i64::from(if zdb1[6] == 0x09 {
                i32::from_le_bytes(b)
            } else {
                i32::from_be_bytes(b)
            })
        };
        let (x, y) = (int_at(9 + 12), int_at(9 + 16));
        assert_eq!(int_at(9 + 20), 30);
        let _ = publisher.kill();
        let _ = publisher.wait();

        // The binary frame back into DDS: the Connext reader takes that shape.
        std::thread::sleep(Duration::from_millis(500));
        while lines.try_recv().is_ok() {}
        binary.send(Frame::binary(zdb1));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let line = lines
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|_| panic!("the binary frame did not reach Connext"));
            if shape_line(&line) == Some(("RED".to_string(), x, y, 30)) {
                break;
            }
        }
        let _ = sub.kill();
        let _ = sub.wait();
        handle.shutdown();
    }
}
