# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# zerodds_xrce_client -- DDS-XRCE 1.0 client state machine + wire codec. A
# port of crates/xrce-client/src/lib.rs's `XrceClient<T>` state machine (340
# lines) combined with the concrete submessage codecs of crates/xrce
# (CREATE_CLIENT/CREATE/DELETE/WRITE_DATA/READ_DATA). crates/xrce-client
# itself never touches the wire -- its module doc says so directly: "the
# caller delegates submessage encoding; we only track state". This module is
# that combination.
#
# Distinct from zerodds_endpoint.py's xrce_write_frame/xrce_read_frame (the
# pre-existing simplified WRITE_DATA data-path framing, sample body only, no
# BaseObjectRequest) -- XrceClient here speaks the spec-conformant
# object-lifecycle frames a real XRCE agent expects for
# CREATE_CLIENT/CREATE/DELETE/READ_DATA, plus the conformant WRITE_DATA that
# carries the target ObjectId (DDS-XRCE 1.0 Annex B.3.1). It is the caller's
# job to hand the frames this module builds to a Transport's deliver() (the
# same transport zerodds_endpoint.Client already uses) -- no new transport
# is introduced here.
#
# Spec mapping (mirrors crates/xrce-client/src/lib.rs's module doc):
#   XrceClient()   -> Sec 7.8.2 CREATE_CLIENT
#   connect        -> Sec 8.4.5 (handshake)
#   create_object  -> Sec 7.8.3 CREATE
#   delete_object  -> Sec 7.8.3 DELETE
#   request_write  -> Sec 7.8.4 WRITE_DATA
#   request_read   -> Sec 7.8.5 READ_DATA
#   disconnect     -> Sec 8.4.5
#
# Pure stdlib, Python 2.7/3.x (matches zerodds_endpoint.py).

from __future__ import print_function

import time

# --- ObjectKind / ObjectId (spec Sec 7.2, Sec 7.2.1) ------------------------

OBJK_INVALID = 0x00
OBJK_PARTICIPANT = 0x01
OBJK_TOPIC = 0x02
OBJK_PUBLISHER = 0x03
OBJK_SUBSCRIBER = 0x04
OBJK_DATAWRITER = 0x05
OBJK_DATAREADER = 0x06
OBJK_TYPE = 0x0A
OBJK_QOSPROFILE = 0x0B
OBJK_APPLICATION = 0x0C
OBJK_AGENT = 0x0D
OBJK_CLIENT = 0x0E
OBJK_OTHER = 0x0F

OBJECTID_INVALID = 0x0000  # DDS-XRCE 1.0 §7.6: {0x00, 0x00}


def new_object_id(raw_id, kind):
    """Pack a 12-bit raw_id and a 4-bit kind into a 16-bit ObjectId."""
    if raw_id > 0x0FFF:
        raise ValueError("ObjectId raw_id exceeds 12 bits")
    return (raw_id << 4) | (kind & 0x0F)


def object_id_kind(object_id):
    return object_id & 0x0F


def object_id_raw12(object_id):
    return (object_id >> 4) & 0x0FFF


def object_id_bytes(object_id):
    """Wire encoding: 2 octets, big-endian (crates/xrce `ObjectId::to_bytes`)."""
    return bytearray([(object_id >> 8) & 0xFF, object_id & 0xFF])


def object_id_from_bytes(b):
    b = bytearray(b)
    if len(b) < 2:
        raise ValueError("need 2 bytes for an ObjectId")
    return (b[0] << 8) | b[1]


# --- ObjectVariant (spec Sec 7.7) -------------------------------------------

_VARIANT_BY_REFERENCE = 0x01
_VARIANT_BY_XML_STRING = 0x02
_VARIANT_IN_BINARY = 0x03


