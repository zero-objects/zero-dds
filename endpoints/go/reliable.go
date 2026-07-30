// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// Reliable XRCE stream (spec §8.4.10/§8.4.11) for the native Go endpoint SDK.
// Self-contained: the sender/receiver state machines + the WRITE_DATA /
// HEARTBEAT / ACKNACK frame codec, byte-identical to `crates/xrce`
// (`crates/xrce/src/reliable.rs`) and every other endpoint SDK
// (`endpoints/c`, `endpoints/nim`, `endpoints/zig`, ...).
//
// Frame layout (8-byte header + body, all little-endian):
//
//	[0]=session [1]=stream [2..4]=seq LE [4]=submessage id [5]=flags
//	[6..8]=body-len LE [8..]=body
//	WRITE_DATA id=0x07 flags=0x03 body=sample; header seq = the RFC-1982
//	sample sequence number.
//	HEARTBEAT  id=0x0B flags=0x01 body(5)= first i16 LE, last i16 LE, stream
//	ACKNACK    id=0x0A flags=0x01 body(5)= first_unacked i16 LE, nack[0],
//	nack[1], stream
//
// The message-header `stream` byte for HEARTBEAT/ACKNACK is parameterized:
// the true spec encoding (and the byte-goldens) uses `StreamNone` (0x00,
// spec §8.3.5.11/12); this harness's live wire path uses `ReliableStreamID`
// (0x80) for convenience, matching `endpoints/c` and `endpoints/nim` — the
// receiving side never inspects that byte (only `frame[4]` + the body at
// offset 8), so both are wire-safe.
package zerodds

import "time"

// Reliable-stream wire + protocol constants (spec §8.4.10/§8.4.11).
const (
	// ReliableStreamID is the reliable stream id (bit 7 set, RFC-1982 window).
	ReliableStreamID byte = 0x80
	// StreamNone is the control-submessage stream id (spec §8.3.5.11/12).
	StreamNone byte = 0x00

	xrceSMAckNack   byte = 0x0A
	xrceSMHeartbeat byte = 0x0B
	// xrceFlagELE is the E-flag (little-endian) only, control-frame flags.
	xrceFlagELE byte = 0x01

	// SenderWindow is the sender window cap: 16 in-flight samples (matches the
	// 16-bit ACKNACK nack bitmap).
	SenderWindow = 16
	// ReceiverBufferCap is the receiver out-of-order buffer cap (DoS bound).
	ReceiverBufferCap = 64
	// ReliableMaxPayload is the per-sample payload cap (u16 submessage length).
	ReliableMaxPayload = 65535
	// HeartbeatPeriod is the default HEARTBEAT period (spec recommends 100ms;
	// conservatively 500ms here, matching every other endpoint SDK).
	HeartbeatPeriod = 500 * time.Millisecond
)

// --- RFC-1982 16-bit serial-number comparison ---

// SeqLt reports whether a < b in RFC-1982 order (half-window = 32768).
func SeqLt(a, b uint16) bool {
	d := uint32(b-a) & 0xffff
	return d != 0 && d < 0x8000
}

// SeqGt reports whether a > b in RFC-1982 order.
func SeqGt(a, b uint16) bool { return SeqLt(b, a) }

// --- frame codec ---

// ReliableWriteFrame builds a reliable WRITE_DATA frame. The 2-byte header
// sequence carries the RFC-1982 sample sequence number. Byte-identical to
// `XrceWriteFrame(0x80, ReliableStreamID, seq, sample)`.
func ReliableWriteFrame(seq uint16, sample []byte) []byte {
	return XrceWriteFrame(XrceSessionNoKey, ReliableStreamID, seq, sample)
}

// ReliableReadFrame parses a reliable WRITE_DATA frame into (seq, sample).
func ReliableReadFrame(frame []byte) (seq uint16, sample []byte, ok bool) {
	if len(frame) < 8 || frame[4] != xrceSMWriteData {
		return 0, nil, false
	}
	return uint16(frame[2]) | uint16(frame[3])<<8, frame[8:], true
}

