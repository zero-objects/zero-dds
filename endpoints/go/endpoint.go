// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

// XRCE session/stream constants (best-effort, no ClientKey).
const (
	XrceSessionNoKey     byte = 0x80
	XrceStreamBestEffort byte = 0x01
	xrceSMWriteData      byte = 0x07
	xrceWriteFlags       byte = 0x03
)

// XrceWriteFrame wraps an XCDR sample body in an XRCE WRITE_DATA message:
// 4-byte header (session, stream, seq LE) + 4-byte submessage header (id,
// flags, length LE) + the sample. Byte-identical to the C SDK / zerodds-xrce.
func XrceWriteFrame(session, stream byte, seq uint16, sample []byte) []byte {
	out := make([]byte, 0, 8+len(sample))
	n := uint16(len(sample))
	out = append(out,
		session, stream, byte(seq), byte(seq>>8),
		xrceSMWriteData, xrceWriteFlags, byte(n), byte(n>>8))
	return append(out, sample...)
}

// XrceReadFrame returns the sample body inside a received WRITE_DATA frame; ok
// is false if the frame is too short or not a WRITE_DATA submessage.
func XrceReadFrame(frame []byte) (body []byte, ok bool) {
	if len(frame) < 8 || frame[4] != xrceSMWriteData {
		return nil, false
	}
	return frame[8:], true
}
