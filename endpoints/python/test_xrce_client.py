#!/usr/bin/env python
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Tests for zerodds_xrce_client.py: the state-machine transitions
# crates/xrce-client/src/lib.rs's own unit tests check, plus the wire
# octets against endpoints/testdata/xrce-client/*.bin -- fixtures produced
# by endpoints/golden-gen actually running zerodds_xrce_client::XrceClient
# (crates/xrce-client) over zerodds_xrce (crates/xrce), the same two
# crates this module ports. All 20 language ports (plus
# endpoints/xrce-agent-demo) check the same five files; a hardcoded hex
# string here would be a second copy of the truth that can drift from it
# unnoticed. ClientKey = 01:02:03:04 throughout, session_id = 0x01 (<=127,
# so every header after CREATE_CLIENT carries the ClientKey; the
# CREATE_CLIENT header itself has session 0x00, see below).
#
#   create_client: CREATE_CLIENT, a real CLIENT_Representation
#     (version={1,0}, vendor_id={0,0}, this client_key, session_id=0x01,
#     no properties, requested_mtu=508); header = (session 0x00, stream
#     STREAM_ID_NONE, seq 0, key). The header session is
#     SESSION_ID_NONE_WITH_CLIENT_KEY (DDS-XRCE 1.0 Sec 8.3.2.1), not the
#     session_id 0x01 the payload asks for.
#   create: CREATE, request_id={0x00,0x01}, object_id=pack(raw=0x001,
#     kind=DATAWRITER), by_reference("SensorWriter") under
#     parent=pack(raw=0x001, kind=PUBLISHER) -- the real two-level
#     ObjectVariant (Sec 7.7.3), reuse=False, replace=False; header =
#     (session 0x01, stream STREAM_ID_BUILTIN_RELIABLE, seq 0, key).
#   delete: DELETE, request_id={0x00,0x02}, object_id=pack(raw=0x001,
#     kind=DATAWRITER) -- deletes the writer CREATEd above; header =
#     (session 0x01, stream STREAM_ID_BUILTIN_RELIABLE, seq 1, key).
#   read_data: READ_DATA, request_id={0x00,0x03}, object_id=pack(raw=0x002,
#     kind=DATAREADER), preferred_stream_id=0x01, data_format=Data, no
#     filter, no delivery control; header = (session 0x01, stream
#     STREAM_ID_BUILTIN_RELIABLE, seq 2, key).
#   write_data: WRITE_DATA, request_id={0x00,0x04}, object_id=pack(raw=
#     0x001, kind=DATAWRITER) -- the same writer -- sample=[0x19,0x00];
#     header = (session 0x01, stream STREAM_ID_BUILTIN_RELIABLE, seq 3,
#     key).
#
# These four post-handshake fixtures are read independently, each as one
# wire example of its own submessage kind -- not as one
# application-consistent object lifecycle: write_data targets the writer
# delete already removed, and nothing about either message's bytes
# depends on the other one having run first.
#
# usage: python test_xrce_client.py

from __future__ import print_function

import os
import sys

import zerodds_xrce_client as xc

KEY = bytearray([0x01, 0x02, 0x03, 0x04])

_TESTDATA_DIR = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "testdata", "xrce-client")


def load_fixture(name):
    with open(os.path.join(_TESTDATA_DIR, name + ".bin"), "rb") as f:
        return f.read()


def check(name, cond):
    if not cond:
        print("FAIL:", name)
        return 1
    print("ok:", name)
    return 0


# --- state-machine tests, mirroring crates/xrce-client's own unit tests ---

