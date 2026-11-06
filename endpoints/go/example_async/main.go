// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper ASYNC example for the native Go endpoint: the same telemetry publisher,
// but the subscriber consumes via the goroutine/channel AsyncReader and decodes
// every field. Run: `go run ./example_async`

package main

import (
	"fmt"
	"os"
	"time"

	zerodds "zeroddsendpoint"
)

// How long the subscriber waits for the samples; a missing one ends the run
// with an error instead of an endless wait.
const timeout = 10 * time.Second

// Reading mirrors an IDL `@final struct Reading { uint32 id; float value; string label; }`.
type Reading struct {
	ID    uint32
	Value float32
	Label string
}

func (r Reading) marshal(endian zerodds.Endianness) []byte {
	w := zerodds.NewWriter(endian, zerodds.Xcdr2)
	w.PutU32(r.ID)
	w.PutF32(r.Value)
	w.PutString(r.Label)
	return w.Buf
}

func decodeReading(body []byte) Reading {
	rd := zerodds.NewReader(body, zerodds.Little, zerodds.Xcdr2)
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
	writer := zerodds.NewAsyncWriter(tr, zerodds.XrceObjectID(0x001, uint16(zerodds.ObjkDataWriter)))

	// Publisher.
	for i := 0; i < total; i++ {
		r := Reading{ID: uint32(0x2000 + i), Value: 100.0 - float32(i), Label: fmt.Sprintf("sensor-%02d", i)}
		if err := writer.Write(r.marshal(zerodds.Little)); err != nil {
			panic(err)
		}
	}

	// Subscriber: receive from the Samples channel; decode each; graceful stop
	// at `total`, an error when the samples do not arrive within `timeout`.
	reader := zerodds.NewAsyncReader(tr)
	defer reader.Close()
	deadline := time.After(timeout)
	for got := 0; got < total; got++ {
		select {
		case body, open := <-reader.Samples:
			if !open {
				fmt.Fprintf(os.Stderr, "incomplete: the reader stopped after %d of %d samples\n", got, total)
				os.Exit(1)
			}
			r := decodeReading(body)
			fmt.Printf("async reading %d: id=0x%x value=%.1f label=%q\n", got, r.ID, r.Value, r.Label)
		case <-deadline:
			fmt.Fprintf(os.Stderr, "incomplete: got %d of %d samples within %v\n", got, total, timeout)
			os.Exit(1)
		}
	}
	fmt.Println("ALL OK")
}