class ObjectVariant(object):
    """Object-specific CREATE payload. Build via by_reference/by_xml_string/
    in_binary -- do not construct directly."""

    def __init__(self, kind, str_payload=None, bin_payload=None):
        self.kind = kind
        self.str_payload = str_payload
        self.bin_payload = bin_payload

    def _payload_and_nul(self):
        if self.kind in (_VARIANT_BY_REFERENCE, _VARIANT_BY_XML_STRING):
            return bytearray(self.str_payload.encode("utf-8")), True
        return bytearray(self.bin_payload), False

    def _encode_inner(self, pad_len):
        """disc(1) + `pad_len` zero padding bytes + u32 length (little
        endian) + payload, with a trailing NUL for the two string variants
        (XCDR2 `string<>`, crates/xrce/src/object_repr.rs). `pad_len` is 3
        for the never-used-on-the-wire single-level form (this structure
        alone starts 4-aligned) and 2 for `encode_for_kind` (a 1-byte
        ObjectKind precedes it there)."""
        payload, extra_nul = self._payload_and_nul()
        n = len(payload) + (1 if extra_nul else 0)
        out = bytearray([self.kind]) + bytearray(pad_len)
        out += bytearray([n & 0xFF, (n >> 8) & 0xFF, (n >> 16) & 0xFF, (n >> 24) & 0xFF])
        out += payload
        if extra_nul:
            out.append(0)
        return out

    def encode_for_kind(self, object_kind, domain_id=None, parent_object_id=None):
        """The real wire `ObjectVariant` (spec Sec 7.7.3): a **two-level**
        discriminated union -- the outer `ObjectKind` byte (`object_kind`,
        e.g. OBJK_DATAWRITER) selects which `..._Representation` struct
        follows, then this inner format-discriminated payload
        (`_encode_inner`), then that struct's own trailing field:
        `domain_id` (`short`, 2-byte aligned) for OBJK_PARTICIPANT,
        `parent_object_id` (`ObjectId`, unaligned) for OBJK_TOPIC/
        OBJK_PUBLISHER/OBJK_SUBSCRIBER/OBJK_DATAWRITER/OBJK_DATAREADER, or
        nothing for every other kind -- byte-oracled in
        crates/xrce/src/object_repr.rs against real eProsima
        Micro-XRCE-DDS Client 3.0.1 CREATE traffic (OBJK_TOPIC's own
        participant_id: commit 31c62df2a63). This -- not the bare
        `_encode_inner` -- is the only form a real agent accepts."""
        has_domain_id = object_kind == OBJK_PARTICIPANT
        has_parent = object_kind in (OBJK_TOPIC, OBJK_PUBLISHER, OBJK_SUBSCRIBER,
                                      OBJK_DATAWRITER, OBJK_DATAREADER)
        if has_domain_id and domain_id is None:
            raise ValueError("OBJK_PARTICIPANT representation requires domain_id")
        if not has_domain_id and domain_id is not None:
            raise ValueError("only OBJK_PARTICIPANT representation carries domain_id")
        if has_parent and parent_object_id is None:
            raise ValueError("this ObjectKind's representation requires a parent id")
        if not has_parent and parent_object_id is not None:
            raise ValueError("this ObjectKind's representation carries no parent id")

        out = bytearray([object_kind]) + bytearray(self._encode_inner(2))
        if domain_id is not None:
            if len(out) % 2 != 0:
                out.append(0)  # `short` aligns to 2 bytes; ObjectId (below) does not
            d = domain_id & 0xFFFF
            out += bytearray([d & 0xFF, (d >> 8) & 0xFF])
        if parent_object_id is not None:
            out += bytearray(object_id_bytes(parent_object_id))
        return bytes(out)


def by_reference(name):
    """REPRESENTATION_BY_REFERENCE: a name pointing to a template registered
    in the agent."""
    return ObjectVariant(_VARIANT_BY_REFERENCE, str_payload=name)


def by_xml_string(xml):
    """REPRESENTATION_AS_XML_STRING: inline XML describing the object."""
    return ObjectVariant(_VARIANT_BY_XML_STRING, str_payload=xml)


