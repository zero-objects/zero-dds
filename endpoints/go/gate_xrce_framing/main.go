// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// Prints one XRCE WRITE_DATA frame as hex, for
// crates/endpoint-e2e/tests/xrce_write_framing.rs.
package main

import (
	"encoding/hex"
	"fmt"

	zerodds "zeroddsendpoint"
)

func main() {
	frame := zerodds.XrceWriteFrame(
		zerodds.XrceSessionNoKey, zerodds.XrceStreamBestEffort, 1,
		zerodds.XrceObjectID(0x001, uint16(zerodds.ObjkDataWriter)),
		[]byte{0xDE, 0xAD, 0xBE, 0xEF})
	fmt.Println(hex.EncodeToString(frame))
}
