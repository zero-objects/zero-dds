# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# zerodds_endpoint -- XRCE + serial framing for the pure-Python endpoint SDK
# (ADR 0013). Mirrors endpoints/c/src/zerodds_endpoint.c byte-for-byte: XRCE
# WRITE_DATA/DATA, HEARTBEAT/ACKNACK, and the DDS-XRCE 1.0 Annex C serial
# framing (addresses, length, byte-stuffing, RFC 1662 FCS-16). Pure stdlib,
# 2.7/3.x.

from __future__ import print_function

XRCE_SESSION_NOKEY = 0x80
XRCE_STREAM_BEST_EFFORT = 0x01
XRCE_STREAM_NONE = 0x00

_SM_WRITE_DATA = 0x07
_SM_DATA = 0x09
_SM_ACKNACK = 0x0A
_SM_HEARTBEAT = 0x0B
# E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
# DDS-XRCE 1.0 Sec 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would additionally
# require a SampleInfo in the body, which this profile does not carry.
_WRITE_FLAGS = 0x01
# Wire size of the BaseObjectRequest that precedes the sample (Sec 7.7.8).
XRCE_BASE_SIZE = 4
# OBJK_DATAWRITER -- the ObjectKind of a DataWriter (Sec 7.2.2).
OBJK_DATAWRITER = 0x05


def xrce_object_id(raw_id, kind):
    """An ObjectId (Sec 7.2.1) from a 12-bit raw id and a 4-bit ObjectKind."""
    return ((raw_id & 0x0FFF) << 4) | (kind & 0x0F)
_FLAG = 0x7E
_ESC = 0x7D
_STUFF = 0x20
_SERIAL_HEADER_LEN = 4


def xrce_write_frame(session, stream, seq, object_id, sample):
    """Wrap an XCDR sample body in an XRCE WRITE_DATA message.

    The submessage body is the BaseObjectRequest (Sec 7.7.8: request_id then
    object_id, each two octets most significant first) followed by the sample.
    `object_id` names the target DataWriter -- without it no XRCE agent can
    route the sample. The request_id is the message's own `seq`, which Sec 7.7.8
    only requires to be unique per stream.

    The 16-bit submessage_length field cannot describe a body larger than
    0xFFFF; raise rather than truncate the length while appending the full
    payload (mirrors the C SDK's rejection).
    """
    body_len = XRCE_BASE_SIZE + len(sample)
    if body_len > 0xFFFF:
        raise ValueError("sample exceeds 16-bit submessage_length")
    out = bytearray()
    out.append(session & 0xFF)
    out.append(stream & 0xFF)
    out.append(seq & 0xFF)
    out.append((seq >> 8) & 0xFF)
    out.append(_SM_WRITE_DATA)
    out.append(_WRITE_FLAGS)
    out.append(body_len & 0xFF)
    out.append((body_len >> 8) & 0xFF)
    out.append((seq >> 8) & 0xFF)
    out.append(seq & 0xFF)
    out.append((object_id >> 8) & 0xFF)
    out.append(object_id & 0xFF)
    out.extend(bytearray(sample))
    return bytes(out)


def xrce_frame_base(frame):
    """`(request_id, object_id)` of a WRITE_DATA (7) or DATA (9) frame."""
    b = _checked_data_frame(frame)
    return ((b[8] << 8) | b[9], (b[10] << 8) | b[11])


def _checked_data_frame(frame):
    """Validate a data frame and return it as a bytearray."""
    b = bytearray(frame)
    if (len(b) < 8 + XRCE_BASE_SIZE or b[0] < 0x80
            or b[4] not in (_SM_WRITE_DATA, _SM_DATA)):
        raise ValueError("not a WRITE_DATA/DATA frame")
    sm_len = b[6] | (b[7] << 8)
    if sm_len + 8 > len(b):
        raise ValueError("truncated")
    if sm_len < XRCE_BASE_SIZE:
        raise ValueError("body too short for a BaseObjectRequest")
    return b


def xrce_read_frame(frame):
    """Return the sample body of a WRITE_DATA (7) or DATA (9) frame -- the
    bytes after the 4-octet BaseObjectRequest."""
    b = _checked_data_frame(frame)
    sm_len = b[6] | (b[7] << 8)
    return bytes(b[8 + XRCE_BASE_SIZE:8 + sm_len])


def fcs16(data):
    """FCS-16 of RFC 1662 (reflected 0x8408, init 0xFFFF, complemented);
    0x906E for b"123456789"."""
    crc = 0xFFFF
    for byte in bytearray(data):
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ 0x8408 if crc & 1 else crc >> 1
    return (~crc) & 0xFFFF


def _stuff(out, data):
    for byte in bytearray(data):
        if byte == _FLAG or byte == _ESC:
            out.append(_ESC)
            out.append(byte ^ _STUFF)
        else:
            out.append(byte)


