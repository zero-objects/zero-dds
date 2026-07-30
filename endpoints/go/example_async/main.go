// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper ASYNC example for the native Go endpoint: the same telemetry publisher,
// but the subscriber consumes via the goroutine/channel AsyncReader and decodes
// every field. Run: `go run ./example_async`

package main

import (
	"fmt"

	zerodds "zeroddsendpoint"
)

// Reading mirrors an IDL `@final struct Reading { uint32 id; float value; string label; }`.
type Reading struct {
	ID    uint32
	Value float32
	Label string
}

func (r Reading) marshal(endian zerodds.Endianness) []byte {
	w := zerodds.NewWriter(endian)
	w.PutU32(r.ID)
	w.PutF32(r.Value)
	w.PutString(r.Label)
	return w.Buf
}

func decodeReading(body []byte) Reading {
	rd := zerodds.NewReader(body, zerodds.Little)
	return Reading{ID: rd.GetU32(), Value: rd.GetF32(), Label: rd.GetString()}
}

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

func main() {
	const total = 5
	tr := &loopback{}
	writer := zerodds.NewAsyncWriter(tr)

	// Publisher.
	for i := 0; i < total; i++ {
		r := Reading{ID: uint32(0x2000 + i), Value: 100.0 - float32(i), Label: fmt.Sprintf("sensor-%02d", i)}
		if err := writer.Write(r.marshal(zerodds.Little)); err != nil {
			panic(err)
		}
	}

	// Subscriber: range the Samples channel; decode each; graceful stop at `total`.
	reader := zerodds.NewAsyncReader(tr)
	got := 0
	for body := range reader.Samples {
		r := decodeReading(body)
		fmt.Printf("async reading %d: id=0x%x value=%.1f label=%q\n", got, r.ID, r.Value, r.Label)
		got++
		if got == total {
			reader.Close()
			break
		}
	}
	if got != total {
		panic(fmt.Sprintf("got %d/%d", got, total))
	}
	fmt.Println("ALL OK")
}
