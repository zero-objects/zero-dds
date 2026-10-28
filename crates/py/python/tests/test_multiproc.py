"""Tests for §5.2 — multi-process Python ↔ Python.

Two `python -m` subprocesses exchange bytes over RTPS loopback.
The test needs `_core` (the PyO3 module) and a Linux/macOS loopback
multicast configuration. On unsupported platforms it is
skipped.
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
    reason="zerodds._core not compiled — maturin develop needed",
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
    # A writer that goes away discards what is not acknowledged (DDS 1.4
    # §2.2.2.4.1.12). This writer may know the reader before the reader
    # knows it: the reader's SEDP repair comes heartbeatResponseDelay plus
    # nackResponseDelay (500 + 200 ms, DDSI-RTPS 2.5 §8.4.10.1, §8.4.7.1)
    # after a HEARTBEAT, and a sample the reader dropped as coming from an
    # unknown writer is repaired after that. The process waits for the
    # reader's acknowledgment rather than ending first.
    writer.wait_for_acknowledgments(10.0)
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
    # RELIABLE. The DataReader default is BEST_EFFORT (DDS 1.4 §2.2.3):
    # a sample written before this reader knows the writer is then lost, and
    # the writer's wait_for_acknowledgments has no reader to wait for.
    qos = zerodds.DataReaderQos()
    qos.set_reliability("Reliable", 0.1)
    reader = p.create_subscriber().create_bytes_reader_with_qos(topic, qos)
    reader.wait_for_matched_publication(1, 5.0)
    deadline = 5.0
    while deadline > 0:
        try:
            reader.wait_for_data(0.5)
        except TimeoutError:
            pass
        samples = reader.take()
        if expected in samples:
            sys.stdout.write("OK")
            sys.exit(0)
        deadline -= 0.5
    sys.exit(2)
""")


@pytest.mark.skipif(
    sys.platform == "win32",
    reason="multi-proc loopback multicast needs a Linux/macOS setup",
)
def test_multiproc_python_python_roundtrip(tmp_path: object) -> None:
    """§5.2 — two subprocess Pythons, one BytesTopic, one payload."""
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
        pytest.fail("multi-proc roundtrip timeout")

    assert subscriber.returncode == 0, (
        f"subscriber returncode={subscriber.returncode}, "
        f"stdout={sub_out!r}, stderr={sub_err!r}"
    )
    assert b"OK" in sub_out
