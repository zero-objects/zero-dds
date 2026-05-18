"""Tests fuer §5.2 — Multi-Process Python ↔ Python.

Zwei `python -m` Subprocesses tauschen Bytes ueber RTPS-Loopback aus.
Der Test braucht `_core` (PyO3-Modul) und eine Linux/macOS-Loopback-
Multicast-Konfiguration. Auf nicht-supportenden Plattformen wird
geskipped.
"""

from __future__ import annotations

import os
import subprocess
import sys
import textwrap

import pytest

import zerodds

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core nicht kompiliert — maturin develop noetig",
)


_PUBLISHER_SCRIPT = textwrap.dedent("""
    import os, time
    import zerodds
    domain = int(os.environ["TEST_DOMAIN"])
    topic_name = os.environ["TEST_TOPIC"]
    payload = os.environ["TEST_PAYLOAD"].encode("utf-8")
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_fast(domain)
    topic = p.create_bytes_topic(topic_name)
    writer = p.create_publisher().create_bytes_writer(topic)
    writer.wait_for_matched_subscription(1, 5.0)
    for _ in range(5):
        writer.write(payload)
        time.sleep(0.05)
""")


_SUBSCRIBER_SCRIPT = textwrap.dedent("""
    import os, sys
    import zerodds
    domain = int(os.environ["TEST_DOMAIN"])
    topic_name = os.environ["TEST_TOPIC"]
    expected = os.environ["TEST_PAYLOAD"].encode("utf-8")
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_fast(domain)
    topic = p.create_bytes_topic(topic_name)
    reader = p.create_subscriber().create_bytes_reader(topic)
    reader.wait_for_matched_publication(1, 5.0)
    deadline = 5.0
    while deadline > 0:
        reader.wait_for_data(0.5)
        samples = reader.take()
        if expected in samples:
            sys.stdout.write("OK")
            sys.exit(0)
        deadline -= 0.5
    sys.exit(2)
""")


@pytest.mark.skipif(
    sys.platform == "win32",
    reason="Multi-Proc-Loopback-Multicast braucht ein Linux/macOS-Setup",
)
def test_multiproc_python_python_roundtrip(tmp_path: object) -> None:
    """§5.2 — Zwei Subprocess-Python, ein BytesTopic, ein Payload."""
    env = {
        **os.environ,
        "TEST_DOMAIN": "210",
        "TEST_TOPIC": "MultiProcChatter",
        "TEST_PAYLOAD": "ping-from-subproc",
    }

    subscriber = subprocess.Popen(
        [sys.executable, "-c", _SUBSCRIBER_SCRIPT],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    publisher = subprocess.Popen(
        [sys.executable, "-c", _PUBLISHER_SCRIPT],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    try:
        sub_out, sub_err = subscriber.communicate(timeout=15)
        publisher.communicate(timeout=15)
    except subprocess.TimeoutExpired:
        subscriber.kill()
        publisher.kill()
        pytest.fail("Multi-Proc-Roundtrip-Timeout")

    assert subscriber.returncode == 0, (
        f"subscriber returncode={subscriber.returncode}, "
        f"stdout={sub_out!r}, stderr={sub_err!r}"
    )
    assert b"OK" in sub_out
