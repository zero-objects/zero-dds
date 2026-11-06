//! ZeroDDS' SPDP against Fast DDS on this host (DDSI-RTPS 2.5 §8.5.3,
//! coverage item CV-2).
//!
//! **Opt-in** — marked `#[ignore]`, and without `INTEROP_REQUIRE=1` (or
//! `INTEROP_HOST_AVAILABLE=1`) it reports a skip instead of running. The
//! interop pillar's job `vendor-live-dcps`
//! (`ci/interop-omg-weekly.yml`) sets it, in the interop image with Fast DDS
//! under `/opt/fastdds`:
//!
//! ```bash
//! INTEROP_REQUIRE=1 FASTDDS_SHAPES=<tests/interop/ci/fastdds build>/fastdds_shapes \
//!     cargo test -p zerodds-discovery --test fastdds_live_spdp -- --ignored --nocapture
//! ```
//!
//! A Fast DDS participant with the default (simple) discovery, the shapes
//! client of `tests/interop/ci/fastdds` publishing on a leased domain
//! (`zerodds-test-domains`): its SPDP beacon on the domain's multicast port
//! parses with ZeroDDS' SPDP reader and names eProsima's vendor id, the
//! domain and the participant's builtin endpoints.
//!
//! Before, the test started Fast DDS over SSH on a lab host (`fastdds
//! shape`, which no Fast DDS release has) and passed on any foreign beacon.
//! The second test of the file then, an SPDP handshake with a Fast DDS
//! discovery server, went: the server (`fastdds discovery`) answers a Fast
//! DDS client on the same host and not ZeroDDS' `DATA(p)`, also with the
//! client properties `PARTICIPANT_TYPE` and `DS_VERSION`; ZeroDDS is no
//! client of a discovery server (internal/release/OPEN-ITEMS.md).
//!
//! Unix only: the vendor program runs in a process group of its own,
//! which `std` offers there (`CommandExt::process_group`); Fast DDS runs
//! on the Linux job host.

#![cfg(unix)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use core::time::Duration;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use zerodds_discovery::spdp::{DiscoveredParticipant, SpdpReader};
use zerodds_rtps::participant_data::endpoint_flag;
use zerodds_rtps::wire_types::{GuidPrefix, VendorId};

const SPDP_MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 0, 1);
const TEST_TIMEOUT: Duration = Duration::from_secs(15);
const LOCAL_PREFIX: [u8; 12] = [0xCC; 12];

fn required(label: &str) -> bool {
    if std::env::var("INTEROP_REQUIRE").as_deref() == Ok("1")
        || std::env::var("INTEROP_HOST_AVAILABLE").as_deref() == Ok("1")
    {
        return true;
    }
    eprintln!("INTEROP-SKIP[{label}]: opt in with INTEROP_REQUIRE=1 and Fast DDS on this host");
    false
}

/// RTPS 2.5 §9.6.1.4.1: the SPDP multicast port of `domain`.
fn spdp_multicast_port(domain: i32) -> u16 {
    u16::try_from(7400 + 250 * domain).expect("SPDP port of a leased domain")
}

/// A vendor program of this host in a process group of its own, which is
/// killed when the test ends: `fastdds discovery` starts the server as a
/// child of its own. What the program prints goes to a log file.
struct Vendor {
    child: Child,
    log: PathBuf,
}

impl Vendor {
    fn start(program: PathBuf, args: &[String]) -> Self {
        use std::os::unix::process::CommandExt;
        let log = std::env::temp_dir().join(format!(
            "fastdds_live_spdp-{}-{}.log",
            std::process::id(),
            args.first().map_or("vendor", String::as_str)
        ));
        let file = std::fs::File::create(&log).expect("vendor log");
        let child = Command::new(&program)
            .args(args)
            .stdout(Stdio::from(file.try_clone().expect("vendor log")))
            .stderr(Stdio::from(file))
            .process_group(0)
            .spawn()
            .unwrap_or_else(|e| panic!("start {}: {e}", program.display()));
        Self { child, log }
    }

    fn output(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }
}

impl Drop for Vendor {
    fn drop(&mut self) {
        let group = format!("-{}", self.child.id());
        let _ = Command::new("kill")
            .args(["-KILL", "--", &group])
            .stderr(Stdio::null())
            .status();
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.log);
    }
}

/// The address a packet to another network would leave from.
fn detect_local_interface() -> Ipv4Addr {
    let probe = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|s| {
            s.connect(SocketAddrV4::new(Ipv4Addr::new(10, 255, 255, 255), 9))
                .map(|()| s)
        })
        .and_then(|s| s.local_addr());
    match probe {
        Ok(std::net::SocketAddr::V4(addr)) => *addr.ip(),
        _ => Ipv4Addr::UNSPECIFIED,
    }
}

