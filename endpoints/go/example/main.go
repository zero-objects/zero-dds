// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable example for the native Go endpoint SDK: encode a sample, send it
// over an in-memory transport, and receive it back both ways — synchronous
// (Client) and asynchronous (goroutine/channel AsyncReader).
//
//	go run ./example

package main

import (
	"fmt"
	"time"

	zerodds "zeroddsendpoint"
)

// A trivial in-memory loopback transport (an integrator supplies a real one).
type loopback struct {
	frames [][]byte
	idx    int
}

func (l *loopback) Deliver(frame []byte) error {
	cp := make([]byte, len(frame))
	copy(cp, frame)
	l.frames = append(l.frames, cp)
	return nil
}

func (l *loopback) Receive(buf []byte) (int, bool, error) {
	if l.idx >= len(l.frames) {
		return 0, true, nil
	}
	n := copy(buf, l.frames[l.idx])
	l.idx++
	return n, false, nil
}

func sample(id uint32, label string) []byte {
	w := zerodds.NewWriter(zerodds.Little)
	w.PutU32(id)
	w.PutString(label)
	return w.Buf
}

func main() {
	// --- synchronous ---
	tr := &loopback{}
	c := zerodds.NewClient(tr)
	_ = c.Write(sample(0x42, "sync-hello"))
	if body, ok, _ := c.Receive(time.Second); ok {
		fmt.Printf("sync: received id=0x%X\n", zerodds.NewReader(body, zerodds.Little).GetU32())
	}

	// --- asynchronous (goroutine + channel) ---
	tr2 := &loopback{}
	w := zerodds.NewAsyncWriter(tr2)
	for i := 0; i < 3; i++ {
		_ = w.Write(sample(0x100+uint32(i), "async"))
	}
	r := zerodds.NewAsyncReader(tr2)
	defer r.Close()
	for i := 0; i < 3; i++ {
		select {
		case body := <-r.Samples:
			fmt.Printf("async: received id=0x%X\n", zerodds.NewReader(body, zerodds.Little).GetU32())
		case <-time.After(2 * time.Second):
			fmt.Println("async: timeout")
			return
		}
	}
	fmt.Println("ALL OK")
}