def test_state_machine():
    rc = 0
    c = xc.XrceClient(KEY)
    rc |= check("new client starts Disconnected", c.state == xc.DISCONNECTED)

    c.connect()
    rc |= check("connect -> Connecting", c.state == xc.CONNECTING)
    try:
        c.connect()
        rc |= check("double connect rejected", False)
    except xc.ClientError:
        rc |= check("double connect rejected", True)

    c.mark_connected()
    rc |= check("mark_connected -> Connected", c.state == xc.CONNECTED)

    c2 = xc.XrceClient(KEY)
    oid = xc.new_object_id(0x010, xc.OBJK_TOPIC)
    try:
        c2.create_object(oid, xc.by_reference("topic-ref"))
        rc |= check("create without connect rejected", False)
    except xc.ClientError:
        rc |= check("create without connect rejected", True)
    try:
        c2.request_write(oid, b"x")
        rc |= check("write without connect rejected", False)
    except xc.ClientError:
        rc |= check("write without connect rejected", True)
    try:
        c2.request_read(oid)
        rc |= check("read without connect rejected", False)
    except xc.ClientError:
        rc |= check("read without connect rejected", True)

    c3 = xc.XrceClient(KEY)
    c3.connect()
    c3.mark_connected()
    r1 = c3.create_object(oid, xc.by_reference("a"))
    r2 = c3.delete_object(oid)
    r3 = c3.request_write(oid, b"x")
    r4 = c3.request_read(oid)
    rc |= check("monotonic request ids 1,2,3,4", (r1, r2, r3, r4) == (1, 2, 3, 4))

    c3.disconnect()
    rc |= check("disconnect clears state", c3.state == xc.DISCONNECTED)
    return rc


# --- wire-octet tests -------------------------------------------------------

