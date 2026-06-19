"""Tests for §5.4 — cross-vendor Python ↔ Cyclone/Fast-DDS ShapesDemo.

Verifies that a local Cyclone `ddsperf -P` publisher is received by a
ZeroDDS Python subscriber. Skipped if `ddsperf` is not on the
PATH (Cyclone DDS must be installed).
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import time

import pytest

import zerodds

DDS_PERF = shutil.which("ddsperf")

pytestmark = [
    pytest.mark.skipif(
        not getattr(zerodds, "_CORE_AVAILABLE", False),
        reason="zerodds._core not compiled — maturin develop needed",
    ),
    pytest.mark.skipif(
        DDS_PERF is None,
        reason="ddsperf not on PATH — install Cyclone DDS for the cross-vendor test",
    ),
    pytest.mark.skipif(
        sys.platform == "win32",
        reason="cross-vendor test needs Linux/macOS loopback multicast",
    ),
]


def _shape_type_name() -> str:
    """§5.4 — cross-vendor topic type name. Must match Cyclone's ShapeType."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(90)
    topic = p.create_shape_topic("CrossVendorProbe")
    return topic.type_name


def test_shape_type_name_matches_shapesdemo_convention():
    """§5.4 — the topic type name on the Python side is `ShapeType`, compatible
    with Cyclone/Fast-DDS ShapesDemo (no module prefix)."""
    assert _shape_type_name() == "ShapeType"


def test_cross_vendor_participant_discovery_against_ddsperf():
    """§5.4 — Cyclone `ddsperf -i <domain> pub 1Hz` starts a
    DomainParticipant on the given domain. ZeroDDS speaks
    SPDP multicast on the same domain and MUST see the Cyclone
    participant in the discovered-participants list.

    This is the minimal cross-vendor wire proof: compatible
    DDSI-RTPS-2.5 SPDP beacons. Topic matching against ddsperf-internal
    topic names (the KeyedSeq pattern changes between ddsperf versions)
    would be fragile; full byte tests run on the Rust side in
    `crates/discovery/tests/cyclone_live_*.rs`."""
    domain = 91
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_fast(domain)

    env = {**os.environ, "CYCLONEDDS_URI": ""}
    publisher = subprocess.Popen(
        [DDS_PERF, "-D", "20", "-i", str(domain), "pub", "1Hz"],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )

    try:
        # Wait up to 10s for SPDP to discover the Cyclone participant.
        deadline = time.time() + 10.0
        discovered = 0
        while time.time() < deadline:
            discovered = p.discovered_participants_count()
            if discovered >= 1:
                break
            time.sleep(0.2)
        assert discovered >= 1, (
            f"Cyclone ddsperf participant on domain {domain} not discovered via "
            f"SPDP (discovered_participants_count={discovered})"
        )
    finally:
        publisher.terminate()
        try:
            publisher.wait(timeout=5)
        except subprocess.TimeoutExpired:
            publisher.kill()
        time.sleep(0.1)
