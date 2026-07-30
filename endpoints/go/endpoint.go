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
	xrceWriteFlags       byte = 0x03
)

// XrceWriteFrame wraps an XCDR sample body in an XRCE WRITE_DATA message:
// 4-byte header (session, stream, seq LE) + 4-byte submessage header (id,
// flags, length LE) + the sample. Byte-identical to the C SDK / zerodds-xrce.
// Returns nil when the sample is larger than the 16-bit submessage_length field
// can describe (0xFFFF): refuse rather than truncate the length while appending
// the full payload (the C SDK rejects the same way).
func XrceWriteFrame(session, stream byte, seq uint16, sample []byte) []byte {
	if len(sample) > 0xFFFF {
		return nil
	}
	out := make([]byte, 0, 8+len(sample))
	n := uint16(len(sample))
	out = append(out,
		session, stream, byte(seq), byte(seq>>8),
		xrceSMWriteData, xrceWriteFlags, byte(n), byte(n>>8))
	return append(out, sample...)
}

// XrceReadFrame returns the sample body inside a received DATA (0x09, hub ->
// endpoint) or WRITE_DATA (0x07, loopback) frame. ok is false — never a panic or
// a bogus sample — if the frame is shorter than the 8-byte header, carries some
// other submessage id, or declares a body length that runs past the datagram
// (truncated / wrong length). The returned body is bounded by the declared
// length, so trailing bytes (e.g. an appended submessage) never leak in.
func XrceReadFrame(frame []byte) (body []byte, ok bool) {
	if len(frame) < 8 || (frame[4] != xrceSMData && frame[4] != xrceSMWriteData) {
		return nil, false
	}
	smLen := int(frame[6]) | int(frame[7])<<8
	if 8+smLen > len(frame) {
		return nil, false
	}
	return frame[8 : 8+smLen], true
}