def in_binary(data):
    """REPRESENTATION_IN_BINARY: inline XCDR2-encoded bytes."""
    return ObjectVariant(_VARIANT_IN_BINARY, bin_payload=data)


# --- BaseObjectRequest (spec Sec 7.7.8) -------------------------------------

def _request_id_bytes(request_id):
    """Packs the state machine's u16 request id into the wire
    RequestId(2 octets) field, most significant octet first.

    DDS-XRCE 1.0 gives RequestId as octet[2] and never as an integer, so the
    wire has no byte order -- but this client hands out a uint16 counter,
    and the mapping from one to the other went unstated until three ports
    had already chosen one way and three the other. crates/xrce-client's
    `request_id_octets` settles it as big-endian, which is what the
    fixtures under endpoints/testdata/xrce-client hold: [0x00, 0x01] for
    the first request."""
    return bytearray([(request_id >> 8) & 0xFF, request_id & 0xFF])


def encode_base_object_request(request_id_bytes_, object_id):
    """`request_id (2 octets) + object_id (2 octets)` -- the header every
    CREATE/DELETE/WRITE_DATA/READ_DATA body starts with."""
    return bytes(bytearray(request_id_bytes_) + object_id_bytes(object_id))


# --- DataFormat / ReadSpecification / DataDeliveryControl (spec Sec 8.3.5.9)

DATA_FORMAT_DATA = 0x00  # FORMAT_DATA (0b000) -- a bare sample, no SampleInfo.


class DataDeliveryControl(object):
    """Paces a READ_DATA reply: four unsigned shorts, 8 octets little-endian."""

    def __init__(self, max_samples, max_elapsed_time, max_bytes_per_second, min_pace_period):
        self.max_samples = max_samples
        self.max_elapsed_time = max_elapsed_time
        self.max_bytes_per_second = max_bytes_per_second
        self.min_pace_period = min_pace_period

    def encode(self):
        def u16(v):
            return bytearray([v & 0xFF, (v >> 8) & 0xFF])
        out = bytearray()
        out += u16(self.max_samples)
        out += u16(self.max_elapsed_time)
        out += u16(self.max_bytes_per_second)
        out += u16(self.min_pace_period)
        return bytes(out)


class ReadSpecification(object):
    """How the client wants READ_DATA's samples delivered. The two
    @optional members use the conservative presence-flag encoding
    crates/xrce/src/submessages/read_data.rs chose (1 = present / 0 = absent
    ahead of the payload), not the full XCDR2 EMHEADER form -- Phase 2
    there too."""

    def __init__(self, preferred_stream_id, data_format=DATA_FORMAT_DATA,
                 content_filter_expression=None, delivery_control=None):
        self.preferred_stream_id = preferred_stream_id
        self.data_format = data_format
        self.content_filter_expression = content_filter_expression
        self.delivery_control = delivery_control

    def encode(self):
        out = bytearray([self.preferred_stream_id & 0xFF, self.data_format & 0xFF])
        if self.content_filter_expression is not None:
            s = bytearray(self.content_filter_expression.encode("utf-8"))
            n = len(s)
            out.append(1)
            out += bytearray([n & 0xFF, (n >> 8) & 0xFF, (n >> 16) & 0xFF, (n >> 24) & 0xFF])
            out += s
        else:
            out.append(0)
        if self.delivery_control is not None:
            out.append(1)
            out += bytearray(self.delivery_control.encode())
        else:
            out.append(0)
        return bytes(out)


# --- MessageHeader (spec Sec 8.3.2) -----------------------------------------

SESSION_ID_NONE_WITH_CLIENT_KEY = 0x00
SESSION_ID_NONE_WITHOUT_CLIENT_KEY = 0x80

STREAM_ID_NONE = 0x00
STREAM_ID_BEST_EFFORT = 0x01
STREAM_ID_BUILTIN_RELIABLE = 0x80

CLIENT_KEY_LEN = 4
CLIENTKEY_INVALID = bytes(bytearray(CLIENT_KEY_LEN))  # CLIENTKEY_INVALID (spec Sec 7.8.2.2)