// HeartbeatBody is the 5-byte HEARTBEAT body (first/last unacked seq, stream).
type HeartbeatBody struct {
	First, Last uint16
	StreamID    byte
}

// AckNackBody is the 5-byte ACKNACK body (first-unacked seq, 16-bit nack
// bitmap, stream).
type AckNackBody struct {
	FirstUnacked   uint16
	NackLo, NackHi byte
	StreamID       byte
}

// HeartbeatFrame builds a HEARTBEAT frame. `streamHdr` is the message-header
// stream byte (StreamNone for the byte-golden layout, ReliableStreamID on the
// live wire path). Byte-identical to `golden_heartbeat_le.bin` for
// (session=0x80, streamHdr=StreamNone, msgSeq=1, {first:1, last:3, stream:0x80}).
func HeartbeatFrame(session, streamHdr byte, msgSeq uint16, hb HeartbeatBody) []byte {
	return []byte{
		session, streamHdr, byte(msgSeq), byte(msgSeq >> 8),
		xrceSMHeartbeat, xrceFlagELE, 5, 0,
		byte(hb.First), byte(hb.First >> 8),
		byte(hb.Last), byte(hb.Last >> 8),
		hb.StreamID,
	}
}

// AckNackFrame builds an ACKNACK frame. `streamHdr` is the message-header
// stream byte (StreamNone for the byte-golden layout, ReliableStreamID on the
// live wire path). Byte-identical to `golden_acknack_le.bin` for
// (session=0x80, streamHdr=StreamNone, msgSeq=1, {firstUnacked:1, nack:0, stream:0x80}).
func AckNackFrame(session, streamHdr byte, msgSeq uint16, ack AckNackBody) []byte {
	return []byte{
		session, streamHdr, byte(msgSeq), byte(msgSeq >> 8),
		xrceSMAckNack, xrceFlagELE, 5, 0,
		byte(ack.FirstUnacked), byte(ack.FirstUnacked >> 8),
		ack.NackLo, ack.NackHi,
		ack.StreamID,
	}
}

// ParseHeartbeat parses a HEARTBEAT frame; ok is false if too short or not a
// HEARTBEAT submessage.
func ParseHeartbeat(frame []byte) (HeartbeatBody, bool) {
	if len(frame) < 13 || frame[4] != xrceSMHeartbeat {
		return HeartbeatBody{}, false
	}
	return HeartbeatBody{
		First:    uint16(frame[8]) | uint16(frame[9])<<8,
		Last:     uint16(frame[10]) | uint16(frame[11])<<8,
		StreamID: frame[12],
	}, true
}

// ParseAckNack parses an ACKNACK frame; ok is false if too short or not an
// ACKNACK submessage.
func ParseAckNack(frame []byte) (AckNackBody, bool) {
	if len(frame) < 13 || frame[4] != xrceSMAckNack {
		return AckNackBody{}, false
	}
	return AckNackBody{
		FirstUnacked: uint16(frame[8]) | uint16(frame[9])<<8,
		NackLo:       frame[10],
		NackHi:       frame[11],
		StreamID:     frame[12],
	}, true
}

// --- sender ---

// SubmitStatus is the result of ReliableSender.Submit.
type SubmitStatus int

const (
	// SubmitOK: the sample was buffered and assigned a sequence number.
	SubmitOK SubmitStatus = iota
	// SubmitTooLarge: payload exceeds ReliableMaxPayload.
	SubmitTooLarge
	// SubmitWindowFull: SenderWindow in-flight samples already buffered; the
	// caller must process ACKNACKs (or wait for drain) first.
	SubmitWindowFull
)

// ReliableSender is the sender half of a reliable stream (mirrors
// `ReliableStreamState`'s sender side in `crates/xrce`).
type ReliableSender struct {
	nextSeq       uint16
	inFlight      map[uint16][]byte
	lastHeartbeat time.Time
	hbStarted     bool
}

