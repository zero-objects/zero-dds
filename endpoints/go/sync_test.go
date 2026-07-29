// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

import (
	"testing"
	"time"
)

func TestSyncLoopback(t *testing.T) {
	const N = 5
	tr := &memTransport{}
	c := NewClient(tr)

	for i := 0; i < N; i++ {
		if err := c.Write(sampleBody(0x3000 + uint32(i))); err != nil {
			t.Fatalf("write %d: %v", i, err)
		}
	}

	for i := 0; i < N; i++ {
		body, ok, err := c.Receive(2 * time.Second)
		if err != nil || !ok {
			t.Fatalf("receive %d: ok=%v err=%v", i, ok, err)
		}
		if id := NewReader(body, Little).GetU32(); id != 0x3000+uint32(i) {
			t.Fatalf("sample %d: id 0x%X out of order", i, id)
		}
	}
}