def _encode_message_header(session, stream, seq, client_key):
    """session_id(1) + stream_id(1) + sequence_nr(2, LE) + an optional
    4-octet ClientKey -- present exactly when session<=127 (spec
    Sec 8.3.2.1). Raises if `client_key`'s presence disagrees, mirroring
    `MessageHeader::with_client_key`/`without_client_key` refusing the same
    mismatch."""
    carries_key = session <= 127
    if carries_key != (client_key is not None):
        raise ValueError(
            "xrce message header: session_id %#x requires client_key=%r"
            % (session, carries_key))
    out = bytearray([session & 0xFF, stream & 0xFF, seq & 0xFF, (seq >> 8) & 0xFF])
    if client_key is not None:
        out += bytearray(client_key)
    return bytes(out)


# --- submessage ids (spec Sec 8.3.5, values 0..15) --------------------------

_SM_CREATE_CLIENT = 0x00
_SM_CREATE = 0x01
_SM_DELETE = 0x03
_SM_WRITE_DATA = 0x07
_SM_READ_DATA = 0x08

_FLAG_E_LE = 0x01  # E-flag: the submessage body is little-endian (spec Sec 8.3.4).


def _build_message(session, stream, seq, client_key, sm_id, flags, body):
    """One full XRCE Message: MessageHeader + one SubmessageHeader (id,
    flags, little-endian u16 length) + body. No padding is needed -- a
    single submessage is always the last one in its message (spec Sec 8.3.3
    pads only *between* submessages)."""
    header = _encode_message_header(session, stream, seq, client_key)
    if len(body) > 0xFFFF:
        raise ValueError("submessage body exceeds the 16-bit length field")
    n = len(body)
    out = bytearray(header)
    out += bytearray([sm_id, flags, n & 0xFF, (n >> 8) & 0xFF])
    out += bytearray(body)
    return bytes(out)


XRCE_COOKIE = bytes(bytearray(b"XRCE"))
XRCE_VERSION = bytes(bytearray([0x01, 0x00]))
DEFAULT_MTU = 508  # eProsima's own hard datagram ceiling; see the doc below.


REPRESENTATION_TIMESTAMPS_ABSENT = "absent"
REPRESENTATION_TIMESTAMPS_PRESENT = "present"


def encode_client_representation(version, vendor_id, client_key, session_id,
                                  requested_mtu=DEFAULT_MTU,
                                  representation_timestamps="absent",
                                  seconds=0, nanoseconds=0,
                                  unannotated_types="appendable"):
    """`CLIENT_Representation` (Annex A, Sec 7.8.2.1): xrce_cookie(4) +
    xrce_version(2) + xrce_vendor_id(2) + client_key(4) + session_id(1) +
    a `0` properties-absent octet + `requested_mtu` (2, little endian) --
    eProsima's own non-normative MTU extension
    (crates/xrce/src/handshake.rs's module doc has the byte-oracle
    measurement backing `DEFAULT_MTU = 508`), always sent by this client.
    No `PropertySeq` support here -- this client sends none.

    `representation_timestamps`: "absent" (default, Annex A / B.1 form) or
    "present" (the prose form of Sec 7.8.2.1): after xrce_vendor_id the
    `Time_t client_timestamp` (`seconds` i32, `nanoseconds` u32, little
    endian). Time_t is APPENDABLE (DDS-XTypes 1.3 Sec 7.3.1.2.1.8) with
    `unannotated_types="appendable"` (default): DHEADER = 8 in front,
    12 octets; with "final" no DHEADER, 8 octets. The agent has to be set
    the same way (`representation_timestamps:` in its configuration)."""
    if representation_timestamps not in ("absent", "present"):
        raise ValueError(
            "representation_timestamps must be 'absent' or 'present', got %r"
            % (representation_timestamps,))
    if unannotated_types not in ("appendable", "final"):
        raise ValueError(
            "unannotated_types must be 'appendable' or 'final', got %r" % (unannotated_types,))
    out = bytearray(XRCE_COOKIE)
    out += bytearray(version)
    out += bytearray(vendor_id)
    if representation_timestamps == "present":
        if unannotated_types == "appendable":
            out += bytearray([8, 0, 0, 0])  # DHEADER of Time_t
        sec = seconds & 0xFFFFFFFF
        nsec = nanoseconds & 0xFFFFFFFF
        out += bytearray([(sec >> (8 * i)) & 0xFF for i in range(4)])
        out += bytearray([(nsec >> (8 * i)) & 0xFF for i in range(4)])
    out += bytearray(client_key)
    out.append(session_id & 0xFF)
    out.append(0)  # properties absent
    out += bytearray([requested_mtu & 0xFF, (requested_mtu >> 8) & 0xFF])
    return bytes(out)


