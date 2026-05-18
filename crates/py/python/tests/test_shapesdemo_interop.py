"""Tests fuer §5.4 — Cross-Vendor Python ↔ Cyclone-/Fast-DDS-ShapesDemo.

Verifiziert, dass ein lokaler Cyclone-`ddsperf -P` Publisher von einem
ZeroDDS-Python-Subscriber empfangen wird. Skip wenn `ddsperf` nicht im
PATH (Cyclone DDS muss installiert sein).
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
        reason="zerodds._core nicht kompiliert — maturin develop noetig",
    ),
    pytest.mark.skipif(
        DDS_PERF is None,
        reason="ddsperf nicht im PATH — Cyclone DDS installieren fuer Cross-Vendor-Test",
    ),
    pytest.mark.skipif(
        sys.platform == "win32",
        reason="Cross-Vendor-Test braucht Linux/macOS-Loopback-Multicast",
    ),
]


def _shape_type_name() -> str:
    """§5.4 — Cross-Vendor-Topic-Type-Name. Muss zu Cyclone ShapeType passen."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(90)
    topic = p.create_shape_topic("CrossVendorProbe")
    return topic.type_name


def test_shape_type_name_matches_shapesdemo_convention():
    """§5.4 — Topic-Type-Name auf der Python-Side ist `ShapeType`, kompatibel
    zu Cyclone-/Fast-DDS-ShapesDemo (kein Module-Prefix)."""
    assert _shape_type_name() == "ShapeType"


def test_cross_vendor_participant_discovery_against_ddsperf():
    """§5.4 — Cyclone `ddsperf -i <domain> pub 1Hz` startet einen
    DomainParticipant auf der gegebenen Domain. ZeroDDS spricht
    SPDP-Multicast auf der gleichen Domain und MUSS den Cyclone-
    Participant in der Discovered-Participants-Liste sehen.

    Das ist der minimale Cross-Vendor-Wire-Beweis: kompatible
    DDSI-RTPS-2.5-SPDP-Beacons. Topic-Matching gegen ddsperf-interne
    Topic-Namen (KeyedSeq-Pattern wechselt zwischen ddsperf-Versionen)
    waere fragil; volle byte-Tests laufen Rust-seitig in
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
            f"Cyclone ddsperf participant auf Domain {domain} nicht via SPDP "
            f"discovered (discovered_participants_count={discovered})"
        )
    finally:
        publisher.terminate()
        try:
            publisher.wait(timeout=5)
        except subprocess.TimeoutExpired:
            publisher.kill()
        time.sleep(0.1)