// NewReliableSender returns a fresh sender (next sequence 0, empty window).
func NewReliableSender() *ReliableSender {
	return &ReliableSender{inFlight: make(map[uint16][]byte)}
}

// InFlightCount is the number of unacknowledged in-flight samples.
func (s *ReliableSender) InFlightCount() int { return len(s.inFlight) }

// Submit buffers a new sample and assigns it the next RFC-1982 sequence
// number. Returns SubmitWindowFull if SenderWindow samples are already
// in-flight, or SubmitTooLarge if payload exceeds ReliableMaxPayload.
func (s *ReliableSender) Submit(payload []byte) (uint16, SubmitStatus) {
	if len(payload) > ReliableMaxPayload {
		return 0, SubmitTooLarge
	}
	if len(s.inFlight) >= SenderWindow {
		return 0, SubmitWindowFull
	}
	seq := s.nextSeq
	s.inFlight[seq] = payload
	s.nextSeq++
	return seq, SubmitOK
}

// GetInFlight looks up an in-flight payload (e.g. for retransmit).
func (s *ReliableSender) GetInFlight(seq uint16) ([]byte, bool) {
	p, ok := s.inFlight[seq]
	return p, ok
}

// serialWindow returns the RFC-1982 oldest (window base) and newest in-flight
// sequence numbers. Unlike the numeric map min/max, these are correct across a
// 16-bit wrap: for the window 0xFFFE,0xFFFF,0x0000,0x0001 the RFC-1982 order is
// 0xFFFE < 0xFFFF < 0x0000 < 0x0001, so base=0xFFFE / end=0x0001 (the numeric
// path would report base=0x0000 / end=0xFFFF). Mirrors window_base /
// serial_max_in_flight in crates/xrce/src/reliable.rs.
func (s *ReliableSender) serialWindow() (first, last uint16) {
	seen := false
	for k := range s.inFlight {
		if !seen {
			first, last, seen = k, k, true
			continue
		}
		if SeqLt(k, first) {
			first = k
		}
		if SeqGt(k, last) {
			last = k
		}
	}
	return first, last
}

// InFlightSeqs returns the currently in-flight sequence numbers in numeric
// (map key) order — a deterministic retransmit order. NOT the HEARTBEAT window:
// the window base/end come from serialWindow (RFC-1982), which is wrap-correct.
func (s *ReliableSender) InFlightSeqs() []uint16 {
	out := make([]uint16, 0, len(s.inFlight))
	for k := range s.inFlight {
		out = append(out, k)
	}
	for i := 1; i < len(out); i++ {
		for j := i; j > 0 && out[j-1] > out[j]; j-- {
			out[j-1], out[j] = out[j], out[j-1]
		}
	}
	return out
}

// PendingHeartbeat returns (HEARTBEAT, true) if the heartbeat period has
// elapsed and in-flight samples exist (fires on the first call, then every
// HeartbeatPeriod); (zero, false) otherwise.
func (s *ReliableSender) PendingHeartbeat(now time.Time) (HeartbeatBody, bool) {
	if len(s.inFlight) == 0 {
		return HeartbeatBody{}, false
	}
	due := !s.hbStarted || now.Sub(s.lastHeartbeat) >= HeartbeatPeriod
	if !due {
		return HeartbeatBody{}, false
	}
	s.hbStarted = true
	s.lastHeartbeat = now
	// Window base = RFC-1982 oldest unacked, end = RFC-1982 newest unacked
	// (wrap-correct); never the numeric map min/max.
	first, last := s.serialWindow()
	return HeartbeatBody{First: first, Last: last, StreamID: ReliableStreamID}, true
}