def build_create_client_message(session, stream, seq, client_key, representation):
    """CREATE_CLIENT (Sec 7.8.2/Sec 8.3.5.1): the client -> agent handshake
    `XrceClient.connect()` starts. `representation` is the XCDR2-encoded
    CLIENT_Representation (build it with `encode_client_representation`)
    -- this function only frames it, without interpreting it. `session` is
    the header's session_id: SESSION_ID_NONE_WITH_CLIENT_KEY (0x00, with
    `client_key`) when the representation asks for a session_id of
    0..127, SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80, `client_key` None)
    when it asks for 128..255 (DDS-XRCE 1.0 Sec 8.3.2.1)."""
    return _build_message(session, stream, seq, client_key, _SM_CREATE_CLIENT,
                           _FLAG_E_LE, representation)


def build_create_message(session, stream, seq, client_key, request_id, object_id, object_kind,
                          variant, reuse=False, replace=False, domain_id=None,
                          parent_object_id=None):
    """CREATE (Sec 7.8.3/Sec 8.3.5.2) for the request id
    XrceClient.create_object returned. `object_kind` is the outer
    `ObjectKind` byte the wire `ObjectVariant` (Sec 7.7.3) always carries
    alongside the inner `variant`; `domain_id`/`parent_object_id` are that
    kind's own trailing field -- see `ObjectVariant.encode_for_kind`.
    `reuse`/`replace` set CreationMode bits 1/2 of the submessage flags."""
    body = bytearray(encode_base_object_request(_request_id_bytes(request_id), object_id))
    body += bytearray(variant.encode_for_kind(object_kind, domain_id=domain_id,
                                               parent_object_id=parent_object_id))
    flags = _FLAG_E_LE
    if reuse:
        flags |= 0x02
    if replace:
        flags |= 0x04
    return _build_message(session, stream, seq, client_key, _SM_CREATE, flags, bytes(body))


def build_delete_message(session, stream, seq, client_key, request_id, object_id):
    """DELETE (Sec 7.8.3/Sec 8.3.5.4) for the request id
    XrceClient.delete_object returned."""
    body = encode_base_object_request(_request_id_bytes(request_id), object_id)
    return _build_message(session, stream, seq, client_key, _SM_DELETE, _FLAG_E_LE, body)


def build_write_data_message(session, stream, seq, client_key, request_id, writer, sample):
    """WRITE_DATA (Sec 7.8.4/Sec 8.3.5.8, FORMAT_DATA) for the request id
    XrceClient.request_write returned: BaseObjectRequest (4 octets) then
    `sample` verbatim -- no length of its own (DDS-XRCE 1.0 Annex B.3.1;
    crates/xrce/src/submessages/write_data.rs)."""
    body = bytearray(encode_base_object_request(_request_id_bytes(request_id), writer))
    body += bytearray(sample)
    return _build_message(session, stream, seq, client_key, _SM_WRITE_DATA, _FLAG_E_LE, bytes(body))


