// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

import (
	"net"
	"testing"
	"time"
)

// An in-memory transport: frames are produced before the reader starts.
type memTransport struct {
	frames [][]byte
	idx    int
}

func (m *memTransport) Deliver(frame []byte) error {
	cp := make([]byte, len(frame))
	copy(cp, frame)
	m.frames = append(m.frames, cp)
	return nil
}

func (m *memTransport) Receive(buf []byte) (int, bool, error) {
	if m.idx >= len(m.frames) {
		return 0, true, nil
	}
	n := copy(buf, m.frames[m.idx])
	m.idx++
	return n, false, nil
}

func sampleBody(id uint32) []byte {
	w := NewWriter(Little)
	w.PutU32(id)
	w.PutU16(0)
	w.PutU8(0)
	return w.Buf
}

func TestAsyncLoopback(t *testing.T) {
	const N = 5
	tr := &memTransport{}
	w := NewAsyncWriter(tr)
	for i := 0; i < N; i++ {
		if err := w.Write(sampleBody(0x1000 + uint32(i))); err != nil {
			t.Fatalf("write %d: %v", i, err)
		}
	}

	r := NewAsyncReader(tr)
	defer r.Close()

	var got []uint32
	timeout := time.After(5 * time.Second)
	for len(got) < N {
		select {
		case body := <-r.Samples:
			got = append(got, NewReader(body, Little).GetU32())
		case <-timeout:
			t.Fatalf("timeout, got %d/%d", len(got), N)
		}
	}
	for i := 0; i < N; i++ {
		if got[i] != 0x1000+uint32(i) {
			t.Fatalf("sample %d: id 0x%X out of order", i, got[i])
		}
	}
}

// floodTransport always yields the same valid frame and never reports `again`,
// so the reader produces continuously and fills the 64-slot Samples buffer.
type floodTransport struct{ frame []byte }

func (f *floodTransport) Deliver(frame []byte) error { return nil }

func (f *floodTransport) Receive(buf []byte) (int, bool, error) {
	return copy(buf, f.frame), false, nil
}

// TestAsyncCloseUnblocksFullChannel pins the fix for the full-channel Close hang:
// with no consumer the send goroutine blocks on a full Samples buffer, and Close
// must still reach it. The bug (a bare `Samples <- cp`) left the goroutine wedged
// so close(done) never landed and the goroutine leaked.
func TestAsyncCloseUnblocksFullChannel(t *testing.T) {
	body := sampleBody(0x9000)
	frame := XrceWriteFrame(XrceSessionNoKey, XrceStreamBestEffort, 1, body)
	frame[4] = xrceSMData // hub -> endpoint DATA (0x09)
	tr := &floodTransport{frame: frame}

	r := NewAsyncReader(tr)
	// Deliberately do NOT read Samples: let the 64-slot buffer fill so the send
	// goroutine is parked on a blocked send when Close arrives.
	time.Sleep(50 * time.Millisecond)

	r.Close()

	// A terminating goroutine runs its deferred close(Samples), which ends this
	// range; a wedged goroutine (the bug) leaves Samples open and times out.
	drained := make(chan struct{})
	go func() {
		for range r.Samples {
		}
		close(drained)
	}()
	select {
	case <-drained:
	case <-time.After(2 * time.Second):
		t.Fatal("Close did not terminate the send goroutine blocked on a full channel")
	}
}

// A live non-blocking UDP transport.
type udpTransport struct {
	conn *net.UDPConn
	peer *net.UDPAddr
}

func (u *udpTransport) Deliver(frame []byte) error {
	_, err := u.conn.WriteToUDP(frame, u.peer)
	return err
}

func (u *udpTransport) Receive(buf []byte) (int, bool, error) {
	_ = u.conn.SetReadDeadline(time.Now().Add(10 * time.Millisecond))
	n, _, err := u.conn.ReadFromUDP(buf)
	if err != nil {
		if ne, ok := err.(net.Error); ok && ne.Timeout() {
			return 0, true, nil
		}
		return 0, false, err
	}
	return n, false, nil
}

func TestAsyncUDP(t *testing.T) {
	const N = 5
	addr, _ := net.ResolveUDPAddr("udp", "127.0.0.1:0")
	conn, err := net.ListenUDP("udp", addr)
	if err != nil {
		t.Fatalf("listen: %v", err)
	}
	defer conn.Close()

	tr := &udpTransport{conn: conn, peer: conn.LocalAddr().(*net.UDPAddr)}
	w := NewAsyncWriter(tr)
	for i := 0; i < N; i++ {
		if err := w.Write(sampleBody(0x2000 + uint32(i))); err != nil {
			t.Fatalf("write %d: %v", i, err)
		}
	}

	r := NewAsyncReader(tr)
	defer r.Close()

	got := 0
	timeout := time.After(5 * time.Second)
	for got < N {
		select {
		case <-r.Samples:
			got++
		case <-timeout:
			t.Fatalf("UDP timeout, got %d/%d", got, N)
		}
	}
}
