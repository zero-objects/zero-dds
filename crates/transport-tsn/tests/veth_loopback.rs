// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Real RTPS-over-Ethernet roundtrip between two [`TsnTransport`]
//! instances over a veth pair in the root namespace (DDS-TSN 1.0
//! Annex A, AF_PACKET, EtherType 0x88B5).
//!
//! In contrast to the full-stack DCPS script `tests/interop/tsn_netns_e2e.sh`
//! this targets the transport layer specifically: two raw AF_PACKET
//! sockets, each bound to one veth end, one VLAN-tagged frame
//! out and one back — no discovery, no DDS stack.
//!
//! veth in the root NS instead of an isolated netns, because the latter is
//! blocked in LXC (e.g. codepit) (`ip netns add` = EPERM). The veth pair
//! proves the AF_PACKET path over a real L2 link nonetheless.
//!
//! Needs root (CAP_NET_RAW + CAP_NET_ADMIN), hence `#[ignore]`: skipped on
//! normal `cargo test`, run in CI via `--ignored` as
//! root. Runs only with `--features live` on Linux.

#![cfg(all(feature = "live", target_os = "linux"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::process::Command;
use std::time::Duration;

use zerodds_rtps::wire_types::Locator;
use zerodds_transport::Transport;
use zerodds_transport_tsn::TsnTransport;

const VETH_A: &str = "zdtsnvA";
const VETH_B: &str = "zdtsnvB";
const VLAN_VID: u16 = 42;
const PCP: u8 = 6;

fn run_ip(args: &[&str]) -> bool {
    Command::new("ip")
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn teardown() {
    // Deletes the pair (one end suffices — the peer goes with it).
    let _ = run_ip(&["link", "del", VETH_A]);
}

/// RAII guard: cleans up the veth pair even if the test panics
/// (otherwise the interface leaks into the next run).
struct VethGuard;

impl Drop for VethGuard {
    fn drop(&mut self) {
        teardown();
    }
}

/// Creates a fresh veth pair and brings both ends up. Panics
/// with a clear diagnostic if it fails (typically: not root) —
/// an `--ignored` run must never silently pass. The
/// returned guard cleans up on drop.
#[must_use]
fn setup() -> VethGuard {
    teardown();
    assert!(
        run_ip(&[
            "link", "add", VETH_A, "type", "veth", "peer", "name", VETH_B
        ]),
        "creating veth pair failed — needs root (CAP_NET_ADMIN)"
    );
    assert!(run_ip(&["link", "set", VETH_A, "up"]), "{VETH_A} up");
    assert!(run_ip(&["link", "set", VETH_B, "up"]), "{VETH_B} up");
    VethGuard
}

/// The live transport pads every frame to the Ethernet minimum size
/// (60 bytes incl. header); shorter payloads therefore arrive with appended
/// null bytes — RTPS tolerates this as a PAD submessage. So we check
/// the payload prefix + null padding instead of the exact length.
fn assert_rtps_prefix(got: &[u8], expected: &[u8]) {
    assert!(
        got.len() >= expected.len(),
        "frame too short: {} < {}",
        got.len(),
        expected.len()
    );
    assert_eq!(
        &got[..expected.len()],
        expected,
        "payload prefix must arrive byte-identical"
    );
    assert!(
        got[expected.len()..].iter().all(|&b| b == 0),
        "frame padding must be null"
    );
}

#[test]
#[ignore = "needs root (CAP_NET_RAW + CAP_NET_ADMIN); CI runs this via --ignored"]
fn rtps_frame_roundtrips_over_veth() {
    let _veth = setup(); // The guard cleans up the pair on drop.
    let timeout = Some(Duration::from_secs(2));

    // Both endpoints are configured VLAN-tagged (PCP=6, VID=42), so the
    // send-path frames carry the 802.1Q tag (proven by the `live_frame` unit
    // tests). Note: on a pure software link (veth) the kernel
    // removes the VLAN tag on RX without reporting it via
    // `PACKET_AUXDATA` (`tp_vlan_tci=0`, no `VLAN_VALID`) —
    // so the received VID is not reconstructible there. On real
    // NICs with HW VLAN offload the kernel provides it (see `socket::recv`).
    // This test checks the hardware-independently guaranteed
    // properties: bidirectional payload roundtrip + correct
    // source MAC over a real L2 link.
    let a = TsnTransport::bind(VETH_A, VLAN_VID, PCP, timeout).expect("bind A (CAP_NET_RAW?)");
    let b = TsnTransport::bind(VETH_B, VLAN_VID, PCP, timeout).expect("bind B (CAP_NET_RAW?)");

    let dest_b = Locator::tsn(b.local_mac(), VLAN_VID);

    // Plausible RTPS message (header `RTPS` + version 2.5 + vendor +
    // GUID prefix); the transport is payload-agnostic.
    let payload: &[u8] = b"RTPS\x02\x05\x01\x0f\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01";

    a.send(&dest_b, payload).expect("send A->B");

    let got = b.recv().expect("recv on B (timeout?)");
    assert_rtps_prefix(&got.data, payload);
    assert_eq!(
        got.source.address[..6],
        a.local_mac(),
        "source MAC must be A"
    );

    // Return path B->A, so both directions are proven.
    let dest_a = Locator::tsn(a.local_mac(), VLAN_VID);
    let reply: &[u8] = b"RTPS\x02\x05PONGPONGPONGPONG";
    b.send(&dest_a, reply).expect("send B->A");
    let back = a.recv().expect("recv on A");
    assert_rtps_prefix(&back.data, reply);
    assert_eq!(
        back.source.address[..6],
        b.local_mac(),
        "source MAC must be B"
    );

    // veth cleanup is handled by the VethGuard on drop.
}