def serial_frame(source, remote, payload):
    """DDS-XRCE 1.0 Annex C.1.1 frame from `source` to `remote`:
    7E | source | remote | length LE | payload | FCS-16 LE, stuffed after
    the flag."""
    payload = bytearray(payload)
    if len(payload) > 0xFFFF:
        raise ValueError("payload over 65535 bytes")
    raw = bytearray([source & 0xFF, remote & 0xFF,
                     len(payload) & 0xFF, (len(payload) >> 8) & 0xFF])
    raw += payload
    crc = fcs16(raw)
    raw += bytearray([crc & 0xFF, (crc >> 8) & 0xFF])
    out = bytearray([_FLAG])
    _stuff(out, raw)
    return bytes(out)


def serial_deframe(frame):
    """The frame at the start of `frame` (beginning with its 0x7E) as
    (source, remote, payload). Refuses a wrong frameCheck, a bad escape, a
    0x7E before the frame is complete (Annex C.1.3) and a frame that ends
    early."""
    b = bytearray(frame)
    if len(b) < 1 or b[0] != _FLAG:
        raise ValueError("no BEGIN_FLAG")
    raw = bytearray()
    total = _SERIAL_HEADER_LEN + 2
    i = 1
    while i < len(b) and len(raw) < total:
        byte = b[i]
        if byte == _FLAG:
            raise ValueError("BEGIN_FLAG inside the frame")
        if byte == _ESC:
            i += 1
            if i >= len(b):
                raise ValueError("dangling escape")
            byte = b[i] ^ _STUFF
            if byte != _FLAG and byte != _ESC:
                raise ValueError("invalid escape")
        raw.append(byte)
        if len(raw) == _SERIAL_HEADER_LEN:
            total = _SERIAL_HEADER_LEN + (raw[2] | (raw[3] << 8)) + 2
        i += 1
    if len(raw) < total:
        raise ValueError("short frame")
    crc = fcs16(raw[:-2])
    if raw[-2] != (crc & 0xFF) or raw[-1] != ((crc >> 8) & 0xFF):
        raise ValueError("frameCheck mismatch")
    return raw[0], raw[1], bytes(raw[_SERIAL_HEADER_LEN:-2])


def acknack_frame(session, stream, seq, first_unacked, nack_lo, nack_hi, payload_stream,
                  unannotated_types="appendable"):
    # ACKNACK_Payload has no @extensibility, so it is APPENDABLE (DDS-XTypes 1.3
    # section 7.3.1.2.1.8): unannotated_types="appendable" (default) writes the
    # 4-octet DHEADER (=5) + 5-octet body = 9 octets; "final" writes the 5-octet
    # body of the reference implementation (eProsima), length field 5.
    if unannotated_types not in ("appendable", "final"):
        raise ValueError(
            "unannotated_types must be 'appendable' or 'final', got %r" % (unannotated_types,))
    final = unannotated_types == "final"
    out = bytearray()
    out.append(session & 0xFF)
    out.append(stream & 0xFF)
    out.append(seq & 0xFF)
    out.append((seq >> 8) & 0xFF)
    out.append(_SM_ACKNACK)
    out.append(0x01)  # FLAG_E_LITTLE_ENDIAN
    out.append(5 if final else 9)
    out.append(0)
    if not final:
        out.extend((5, 0, 0, 0))  # DHEADER u32 LE: body size 5
    out.append(first_unacked & 0xFF)
    out.append((first_unacked >> 8) & 0xFF)
    out.append(nack_lo & 0xFF)
    out.append(nack_hi & 0xFF)
    out.append(payload_stream & 0xFF)
    return bytes(out)


def heartbeat_read(frame):
    """Return (first, last, stream) from a HEARTBEAT frame."""
    b = bytearray(frame)
    if len(b) < 13 or b[4] != _SM_HEARTBEAT:
        raise ValueError("not a HEARTBEAT")

    def i16(lo, hi):
        v = lo | (hi << 8)
        return v - 0x10000 if v & 0x8000 else v

    return (i16(b[8], b[9]), i16(b[10], b[11]), b[12])


# --- transport + sync client (stdlib, 2.7/3.x) -------------------------------
#
# A transport is any object with deliver(frame) and receive()->frame|None. The
# async reader (asyncio, Python 3) lives in example_async.py, so this module
# stays import-clean on 2.7.


class MemTransport(object):
    """In-memory FIFO transport for tests + examples."""

    def __init__(self):
        self._q = []

    def deliver(self, frame):
        self._q.append(bytes(frame))

    def receive(self):
        return self._q.pop(0) if self._q else None


class Client(object):
    """Sync endpoint: frames an XCDR sample and delivers it; polls one back."""

    def __init__(self, transport, object_id, session=XRCE_SESSION_NOKEY,
                 stream=XRCE_STREAM_BEST_EFFORT):
        self.transport = transport
        # ObjectId of the target DataWriter, in every frame's
        # BaseObjectRequest (Sec 7.7.8).
        self.object_id = object_id
        self.session = session
        self.stream = stream
        self.seq = 1

    def write(self, sample):
        frame = xrce_write_frame(self.session, self.stream, self.seq,
                                 self.object_id, sample)
        self.seq = (self.seq + 1) & 0xFFFF
        self.transport.deliver(frame)

    def poll(self):
        """One non-blocking receive: the sample body, or None."""
        frame = self.transport.receive()
        if frame is None:
            return None
        return xrce_read_frame(frame)