def test_wire_fixtures():
    rc = 0

    # CREATE_CLIENT -- a real CLIENT_Representation, not an ad hoc blob.
    rep = xc.encode_client_representation(
        version=xc.XRCE_VERSION, vendor_id=bytearray([0x00, 0x00]), client_key=KEY,
        session_id=0x01, requested_mtu=xc.DEFAULT_MTU)
    # Sec 8.3.2.1: the CREATE_CLIENT of a session_id below 128 goes in a header
    # with SESSION_ID_NONE_WITH_CLIENT_KEY (0x00); the payload asks for session 1.
    got = xc.build_create_client_message(0x00, xc.STREAM_ID_NONE, 0, KEY, rep)
    rc |= check("CREATE_CLIENT matches fixture", got == load_fixture("xrce_client_create_client"))

    # DDS-XRCE 1.0 Sec 7.8.2.1 prose vs Annex A / B.1: representation_timestamps.
    absent_b = bytes(bytearray([0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00,
                                0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01]))
    time_t = [0x8c, 0xaa, 0x88, 0x5a, 0x00, 0x65, 0xcd, 0x1d]
    appendable_b = bytes(bytearray([0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00]
                                   + time_t + [0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01]))
    final_b = bytes(bytearray([0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00]
                              + time_t + [0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01]))
    ver, ven = xc.XRCE_VERSION, bytearray([0x00, 0x00])
    rc |= check("representation default is the 16-octet absent form",
                xc.encode_client_representation(ver, ven, KEY, 0x01, 508) == absent_b)
    rc |= check("representation_timestamps absent",
                xc.encode_client_representation(ver, ven, KEY, 0x01, 508, "absent", 1518905996, 500000000) == absent_b)
    rc |= check("representation_timestamps present, appendable (28 octets)",
                xc.encode_client_representation(ver, ven, KEY, 0x01, 508, "present", 1518905996, 500000000) == appendable_b)
    rc |= check("representation_timestamps present, final (24 octets)",
                xc.encode_client_representation(ver, ven, KEY, 0x01, 508, "present", 1518905996, 500000000,
                                                unannotated_types="final") == final_b)
    rc |= check("client_representation present writes 28 octets",
                len(xc.XrceClient(KEY, representation_timestamps="present").client_representation()) == 28)

    # Sec 8.3.2.1: a requested session_id of 128..255 goes in a header with
    # SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80), which carries no client key.
    rep81 = xc.encode_client_representation(
        version=xc.XRCE_VERSION, vendor_id=bytearray([0x00, 0x00]), client_key=KEY,
        session_id=0x81, requested_mtu=xc.DEFAULT_MTU)
    got81 = xc.build_create_client_message(
        xc.SESSION_ID_NONE_WITHOUT_CLIENT_KEY, xc.STREAM_ID_NONE, 0, None, rep81)
    rc |= check("CREATE_CLIENT for session 0x81 has header session 0x80, no key",
                got81[0] == 0x80 and len(got81) == 4 + 4 + len(rep81) and got81[4 + 4 + 12] == 0x81)

    # CREATE/DELETE/READ_DATA/WRITE_DATA -- exercised through the real
    # client so its request-id counter is proven too, not just the
    # encoder. All four are reliable, so they share STREAM_ID_BUILTIN_
    # RELIABLE and a real, strictly increasing sequence number (0..3) --
    # not the best-effort stream with hand-picked numbers a previous
    # fixture set used.
    c2 = xc.XrceClient(KEY)
    c2.connect()
    c2.mark_connected()
    w_oid = xc.new_object_id(0x001, xc.OBJK_DATAWRITER)
    publisher = xc.new_object_id(0x001, xc.OBJK_PUBLISHER)
    variant = xc.by_reference(u"SensorWriter")

    req = c2.create_object(w_oid, variant)
    rc |= check("create request id == 1", req == 1)
    got = xc.build_create_message(
        0x01, xc.STREAM_ID_BUILTIN_RELIABLE, 0, KEY, req, w_oid, xc.OBJK_DATAWRITER, variant,
        parent_object_id=publisher)
    rc |= check("CREATE matches fixture", got == load_fixture("xrce_client_create"))

    req = c2.delete_object(w_oid)
    rc |= check("delete request id == 2", req == 2)
    got = xc.build_delete_message(0x01, xc.STREAM_ID_BUILTIN_RELIABLE, 1, KEY, req, w_oid)
    rc |= check("DELETE matches fixture", got == load_fixture("xrce_client_delete"))

    reader = xc.new_object_id(0x002, xc.OBJK_DATAREADER)
    spec = xc.ReadSpecification(preferred_stream_id=0x01, data_format=xc.DATA_FORMAT_DATA)
    req = c2.request_read(reader)
    rc |= check("read request id == 3", req == 3)
    got = xc.build_read_data_message(
        0x01, xc.STREAM_ID_BUILTIN_RELIABLE, 2, KEY, req, reader, spec)
    rc |= check("READ_DATA matches fixture", got == load_fixture("xrce_client_read_data"))

    req = c2.request_write(w_oid, b"x")
    rc |= check("write request id == 4", req == 4)
    got = xc.build_write_data_message(
        0x01, xc.STREAM_ID_BUILTIN_RELIABLE, 3, KEY, req, w_oid, bytearray([0x19, 0x00]))
    rc |= check("WRITE_DATA matches fixture", got == load_fixture("xrce_client_write_data"))

    # A second CREATE, appended to the same session: OBJK_TOPIC's own
    # representation carries its participant's ObjectId (commit
    # 31c62df2a63, `xrce: OBJK_TOPIC_Representation carries its
    # participant's ObjectId`), the same trailing field OBJK_PUBLISHER's
    # representation carries above -- xrce_client_create_topic.bin is a
    # sixth fixture generated the same way (endpoints/golden-gen), not a
    # hand-picked byte string.
    topic_oid = xc.new_object_id(0x010, xc.OBJK_TOPIC)
    participant = xc.new_object_id(0x001, xc.OBJK_PARTICIPANT)
    topic_variant = xc.by_reference(u"HelloWorldTopic")
    req = c2.create_object(topic_oid, topic_variant)
    rc |= check("create topic request id == 5", req == 5)
    got = xc.build_create_message(
        0x01, xc.STREAM_ID_BUILTIN_RELIABLE, 4, KEY, req, topic_oid, xc.OBJK_TOPIC,
        topic_variant, parent_object_id=participant)
    rc |= check("CREATE(topic) matches fixture", got == load_fixture("xrce_client_create_topic"))

    return rc


