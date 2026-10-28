// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Live OPC-UA interop against the open62541 reference client (item 7):
//! Browse, Read, Write, driven from a tiny C client built against
//! `libopen62541`'s own public API — see `tests/foreign/README.md` for
//! prerequisites and `tests/foreign/open62541_client.c` for what it does.
//!
//! Gated on `OPCUA_OPEN62541=1` + `pkg-config`/`cc` finding open62541;
//! `#[ignore]` by default, matching this workspace's established pattern
//! for a foreign-vendor reference client
//! (`crates/coap-bridge/tests/libcoap_interop_e2e.rs`,
//! `crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs`). Not run by the
//! agent that wrote it — no open62541 install on this host, and this
//! repo's own convention for foreign-vendor interop is Linux, not macOS
//! (`tests/interop/shapes_cyclone_interop.sh`'s own header comment).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_opcua_bridge::daemon::config::DaemonConfig;
use zerodds_opcua_bridge::daemon::server;
use zerodds_opcua_bridge::dds_bridge::DdsBridge;

const IDL: &str = "\
@final struct Telemetry { double value; };
@final struct Command { long value; };
";

const GATEWAY_XML: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<zerodds_opcua_gateway>
  <bridge name=\"open62541\">
    <domain_id>0</domain_id>
    <dds_to_ua_connection>
      <dds_topic>Telemetry</dds_topic>
      <dds_type>Telemetry</dds_type>
    </dds_to_ua_connection>
    <ua_to_dds_connection>
      <dds_topic>Commands</dds_topic>
      <dds_type>Command</dds_type>
    </ua_to_dds_connection>
  </bridge>
</zerodds_opcua_gateway>
";

fn enabled() -> bool {
    std::env::var("OPCUA_OPEN62541").is_ok()
}

