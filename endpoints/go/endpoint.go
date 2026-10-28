// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

// XRCE session/stream constants (best-effort, no ClientKey).
//
// Direction (DDS-XRCE spec §8.3.5): WRITE_DATA (0x07) is client->agent — what
// this endpoint SENDS; DATA (0x09) is agent->client — what this endpoint
// RECEIVES from the hub. The reader therefore accepts both (parity with the C
// and Python SDKs): 0x09 for real hub traffic, 0x07 for loopback/self-test.
const (
	XrceSessionNoKey     byte = 0x80
	XrceStreamBestEffort byte = 0x01
	xrceSMWriteData      byte = 0x07 // endpoint -> hub
	xrceSMData           byte = 0x09 // hub -> endpoint
	// E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits
	// 1..3 — DDS-XRCE 1.0 §8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would
	// additionally require a SampleInfo in the body, which this profile does
	// not carry.
	xrceWriteFlags byte = 0x01
	// XrceBaseSize is the wire size of the BaseObjectRequest that precedes the
	// sample (§7.7.8: request_id[2] + object_id[2]).
	XrceBaseSize = 4
	// ObjkDataWriter (OBJK_DATAWRITER, §7.2.2) lives in xrce_client_wire.go.
)

// XrceObjectID builds an ObjectId (§7.2.1) from a 12-bit raw id and a 4-bit
// ObjectKind.
func XrceObjectID(rawID, kind uint16) uint16 {
	return ((rawID & 0x0FFF) << 4) | (kind & 0x0F)
}

// XrceWriteFrame wraps an XCDR sample body in an XRCE WRITE_DATA message:
// 4-byte header (session, stream, seq LE) + 4-byte submessage header (id,
// flags, length LE) + the BaseObjectRequest (§7.7.8: request_id then object_id,
// each two octets most significant first) + the sample. Byte-identical to the C
// SDK / zerodds-xrce.
//
// objectID names the target DataWriter — without it no XRCE agent can route the
// sample. The request_id is the message's own seq, which §7.7.8 only requires
// to be unique per stream.
//
// Returns nil when the body is larger than the 16-bit submessage_length field
// can describe (0xFFFF): refuse rather than truncate the length while appending
// the full payload (the C SDK rejects the same way).
func XrceWriteFrame(session, stream byte, seq, objectID uint16, sample []byte) []byte {
	if len(sample)+XrceBaseSize > 0xFFFF {
		return nil
	}
	out := make([]byte, 0, 8+XrceBaseSize+len(sample))
	n := uint16(len(sample) + XrceBaseSize)
	out = append(out,
		session, stream, byte(seq), byte(seq>>8),
		xrceSMWriteData, xrceWriteFlags, byte(n), byte(n>>8),
		byte(seq>>8), byte(seq), byte(objectID>>8), byte(objectID))
	return append(out, sample...)
}

// XrceReadFrame returns the sample body inside a received DATA (0x09, hub ->
// endpoint) or WRITE_DATA (0x07, loopback) frame — the bytes AFTER the 4-octet
// BaseObjectRequest. ok is false — never a panic or a bogus sample — if the
// frame is shorter than the header, carries some other submessage id, declares
// a body length that runs past the datagram (truncated / wrong length), or
// declares a body too short to hold the BaseObjectRequest. The returned body is
// bounded by the declared length, so trailing bytes (e.g. an appended
// submessage) never leak in.
func XrceReadFrame(frame []byte) (body []byte, ok bool) {
	_, _, body, ok = XrceReadFrameWithBase(frame)
	return body, ok
}

// XrceReadFrameWithBase is XrceReadFrame plus the frame's BaseObjectRequest —
// the (requestID, objectID) an agent routes on (§7.7.8).
func XrceReadFrameWithBase(frame []byte) (requestID, objectID uint16, body []byte, ok bool) {
	if len(frame) < 8+XrceBaseSize || (frame[4] != xrceSMData && frame[4] != xrceSMWriteData) {
		return 0, 0, nil, false
	}
	smLen := int(frame[6]) | int(frame[7])<<8
	if smLen < XrceBaseSize || 8+smLen > len(frame) {
		return 0, 0, nil, false
	}
	requestID = uint16(frame[8])<<8 | uint16(frame[9])
	objectID = uint16(frame[10])<<8 | uint16(frame[11])
	return requestID, objectID, frame[8+XrceBaseSize : 8+smLen], true
}