// RecvAcknack processes an incoming ACKNACK on the sender side: everything
// strictly before `FirstUnacked` (RFC-1982 order) is acknowledged and
// dropped; within [FirstUnacked, FirstUnacked+16) a clear bit means acked
// (drop), a set bit means still missing (keep for retransmit).
func (s *ReliableSender) RecvAcknack(ack AckNackBody) {
	base := ack.FirstUnacked
	bitmap := uint16(ack.NackLo) | uint16(ack.NackHi)<<8

	for k := range s.inFlight {
		diff := uint32(base-k) & 0xffff
		if diff != 0 && diff < 0x8000 {
			delete(s.inFlight, k)
		}
	}
	for i := uint16(0); i < 16; i++ {
		seq := base + i
		if (bitmap>>i)&1 == 0 {
			delete(s.inFlight, seq)
		}
	}
}

// Reset clears the sender state (next sequence 0, empty window).
func (s *ReliableSender) Reset() {
	s.nextSeq = 0
	s.inFlight = make(map[uint16][]byte)
	s.hbStarted = false
}

// --- receiver ---

// ReliableSample is one delivered (seq, payload) pair.
type ReliableSample struct {
	Seq     uint16
	Payload []byte
}

// ReliableReceiver is the receiver half of a reliable stream (mirrors
// `ReliableStreamState`'s receiver side in `crates/xrce`).
type ReliableReceiver struct {
	expected uint16
	received map[uint16][]byte
}

// NewReliableReceiver returns a fresh receiver (expects sequence 0).
func NewReliableReceiver() *ReliableReceiver {
	return &ReliableReceiver{received: make(map[uint16][]byte)}
}

// Expected is the next sequence number the receiver expects to deliver.
func (r *ReliableReceiver) Expected() uint16 { return r.expected }

// OutOfOrderCount is the number of out-of-order samples currently buffered.
func (r *ReliableReceiver) OutOfOrderCount() int { return len(r.received) }

// RecvData buffers an incoming sample. Duplicates (seq before Expected, or
// already buffered) are silently dropped (returns true, a no-op). Returns
// false only when the ReceiverBufferCap out-of-order buffer is full (DoS
// bound) and the caller must not add `payload`.
func (r *ReliableReceiver) RecvData(seq uint16, payload []byte) bool {
	if SeqLt(seq, r.expected) {
		return true // already delivered → drop
	}
	if _, ok := r.received[seq]; ok {
		return true // already buffered
	}
	if len(r.received) >= ReceiverBufferCap {
		return false
	}
	r.received[seq] = payload
	return true
}

// DrainInOrder returns all contiguously available samples starting at
// Expected, in order, and advances Expected past them.
func (r *ReliableReceiver) DrainInOrder() []ReliableSample {
	var out []ReliableSample
	for {
		p, ok := r.received[r.expected]
		if !ok {
			break
		}
		out = append(out, ReliableSample{Seq: r.expected, Payload: p})
		delete(r.received, r.expected)
		r.expected++
	}
	return out
}

// PendingAcknack computes the ACKNACK marking the missing slots in
// [Expected, Expected+16). If hintLastSeen is non-nil, slots strictly after
// it are treated as not-missing (a HEARTBEAT hint bounds the window); nil
// marks every not-yet-received slot missing (unambiguous "clear bit = received").
func (r *ReliableReceiver) PendingAcknack(hintLastSeen *uint16) AckNackBody {
	base := r.expected
	var bitmap uint16
	for i := uint16(0); i < 16; i++ {
		seq := base + i
		if hintLastSeen != nil && SeqGt(seq, *hintLastSeen) {
			continue
		}
		if _, ok := r.received[seq]; !ok {
			bitmap |= 1 << i
		}
	}
	return AckNackBody{
		FirstUnacked: base,
		NackLo:       byte(bitmap),
		NackHi:       byte(bitmap >> 8),
		StreamID:     ReliableStreamID,
	}
}

// Reset clears the receiver state (expects sequence 0 again).
func (r *ReliableReceiver) Reset() {
	r.expected = 0
	r.received = make(map[uint16][]byte)
}