/// The first SPDP `DATA(p)` of Fast DDS that `socket` receives within the
/// timeout, or what arrived instead: the number of datagrams and, per RTPS
/// message, its vendor and the submessage ids in it.
fn fastdds_participant(
    socket: &UdpSocket,
    ours: GuidPrefix,
) -> Result<DiscoveredParticipant, String> {
    let reader = SpdpReader::new();
    let mut buf = vec![0u8; 65_536];
    let mut seen = Vec::new();
    let start = Instant::now();
    while start.elapsed() < TEST_TIMEOUT {
        let Ok(n) = socket.recv(&mut buf) else {
            continue;
        };
        match reader.parse_datagram(&buf[..n]) {
            Ok(p) if p.sender_prefix != ours && p.sender_vendor == VendorId::FASTDDS => {
                return Ok(p);
            }
            _ => seen.push(describe(&buf[..n])),
        }
    }
    Err(format!(
        "{} other datagram(s): {}",
        seen.len(),
        seen.join("; ")
    ))
}

/// An RTPS message's vendor and submessage ids (RTPS 2.5 §9.4.4, §9.4.5).
fn describe(datagram: &[u8]) -> String {
    if datagram.len() < 20 || &datagram[..4] != b"RTPS" {
        return format!("{} octets, no RTPS header", datagram.len());
    }
    let mut ids = Vec::new();
    let mut at = 20;
    while at + 4 <= datagram.len() {
        let id = datagram[at];
        let little = datagram[at + 1] & 1 == 1;
        let len = if little {
            u16::from_le_bytes([datagram[at + 2], datagram[at + 3]])
        } else {
            u16::from_be_bytes([datagram[at + 2], datagram[at + 3]])
        } as usize;
        ids.push(format!("{id:#04x}"));
        if len == 0 {
            break;
        }
        at += 4 + len;
    }
    format!(
        "vendor {:02x}{:02x} [{}]",
        datagram[6],
        datagram[7],
        ids.join(" ")
    )
}

fn assert_simple_participant(p: &DiscoveredParticipant, domain: i32) {
    eprintln!(
        "Fast DDS participant {:?}, builtin endpoints {:#x}, domain {:?}",
        p.sender_prefix, p.data.builtin_endpoint_set, p.data.domain_id
    );
    // §8.5.3.2: an SPDP participant announces and detects participants, and
    // its SEDP endpoints for publications and subscriptions.
    for (flag, name) in [
        (
            endpoint_flag::PARTICIPANT_ANNOUNCER,
            "participant announcer",
        ),
        (endpoint_flag::PARTICIPANT_DETECTOR, "participant detector"),
        (
            endpoint_flag::PUBLICATIONS_ANNOUNCER,
            "publications announcer",
        ),
        (
            endpoint_flag::SUBSCRIPTIONS_DETECTOR,
            "subscriptions detector",
        ),
    ] {
        assert_ne!(p.data.builtin_endpoint_set & flag, 0, "{name} missing");
    }
    assert!(
        !p.data.metatraffic_unicast_locators.is_empty(),
        "no metatraffic unicast locator"
    );
    if let Some(announced) = p.data.domain_id {
        assert_eq!(i64::from(announced), i64::from(domain), "PID_DOMAIN_ID");
    }
}

#[test]
#[ignore = "needs Fast DDS on this host (FASTDDS_SHAPES); vendor-live-dcps runs it"]
fn fastdds_default_discovery_via_shape_pub_visible() {
    if !required("fastdds_live_spdp") {
        return;
    }
    let shapes = std::env::var_os("FASTDDS_SHAPES")
        .map(PathBuf::from)
        .expect("FASTDDS_SHAPES names the shapes client of tests/interop/ci/fastdds");
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();
    let interface = detect_local_interface();
    assert!(!interface.is_unspecified(), "no local IPv4 address");

    // The domain's SPDP multicast port, shared with Fast DDS on this host.
    let spdp_sock: UdpSocket = {
        let s = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )
        .expect("spdp socket");
        s.set_reuse_address(true).expect("SO_REUSEADDR");
        s.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, spdp_multicast_port(domain)).into())
            .expect("bind the SPDP multicast port");
        s.into()
    };
    spdp_sock
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    spdp_sock
        .join_multicast_v4(&SPDP_MULTICAST_GROUP, &interface)
        .expect("join the SPDP multicast group");

    let publisher = Vendor::start(
        shapes,
        &[
            "pub".into(),
            "Square".into(),
            domain.to_string(),
            "reliable".into(),
            "volatile".into(),
        ],
    );
    let participant = fastdds_participant(&spdp_sock, GuidPrefix::from_bytes(LOCAL_PREFIX))
        .unwrap_or_else(|seen| {
            panic!(
                "no SPDP beacon of Fast DDS on domain {domain} within {TEST_TIMEOUT:?}; {seen}; \
                 the publisher printed:\n{}",
                publisher.output()
            )
        });
    assert_simple_participant(&participant, domain);
}