fn write_fixture(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(std::format!(
        "zd-opcua-bridge-open62541-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

/// `pkg-config --cflags/--libs open62541`, or `OPEN62541_CFLAGS`/
/// `OPEN62541_LIBS` if set — `None` if neither resolves (the C client
/// cannot be built at all).
fn open62541_flags() -> Option<(Vec<String>, Vec<String>)> {
    if let (Ok(cflags), Ok(libs)) = (
        std::env::var("OPEN62541_CFLAGS"),
        std::env::var("OPEN62541_LIBS"),
    ) {
        return Some((
            cflags.split_whitespace().map(str::to_string).collect(),
            libs.split_whitespace().map(str::to_string).collect(),
        ));
    }
    let cflags = Command::new("pkg-config")
        .args(["--cflags", "open62541"])
        .output()
        .ok()?;
    let libs = Command::new("pkg-config")
        .args(["--libs", "open62541"])
        .output()
        .ok()?;
    if !cflags.status.success() || !libs.status.success() {
        return None;
    }
    let cflags = String::from_utf8_lossy(&cflags.stdout)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let libs = String::from_utf8_lossy(&libs.stdout)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    Some((cflags, libs))
}

/// Compiles `tests/foreign/open62541_client.c` against the discovered
/// open62541 flags, returning the built binary's path.
fn compile_client(cflags: &[String], libs: &[String]) -> Result<PathBuf, String> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/foreign/open62541_client.c");
    let out = std::env::temp_dir().join(std::format!("zd-open62541-client-{}", std::process::id()));
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let status = Command::new(&cc)
        .arg(&src)
        .args(cflags)
        .args(libs)
        .arg("-o")
        .arg(&out)
        .status()
        .map_err(|e| std::format!("spawn {cc}: {e}"))?;
    if !status.success() {
        return Err(std::format!("{cc} failed compiling {}", src.display()));
    }
    Ok(out)
}

#[test]
#[ignore = "needs open62541 (pkg-config-visible) + a C compiler; set OPCUA_OPEN62541=1 — see tests/foreign/README.md"]
fn open62541_browses_reads_and_writes_through_the_daemon() {
    if !enabled() {
        eprintln!("[skip] OPCUA_OPEN62541 not set — see tests/foreign/README.md");
        return;
    }
    let Some((cflags, libs)) = open62541_flags() else {
        panic!(
            "OPCUA_OPEN62541 is set but open62541 was not found via pkg-config \
             (and OPEN62541_CFLAGS/OPEN62541_LIBS are not set) — see \
             tests/foreign/README.md. Skipping here would report interop \
             that never happened."
        );
    };
    let client_bin = compile_client(&cflags, &libs).expect("compile the open62541 C client");

    let idl = write_fixture("open62541.idl", IDL);
    let gateway = write_fixture("gateway_open62541.xml", GATEWAY_XML);
    let lease = zerodds_test_domains::lease_domain();
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.domain = lease.id();
    cfg.idl = vec![idl.clone()];
    cfg.gateway_config = Some(gateway);
    cfg.server.bind = "127.0.0.1:0".to_string();

    let mut handle =
        server::start_with_reload(cfg, RuntimeConfig::default(), None).expect("start daemon");

    // Feed a real DDS sample so READ_OK reports a real value, not the
    // Table-9 default.
    let idl_ast = zerodds_opcua_bridge::idl::compile(&idl, &[]).expect("compile idl");
    let type_support = zerodds_opcua_bridge::idl::type_support_for(&[idl_ast.clone()], "Telemetry")
        .expect("type support");
    let mut writer_bridge =
        DdsBridge::start(lease.id(), "open62541-feed", RuntimeConfig::default())
            .expect("writer bridge");
    writer_bridge
        .writer(
            "Telemetry",
            "Telemetry",
            false,
            false,
            Some(type_support.clone()),
        )
        .expect("writer");
    let data = zerodds_types::dynamic::DynamicData::Aggregate(vec![(
        "value".to_string(),
        zerodds_types::dynamic::DynamicData::Float {
            kind: Some(zerodds_ast::PrimitiveKind::Float64),
            value: 21.5,
        },
    )]);
    let sample = type_support.from_dynamic_data(data).expect("build sample");
    let body = type_support.write_body(&sample, 2, false).expect("encode");
    std::thread::sleep(Duration::from_millis(300));
    writer_bridge.write("Telemetry", &body).expect("dds write");
    std::thread::sleep(Duration::from_millis(300)); // let the pump apply it

    // An independent DDS reader on Commands, to confirm the C client's
    // write actually reached DDS (not just that WRITE_OK printed).
    let cmd_type_support =
        zerodds_opcua_bridge::idl::type_support_for(&[idl_ast.clone()], "Command")
            .unwrap_or_else(|_| type_support.clone());
    let mut reader_bridge =
        DdsBridge::start(lease.id(), "open62541-verify", RuntimeConfig::default())
            .expect("reader bridge");
    let reader = reader_bridge
        .reader("Commands", "Command", false, false, Some(cmd_type_support))
        .expect("reader");

    let endpoint = std::format!("opc.tcp://{}", handle.local_addr);
    let out = Command::new(&client_bin)
        .args([
            &endpoint,
            "Topic:Telemetry/Value/value",
            "Topic:Commands/Value/value",
            "777",
        ])
        .output()
        .expect("run the open62541 client");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!("open62541 client stdout:\n{stdout}");
    if !out.status.success() {
        eprintln!(
            "open62541 client stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    assert!(
        stdout.contains("CONNECT_OK"),
        "the client must connect: {stdout}"
    );
    assert!(
        stdout.contains("BROWSE_OK"),
        "Browse of the Objects folder must succeed: {stdout}"
    );
    assert!(
        stdout.contains("READ_OK value=21.5"),
        "Read of Telemetry.value must return the fed sample: {stdout}"
    );
    assert!(
        stdout.contains("WRITE_OK value=777"),
        "Write of Commands.value must succeed and read back: {stdout}"
    );

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut got = Vec::new();
    while got.is_empty() && std::time::Instant::now() < deadline {
        got = reader.poll();
        if got.is_empty() {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    assert_eq!(
        got.len(),
        1,
        "the open62541 client's Write must reach DDS, not just the AddressSpace"
    );

    handle.shutdown();
    eprintln!("OK: open62541 Browse/Read/Write through zerodds-opcua-bridged");
}
