"""Bounded two-process proof of a freshly generated descriptor/native topic.

Usage: PYTHONPATH=<binding>:<wire-runtime>:<fresh-generated> python3
  crates/py/tests/generated_descriptor_live.py DOMAIN Point
The generated module is tools/idlc/tests/python-support/support.idl.
"""
import pathlib
from dataclasses import replace
import subprocess
import sys
import tempfile
import time

import support
import zerodds


def wait_file(path):
    deadline = time.monotonic() + 15
    while not path.exists():
        if time.monotonic() >= deadline:
            raise AssertionError(f"missing peer acknowledgement: {path.name}")
        time.sleep(0.02)


def endpoint(domain, reader, type_name):
    p = zerodds.DomainParticipantFactory.instance().create_participant_fast(domain)
    descriptor = support.ZD_TOPIC_DESCRIPTORS[f"bridge::{type_name}"]
    if type_name == "NestedKey":
        assert descriptor.type_dependencies, "dependent metadata was not generated"
        count = p.topics_len()
        try:
            zerodds.IdlTopic(p, "MissingDependency", replace(descriptor, _type_dependencies=()))
        except ValueError:
            pass
        else:
            raise AssertionError("missing dependency silently registered")
        assert p.topics_len() == count
    topic = zerodds.IdlTopic(p, f"RC7Generated{type_name}", descriptor)
    assert topic.type_name == f"bridge::{type_name}"
    entity = topic.create_reader(p.create_subscriber()) if reader else topic.create_writer(p.create_publisher())
    return p, descriptor, entity


def receive(domain, directory, type_name):
    p, descriptor, reader = endpoint(domain, True, type_name)
    directory.joinpath("ready").touch()
    reader.wait_for_matched_publication(1, 10)
    expected = [(True, "Alive", "alive"), (False, "NotAliveDisposed", "disposed")]
    handle = None
    for valid, state, acknowledgement in expected:
        deadline = time.monotonic() + 15
        while True:
            try:
                reader.wait_for_data(0.5)
            except TimeoutError:
                pass
            samples = reader.take_with_info()
            if samples:
                assert len(samples) == 1, samples
                value, info = samples[0]
                assert isinstance(value, descriptor.cls)
                assert (value.parts.id if type_name == "NestedKey" else value.id) == 177
                if type_name == "Point":
                    assert value.reading == (1234 if valid else 0)
                elif type_name == "Extended":
                    assert value.calibration == (1.25 if valid else None)
                else:
                    assert value.parts.other == (23 if valid else 0)
                    assert value.value == (456 if valid else 0)
                assert info.valid_data is valid and info.instance_state == state
                assert info.instance_handle != 0
                if handle is not None:
                    assert info.instance_handle == handle
                handle = info.instance_handle
                directory.joinpath(acknowledgement).touch()
                break
            assert time.monotonic() < deadline, (valid, state)
    print("reader: real generated class, value, keyed handle and dispose holder passed", flush=True)


def main(domain, type_name):
    with tempfile.TemporaryDirectory(prefix="zerodds-py-descriptor-") as directory:
        directory = pathlib.Path(directory)
        peer = subprocess.Popen([sys.executable, __file__, str(domain), type_name, "reader", str(directory)])
        try:
            wait_file(directory / "ready")
            p, descriptor, writer = endpoint(domain, False, type_name)
            writer.wait_for_matched_subscription(1, 10)
            if type_name == "NestedKey":
                value = descriptor.cls(parts=support.bridge.KeyParts(id=177, other=23), value=456)
            else:
                value = descriptor.cls(id=177, **({"reading": 1234} if type_name == "Point" else {"calibration": 1.25}))
            assert writer.register_instance(value) != 0
            writer.write(value)
            wait_file(directory / "alive")
            writer.dispose(value)
            wait_file(directory / "disposed")
            assert peer.wait(timeout=5) == 0
            print(f"PASS native generated Python DDS type={type_name} representation={descriptor.representation}")
        finally:
            if peer.poll() is None:
                peer.terminate()
                peer.wait(timeout=5)


if __name__ == "__main__":
    if len(sys.argv) > 3:
        receive(int(sys.argv[1]), pathlib.Path(sys.argv[4]), sys.argv[2])
    else:
        main(int(sys.argv[1]), sys.argv[2])