def test_topic_representation_matches_the_rust_byte_oracle():
    """crates/xrce/src/object_repr.rs's own
    `topic_representation_matches_the_captured_create_bytes` test decodes
    these exact 96 bytes (the real eProsima Micro-XRCE-DDS 3.0.1 capture,
    tools/wire-capture/xrce-sessions/client-to-agent.hex) as
    OBJK_TOPIC/ByXmlString/participant_id=ObjectId(1, PARTICIPANT). This
    port's `encode_for_kind` -- unlike `xrce_client_create_topic.bin`'s
    ByReference case above -- can also express ByXmlString, so it is
    checked against that same oracle directly."""
    rc = 0
    xml = (u"<dds><topic><name>HelloWorldTopic</name><dataType>HelloWorld</dataType>"
           u"</topic></dds>")
    expected = bytearray([0x02, 0x02, 0x00, 0x00, 0x56, 0x00, 0x00, 0x00])
    expected += bytearray(xml.encode("utf-8"))
    expected += bytearray([0x00, 0x00, 0x11])
    participant = xc.new_object_id(0x001, xc.OBJK_PARTICIPANT)
    got = xc.by_xml_string(xml).encode_for_kind(xc.OBJK_TOPIC, parent_object_id=participant)
    rc |= check("OBJK_TOPIC ByXmlString matches the Rust byte oracle", got == bytes(expected))
    return rc


def test_topic_create_without_parent_rejected():
    """OBJK_TOPIC now carries a parent id (has_parent), so a CREATE
    without one is refused the same way OBJK_PUBLISHER's already is --
    `encode_for_kind`'s own `has_parent`/`parent_object_id is None` guard,
    not a topic-specific check."""
    rc = 0
    try:
        xc.by_reference(u"HelloWorldTopic").encode_for_kind(xc.OBJK_TOPIC, parent_object_id=None)
        rc |= check("OBJK_TOPIC CREATE without a parent rejected", False)
    except ValueError:
        rc |= check("OBJK_TOPIC CREATE without a parent rejected", True)
    return rc


def test_object_id_packing():
    """Cross-check independent of the fixtures above (crates/xrce
    `new_packs_kind_into_lower_4_bits`)."""
    rc = 0
    oid = xc.new_object_id(0x123, xc.OBJK_DATAWRITER)
    rc |= check("ObjectId packs kind into low 4 bits", oid == ((0x123 << 4) | 0x05))
    rc |= check("ObjectId kind/raw roundtrip",
                xc.object_id_kind(oid) == xc.OBJK_DATAWRITER and xc.object_id_raw12(oid) == 0x123)
    back = xc.object_id_from_bytes(xc.object_id_bytes(oid))
    rc |= check("ObjectId bytes roundtrip", back == oid)
    return rc


def test_header_key_mismatch_rejected():
    """Cross-checks the constructor guard crates/xrce's
    `MessageHeader::with_client_key`/`without_client_key` enforce."""
    rc = 0
    try:
        xc._encode_message_header(10, xc.STREAM_ID_NONE, 0, None)
        rc |= check("session_id<=127 without a key rejected", False)
    except ValueError:
        rc |= check("session_id<=127 without a key rejected", True)
    try:
        xc._encode_message_header(200, xc.STREAM_ID_NONE, 0, KEY)
        rc |= check("session_id>=128 with a key rejected", False)
    except ValueError:
        rc |= check("session_id>=128 with a key rejected", True)
    return rc


def main():
    rc = 0
    rc |= test_state_machine()
    rc |= test_wire_fixtures()
    rc |= test_object_id_packing()
    rc |= test_header_key_mismatch_rejected()
    rc |= test_topic_representation_matches_the_rust_byte_oracle()
    rc |= test_topic_create_without_parent_rejected()
    if rc == 0:
        print("ALL OK")
    return rc


if __name__ == "__main__":
    sys.exit(main())