// --- async-decoupled writer ---

// ReliableAsyncWriter is the idiomatic Go async-decoupled reliable sender: the
// producer only enqueues payloads onto a buffered channel (never touches the
// Transport / socket); a single drain goroutine owns the ReliableSender state
// + the Transport, submitting, sending WRITE_DATA, emitting periodic
// HEARTBEATs, draining ACKNACKs and retransmitting, until every enqueued
// sample has been submitted AND acknowledged.
type ReliableAsyncWriter struct {
	in   chan []byte
	done chan struct{}
}

// NewReliableAsyncWriter spawns the drain goroutine over t and returns a
// writer ready for Enqueue. `capacity` sizes the producer→drain channel (0
// uses a sensible default).
func NewReliableAsyncWriter(t Transport, capacity int) *ReliableAsyncWriter {
	if capacity <= 0 {
		capacity = 1024
	}
	w := &ReliableAsyncWriter{
		in:   make(chan []byte, capacity),
		done: make(chan struct{}),
	}
	go w.drain(t)
	return w
}

// Enqueue hands one payload to the drain goroutine. Never touches the
// Transport — the call returns as soon as the channel accepts the payload (it
// only blocks if the channel buffer is momentarily full, never on I/O).
func (w *ReliableAsyncWriter) Enqueue(payload []byte) { w.in <- payload }

// Close signals that no more samples will be enqueued, then blocks until the
// drain goroutine has submitted and fully drained (acknowledged) every
// enqueued sample.
func (w *ReliableAsyncWriter) Close() {
	close(w.in)
	<-w.done
}

func (w *ReliableAsyncWriter) drain(t Transport) {
	defer close(w.done)

	sender := NewReliableSender()
	var pending [][]byte
	inputClosed := false
	msgSeq := uint16(1)
	buf := make([]byte, 2048)
	ticker := time.NewTicker(2 * time.Millisecond)
	defer ticker.Stop()

	pullAvailable := func() {
		for {
			select {
			case payload, ok := <-w.in:
				if !ok {
					inputClosed = true
					return
				}
				pending = append(pending, payload)
			default:
				return
			}
		}
	}

	for {
		if !inputClosed {
			select {
			case payload, ok := <-w.in:
				if !ok {
					inputClosed = true
				} else {
					pending = append(pending, payload)
				}
			case <-ticker.C:
			}
		} else {
			<-ticker.C
		}
		pullAvailable()

		// Submit as many pending payloads as the window allows, sending
		// WRITE_DATA for each newly-submitted sample.
		for len(pending) > 0 {
			seq, status := sender.Submit(pending[0])
			if status != SubmitOK {
				break
			}
			_ = t.Deliver(ReliableWriteFrame(seq, pending[0]))
			pending = pending[1:]
		}

		// Period-gated HEARTBEAT.
		if hb, ok := sender.PendingHeartbeat(time.Now()); ok {
			_ = t.Deliver(HeartbeatFrame(XrceSessionNoKey, ReliableStreamID, msgSeq, hb))
			msgSeq++
		}

		// Drain incoming ACKNACKs (bounded, non-blocking per Transport contract).
		for i := 0; i < 64; i++ {
			n, again, err := t.Receive(buf)
			if err != nil || again {
				break
			}
			ack, ok := ParseAckNack(buf[:n])
			if !ok {
				continue
			}
			sender.RecvAcknack(ack)
		}

		// Retransmit whatever remains in-flight (the peer's last ACKNACK marked
		// it missing, or it has not been acked yet).
		for _, seq := range sender.InFlightSeqs() {
			if payload, ok := sender.GetInFlight(seq); ok {
				_ = t.Deliver(ReliableWriteFrame(seq, payload))
			}
		}

		if inputClosed && len(pending) == 0 && sender.InFlightCount() == 0 {
			return
		}
	}
}
