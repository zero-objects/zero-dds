// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper SYNC example for the native Go endpoint: a sensor-telemetry publisher
// writes typed Reading samples; a subscriber polls with a timeout and decodes
// every field. Run: `go run ./example_sync`

package main

import (
	"fmt"
	"time"

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

// An in-memory loopback transport (an integrator supplies a real UDP/SHM one).
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
	client := zerodds.NewClient(tr)

	// Publisher: frame + deliver 5 typed readings with varying values.
	for i := 0; i < total; i++ {
		r := Reading{ID: uint32(0x1000 + i), Value: 20.0 + float32(i)*0.5, Label: fmt.Sprintf("bay-%02d", i)}
		if err := client.Write(r.marshal(zerodds.Little)); err != nil {
			panic(err)
		}
	}

	// Subscriber: poll with a deadline; decode every field; stop at `total`.
	deadline := time.Now().Add(2 * time.Second)
	got := 0
	for got < total && time.Now().Before(deadline) {
		body, ok, err := client.Poll()
		if err != nil {
			panic(err)
		}
		if !ok {
			time.Sleep(time.Millisecond)
			continue
		}
		r := decodeReading(body)
		fmt.Printf("sync reading %d: id=0x%x value=%.1f label=%q\n", got, r.ID, r.Value, r.Label)
		got++
	}
	if got != total {
		panic(fmt.Sprintf("timeout: got %d/%d", got, total))
	}
	fmt.Println("ALL OK")
}
