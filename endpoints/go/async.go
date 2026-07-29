// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

import "time"

// Transport is the single integration point: the endpoint hands framed messages
// to it and receives frames back. The integrator implements it for their link.
type Transport interface {
	// Deliver sends one complete frame to the peer.
	Deliver(frame []byte) error
	// Receive reads one frame into buf; again is true when none is ready.
	Receive(buf []byte) (n int, again bool, err error)
}

// AsyncReader is the idiomatic Go async model: a goroutine polls the transport
// and pushes decoded sample bodies onto the Samples channel. Range over Samples;
// call Close to stop.
type AsyncReader struct {
	Samples chan []byte
	done    chan struct{}
}

// NewAsyncReader spawns the receive goroutine over t.
func NewAsyncReader(t Transport) *AsyncReader {
	r := &AsyncReader{
		Samples: make(chan []byte, 64),
		done:    make(chan struct{}),
	}
	go r.loop(t)
	return r
}

func (r *AsyncReader) loop(t Transport) {
	defer close(r.Samples)
	buf := make([]byte, 2048)
	for {
		select {
		case <-r.done:
			return
		default:
		}
		n, again, err := t.Receive(buf)
		if err != nil {
			return
		}
		if again {
			time.Sleep(time.Millisecond)
			continue
		}
		if body, ok := XrceReadFrame(buf[:n]); ok {
			cp := make([]byte, len(body))
			copy(cp, body)
			r.Samples <- cp
		}
	}
}

// Close stops the receive goroutine (Samples is then drained + closed).
func (r *AsyncReader) Close() { close(r.done) }

// AsyncWriter frames XCDR samples as XRCE WRITE_DATA and delivers them.
type AsyncWriter struct {
	Transport Transport
	Session   byte
	Stream    byte
	seq       uint16
}

// NewAsyncWriter returns a writer over t with a best-effort no-key session.
func NewAsyncWriter(t Transport) *AsyncWriter {
	return &AsyncWriter{
		Transport: t,
		Session:   XrceSessionNoKey,
		Stream:    XrceStreamBestEffort,
		seq:       1,
	}
}

// Write frames and delivers one sample, advancing the sequence.
func (w *AsyncWriter) Write(sample []byte) error {
	frame := XrceWriteFrame(w.Session, w.Stream, w.seq, sample)
	w.seq++
	return w.Transport.Deliver(frame)
}
