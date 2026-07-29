# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# zerodds_endpoint -- XRCE + serial framing for the pure-Python endpoint SDK
# (ADR 0013). Mirrors endpoints/c/src/zerodds_endpoint.c byte-for-byte: XRCE
# WRITE_DATA/DATA, HEARTBEAT/ACKNACK, and the Annex-C serial HDLC
# framing (byte-stuffing + CRC-16-CCITT-FALSE). Pure stdlib, 2.7/3.x.

from __future__ import print_function

XRCE_SESSION_NOKEY = 0x80
XRCE_STREAM_BEST_EFFORT = 0x01
XRCE_STREAM_NONE = 0x00

_SM_WRITE_DATA = 0x07
_SM_DATA = 0x09
_SM_ACKNACK = 0x0A
_SM_HEARTBEAT = 0x0B
_WRITE_FLAGS = 0x03
_FLAG = 0x7E
_ESC = 0x7D
_STUFF = 0x20


def xrce_write_frame(session, stream, seq, sample):
    """Wrap an XCDR sample body in an XRCE WRITE_DATA (Sample) message."""
    out = bytearray()
    out.append(session & 0xFF)
    out.append(stream & 0xFF)
    out.append(seq & 0xFF)
    out.append((seq >> 8) & 0xFF)
    out.append(_SM_WRITE_DATA)
    out.append(_WRITE_FLAGS)
    out.append(len(sample) & 0xFF)
    out.append((len(sample) >> 8) & 0xFF)
    out.extend(bytearray(sample))
    return bytes(out)


def xrce_read_frame(frame):
    """Return the sample body of a WRITE_DATA (7) or DATA (9) frame."""
    b = bytearray(frame)
    if len(b) < 8 or b[0] < 0x80 or b[4] not in (_SM_WRITE_DATA, _SM_DATA):
        raise ValueError("not a WRITE_DATA/DATA frame")
    sm_len = b[6] | (b[7] << 8)
    if sm_len + 8 > len(b):
        raise ValueError("truncated")
    return bytes(b[8:8 + sm_len])


def crc16_ccitt_false(data):
    crc = 0xFFFF
    for byte in bytearray(data):
        crc ^= byte << 8
        for _ in range(8):
            if crc & 0x8000:
                crc = ((crc << 1) ^ 0x1021) & 0xFFFF
            else:
                crc = (crc << 1) & 0xFFFF
    return crc & 0xFFFF


def _stuff(out, data):
    for byte in bytearray(data):
        if byte == _FLAG or byte == _ESC:
            out.append(_ESC)
            out.append(byte ^ _STUFF)
        else:
            out.append(byte)


def serial_frame(payload):
    """HDLC frame: 7E [stuffed payload + crc16 BE] 7E."""
    crc = crc16_ccitt_false(payload)
    out = bytearray([_FLAG])
    _stuff(out, payload)
    _stuff(out, bytearray([(crc >> 8) & 0xFF, crc & 0xFF]))
    out.append(_FLAG)
    return bytes(out)


def serial_deframe(frame):
    b = bytearray(frame)
    i = 1 if (len(b) >= 1 and b[0] == _FLAG) else 0
    out = bytearray()
    while i < len(b) and b[i] != _FLAG:
        byte = b[i]
        if byte == _ESC:
            i += 1
            byte = b[i] ^ _STUFF
        out.append(byte)
        i += 1
    if len(out) < 2:
        raise ValueError("short frame")
    crc = crc16_ccitt_false(out[:-2])
    if out[-2] != ((crc >> 8) & 0xFF) or out[-1] != (crc & 0xFF):
        raise ValueError("crc mismatch")
    return bytes(out[:-2])


def acknack_frame(session, stream, seq, first_unacked, nack_lo, nack_hi, payload_stream):
    out = bytearray()
    out.append(session & 0xFF)
    out.append(stream & 0xFF)
    out.append(seq & 0xFF)
    out.append((seq >> 8) & 0xFF)
    out.append(_SM_ACKNACK)
    out.append(0x01)  # FLAG_E_LITTLE_ENDIAN
    out.append(5)
    out.append(0)
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

    def __init__(self, transport, session=XRCE_SESSION_NOKEY,
                 stream=XRCE_STREAM_BEST_EFFORT):
        self.transport = transport
        self.session = session
        self.stream = stream
        self.seq = 1

    def write(self, sample):
        frame = xrce_write_frame(self.session, self.stream, self.seq, sample)
        self.seq = (self.seq + 1) & 0xFFFF
        self.transport.deliver(frame)

    def poll(self):
        """One non-blocking receive: the sample body, or None."""
        frame = self.transport.receive()
        if frame is None:
            return None
        return xrce_read_frame(frame)