def build_read_data_message(session, stream, seq, client_key, request_id, reader, spec):
    """READ_DATA (Sec 7.8.5/Sec 8.3.5.9) for the request id
    XrceClient.request_read returned."""
    body = bytearray(encode_base_object_request(_request_id_bytes(request_id), reader))
    body += bytearray(spec.encode())
    return _build_message(session, stream, seq, client_key, _SM_READ_DATA, _FLAG_E_LE, bytes(body))


# --- XrceClient state machine (port of crates/xrce-client's XrceClient<T>) --

DISCONNECTED = "Disconnected"
CONNECTING = "Connecting"
CONNECTED = "Connected"


class ClientError(Exception):
    """Mirrors `ClientError::InvalidState`: an operation was attempted in
    the wrong lifecycle state (e.g. create_object before connect)."""


class XrceClient(object):
    """Synchronous XRCE client state machine -- a port of
    crates/xrce-client's `XrceClient<T>`. It does not hold a transport and
    does not send anything itself, exactly like the Rust crate: pair its
    request ids with the build_*_message functions above and hand the
    result to your own transport's deliver(). That split is deliberate: it
    is what lets this module be tested against the same fixtures a Rust
    caller combining crates/xrce-client with crates/xrce would produce.
    """

    def __init__(self, client_key, representation_timestamps="absent",
                 unannotated_types="appendable"):
        self.client_key = bytes(bytearray(client_key))
        self.state = DISCONNECTED
        self._next_request_id = 1
        if representation_timestamps not in ("absent", "present"):
            raise ValueError("representation_timestamps must be 'absent' or 'present'")
        self.representation_timestamps = representation_timestamps
        self.unannotated_types = unannotated_types

    def client_representation(self, version=XRCE_VERSION, vendor_id=b"\x00\x00",
                              session_id=0x01, requested_mtu=DEFAULT_MTU):
        """The CLIENT_Representation of this client for
        build_create_client_message, with its `representation_timestamps`
        setting. The Time_t is the wall clock at the time of the call."""
        now = time.time()
        sec = int(now)
        return encode_client_representation(
            version, vendor_id, self.client_key, session_id, requested_mtu,
            self.representation_timestamps, sec, int((now - sec) * 1e9),
            self.unannotated_types)

    def connect(self):
        """Initiates the handshake (Disconnected -> Connecting). The caller
        still has to build and deliver CREATE_CLIENT itself, e.g. via
        build_create_client_message."""
        if self.state != DISCONNECTED:
            raise ClientError("invalid state: %s" % self.state)
        self.state = CONNECTING

    def mark_connected(self):
        """Completes the handshake once the caller has received
        STATUS_AGENT (Connecting -> Connected)."""
        if self.state != CONNECTING:
            raise ClientError("invalid state: %s" % self.state)
        self.state = CONNECTED

    def disconnect(self):
        """Closes the session -- back to Disconnected. Like the Rust
        client, this sends nothing."""
        self.state = DISCONNECTED

    def _require_connected(self):
        if self.state != CONNECTED:
            raise ClientError("invalid state: %s" % self.state)

    def _alloc_request_id(self):
        """Hands out the next request id, wrapping at 0xFFFF back to 1 (0 is
        never a valid request id) -- identical to
        `next_request_id.wrapping_add(1).max(1)` in crates/xrce-client."""
        req = self._next_request_id
        nxt = (req + 1) & 0xFFFF
        if nxt == 0:
            nxt = 1
        self._next_request_id = nxt
        return req

    def create_object(self, object_id, variant):
        """Submits a CREATE (Sec 7.8.3). Returns a request_id."""
        self._require_connected()
        return self._alloc_request_id()

    def delete_object(self, object_id):
        """Submits a DELETE (Sec 7.8.3)."""
        self._require_connected()
        return self._alloc_request_id()

    def request_write(self, writer, payload):
        """Submits a WRITE_DATA (Sec 7.8.4)."""
        self._require_connected()
        return self._alloc_request_id()

    def request_read(self, reader):
        """Submits a READ_DATA (Sec 7.8.5, pull-based)."""
        self._require_connected()
        return self._alloc_request_id()
