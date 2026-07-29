// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func rs() *ReliableSender   { return NewReliableSender() }
func rr() *ReliableReceiver { return NewReliableReceiver() }
func pl(b ...byte) []byte   { return append([]byte(nil), b...) }
func u32le(v uint32) []byte { return []byte{byte(v), byte(v >> 8), byte(v >> 16), byte(v >> 24)} }

func TestSubmitAssignsMonotonicSeqnrs(t *testing.T) {
	s := rs()
	s0, st0 := s.Submit(pl(1, 2))
	s1, st1 := s.Submit(pl(3, 4))
	if st0 != SubmitOK || st1 != SubmitOK {
		t.Fatalf("submit status: %v %v", st0, st1)
	}
	if s0 != 0 || s1 != 1 {
		t.Fatalf("seqs: got %d, %d want 0, 1", s0, s1)
	}
	if s.InFlightCount() != 2 {
		t.Fatalf("in-flight count = %d, want 2", s.InFlightCount())
	}
}

func TestSubmitRejectsPayloadTooLarge(t *testing.T) {
	s := rs()
	huge := make([]byte, ReliableMaxPayload+1)
	if _, status := s.Submit(huge); status != SubmitTooLarge {
		t.Fatalf("status = %v, want SubmitTooLarge", status)
	}
}

func TestSubmitRejectsWhenWindowFull(t *testing.T) {
	s := rs()
	for i := 0; i < SenderWindow; i++ {
		if _, status := s.Submit(pl(0)); status != SubmitOK {
			t.Fatalf("submit %d: status = %v", i, status)
		}
	}
	if _, status := s.Submit(pl(0)); status != SubmitWindowFull {
		t.Fatalf("status = %v, want SubmitWindowFull", status)
	}
}

func TestPendingHeartbeatFiresFirstTime(t *testing.T) {
	s := rs()
	s.Submit(pl(1))
	hb, ok := s.PendingHeartbeat(time.Unix(0, 0))
	if !ok {
		t.Fatal("expected a heartbeat")
	}
	if hb.First != 0 || hb.Last != 0 || hb.StreamID != ReliableStreamID {
		t.Fatalf("hb = %+v", hb)
	}
}

func TestPendingHeartbeatSilencedUntilPeriodElapsed(t *testing.T) {
	s := rs()
	s.Submit(pl(1))
	base := time.Unix(0, 0)
	if _, ok := s.PendingHeartbeat(base); !ok {
		t.Fatal("first call must fire")
	}
	if _, ok := s.PendingHeartbeat(base.Add(100 * time.Millisecond)); ok {
		t.Fatal("100ms later must be silenced")
	}
	if _, ok := s.PendingHeartbeat(base.Add(600 * time.Millisecond)); !ok {
		t.Fatal("600ms later must fire")
	}
}

func TestPendingHeartbeatNoneWhenWindowEmpty(t *testing.T) {
	s := rs()
	if _, ok := s.PendingHeartbeat(time.Now()); ok {
		t.Fatal("expected no heartbeat on an empty window")
	}
}

func TestRecvAcknackClearsAckedSeqnrs(t *testing.T) {
	s := rs()
	s.Submit(pl(0xA0)) // seq 0
	s.Submit(pl(0xA1)) // seq 1
	s.Submit(pl(0xA2)) // seq 2
	if s.InFlightCount() != 3 {
		t.Fatalf("in-flight = %d, want 3", s.InFlightCount())
	}
	// base=2, bitmap=0b0001 -> seq 2 missing; 0,1 acknowledged.
	s.RecvAcknack(AckNackBody{FirstUnacked: 2, NackLo: 0x01, NackHi: 0x00, StreamID: ReliableStreamID})
	if s.InFlightCount() != 1 {
		t.Fatalf("in-flight = %d, want 1", s.InFlightCount())
	}
	if _, ok := s.GetInFlight(2); !ok {
		t.Fatal("seq 2 must still be in-flight")
	}
}

func TestRecvAcknackFullClearWhenNoBitsSet(t *testing.T) {
	s := rs()
	for i := 0; i < 5; i++ {
		s.Submit(pl(0))
	}
	s.RecvAcknack(AckNackBody{FirstUnacked: 5, NackLo: 0, NackHi: 0, StreamID: 0x80})
	if s.InFlightCount() != 0 {
		t.Fatalf("in-flight = %d, want 0", s.InFlightCount())
	}
}

func TestRecvDataBuffersInOrder(t *testing.T) {
	r := rr()
	r.RecvData(0, pl(10))
	r.RecvData(1, pl(11))
	drained := r.DrainInOrder()
	if len(drained) != 2 || drained[0].Seq != 0 || drained[1].Seq != 1 {
		t.Fatalf("drained = %+v", drained)
	}
	if r.Expected() != 2 {
		t.Fatalf("expected = %d, want 2", r.Expected())
	}
}

func TestRecvDataReordersOutOfOrder(t *testing.T) {
	r := rr()
	r.RecvData(2, pl(22))
	r.RecvData(0, pl(20))
	d1 := r.DrainInOrder()
	if len(d1) != 1 || d1[0].Seq != 0 {
		t.Fatalf("d1 = %+v", d1)
	}
	r.RecvData(1, pl(21))
	d2 := r.DrainInOrder()
	if len(d2) != 2 || d2[0].Seq != 1 || d2[1].Seq != 2 {
		t.Fatalf("d2 = %+v", d2)
	}
}

func TestRecvDataDropsDuplicates(t *testing.T) {
	r := rr()
	r.RecvData(0, pl(1))
	r.DrainInOrder()
	r.RecvData(0, pl(99)) // already delivered -> silently dropped
	if r.OutOfOrderCount() != 0 {
		t.Fatalf("out-of-order = %d, want 0", r.OutOfOrderCount())
	}
}

func TestRecvDataRejectsWhenBufferFull(t *testing.T) {
	r := rr()
	for i := uint16(1); i <= ReceiverBufferCap; i++ {
		if !r.RecvData(i, pl(1)) {
			t.Fatalf("recv %d unexpectedly rejected", i)
		}
	}
	if r.RecvData(ReceiverBufferCap+1, pl(1)) {
		t.Fatal("expected rejection when the buffer is full")
	}
}

func TestPendingAcknackMarksMissingSlots(t *testing.T) {
	r := rr()
	r.RecvData(1, pl(1))
	r.RecvData(3, pl(3))
	hint := uint16(3)
	ack := r.PendingAcknack(&hint)
	bitmap := uint16(ack.NackLo) | uint16(ack.NackHi)<<8
	if bitmap&(1<<0) == 0 {
		t.Error("seq 0 must be marked missing")
	}
	if bitmap&(1<<2) == 0 {
		t.Error("seq 2 must be marked missing")
	}
	if bitmap&(1<<1) != 0 {
		t.Error("seq 1 must not be marked missing")
	}
	if bitmap&(1<<3) != 0 {
		t.Error("seq 3 must not be marked missing")
	}
}

func TestResetClearsStateCompletely(t *testing.T) {
	s := rs()
	s.Submit(pl(1, 2))
	s.Reset()
	if s.InFlightCount() != 0 {
		t.Fatalf("in-flight after reset = %d", s.InFlightCount())
	}

	r := rr()
	r.RecvData(0, pl(3))
	r.Reset()
	if r.OutOfOrderCount() != 0 || r.Expected() != 0 {
		t.Fatalf("receiver after reset: ooo=%d expected=%d", r.OutOfOrderCount(), r.Expected())
	}
}

// Spec §8.4.14 + §9.2 -- end-to-end sender -> receiver with loss recovery via
// ACKNACK, mirroring `end_to_end_sender_receiver_with_loss_recovery` in
// `crates/xrce/src/reliable.rs`.
func TestEndToEndSenderReceiverWithLossRecovery(t *testing.T) {
	sender := rs()
	receiver := rr()

	s0, _ := sender.Submit(pl(10))
	s1, _ := sender.Submit(pl(11))
	s2, _ := sender.Submit(pl(12))
	if sender.InFlightCount() != 3 {
		t.Fatalf("in-flight = %d, want 3", sender.InFlightCount())
	}

	// Receiver sees s0, s2; s1 lost.
	receiver.RecvData(s0, pl(10))
	receiver.RecvData(s2, pl(12))

	drained := receiver.DrainInOrder()
	if len(drained) != 1 || !bytes.Equal(drained[0].Payload, pl(10)) {
		t.Fatalf("drained = %+v", drained)
	}

	ack := receiver.PendingAcknack(&s2)
	sender.RecvAcknack(ack)
	if _, ok := sender.GetInFlight(s1); !ok {
		t.Fatal("s1 must be retransmittable")
	}

	payload, _ := sender.GetInFlight(s1)
	receiver.RecvData(s1, payload)

	drained2 := receiver.DrainInOrder()
	if len(drained2) != 2 || !bytes.Equal(drained2[0].Payload, pl(11)) || !bytes.Equal(drained2[1].Payload, pl(12)) {
		t.Fatalf("drained2 = %+v", drained2)
	}
}

// Spec §9.2 -- config submessages (CREATE/DELETE/UPDATE) reuse the same
// reliable-stream path; drain_in_order guarantees spec-compliant in-order
// delivery even when samples arrive scrambled.
func TestConfigSubmessagesDeliveredInOrderViaReliableStream(t *testing.T) {
	sender := rs()
	receiver := rr()

	var seqs [5]uint16
	for i := 0; i < 5; i++ {
		seq, status := sender.Submit(pl(byte(i)))
		if status != SubmitOK {
			t.Fatalf("submit %d: %v", i, status)
		}
		seqs[i] = seq
	}

	order := []int{2, 0, 4, 1, 3}
	for _, idx := range order {
		receiver.RecvData(seqs[idx], pl(byte(idx)))
	}

	drained := receiver.DrainInOrder()
	if len(drained) != 5 {
		t.Fatalf("drained = %d, want 5", len(drained))
	}
	for i, d := range drained {
		if !bytes.Equal(d.Payload, pl(byte(i))) {
			t.Fatalf("slot %d: payload %v, want [%d]", i, d.Payload, i)
		}
	}
}

func TestSeqLtGtRfc1982Wraparound(t *testing.T) {
	if !SeqLt(0xFFFF, 0) {
		t.Error("0xFFFF < 0 must hold across the RFC-1982 wraparound")
	}
	if !SeqGt(0, 0xFFFF) {
		t.Error("0 > 0xFFFF must hold across the RFC-1982 wraparound")
	}
	if SeqLt(0, 0xFFFF) {
		t.Error("0 < 0xFFFF must not hold (wraps the other way)")
	}
}

// --- byte-golden: HEARTBEAT + ACKNACK, byte-identical to `crates/xrce` /
// `endpoints/golden-gen` (`golden_heartbeat_le.bin` / `golden_acknack_le.bin`
// for session=0x80, streamHdr=StreamNone, msgSeq=1).

// wantHeartbeatGolden / wantAckNackGolden are the hardcoded expected bytes
// (independent of whether the cargo-generated golden files are available),
// mirroring `endpoints/c/test/test_reliable.c`'s unconditional hardcoded
// assertion.
var wantHeartbeatGolden = []byte{0x80, 0x00, 0x01, 0x00, 0x0B, 0x01, 0x05, 0x00, 0x01, 0x00, 0x03, 0x00, 0x80}
var wantAckNackGolden = []byte{0x80, 0x00, 0x01, 0x00, 0x0A, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80}

func TestByteGoldenHeartbeat(t *testing.T) {
	got := HeartbeatFrame(XrceSessionNoKey, StreamNone, 1, HeartbeatBody{First: 1, Last: 3, StreamID: ReliableStreamID})
	if !bytes.Equal(got, wantHeartbeatGolden) {
		t.Fatalf("HEARTBEAT frame = % X, want % X", got, wantHeartbeatGolden)
	}
	if golden, ok := readGoldenFile("golden_heartbeat_le.bin"); ok && !bytes.Equal(got, golden) {
		t.Fatalf("HEARTBEAT frame != cargo golden: got % X, golden % X", got, golden)
	}
}

func TestByteGoldenAckNack(t *testing.T) {
	got := AckNackFrame(XrceSessionNoKey, StreamNone, 1, AckNackBody{FirstUnacked: 1, NackLo: 0, NackHi: 0, StreamID: ReliableStreamID})
	if !bytes.Equal(got, wantAckNackGolden) {
		t.Fatalf("ACKNACK frame = % X, want % X", got, wantAckNackGolden)
	}
	if golden, ok := readGoldenFile("golden_acknack_le.bin"); ok && !bytes.Equal(got, golden) {
		t.Fatalf("ACKNACK frame != cargo golden: got % X, golden % X", got, golden)
	}
}

// readGoldenFile looks up `name` under goldenDir() (`$GOLDEN_DIR`, default
// "build" -- the same convention `wire_test.go`'s TestByteIdentity uses).
// ok is false when the file is missing (the hardcoded assertion above still
// runs unconditionally).
func readGoldenFile(name string) ([]byte, bool) {
	b, err := os.ReadFile(filepath.Join(goldenDir(), name))
	if err != nil {
		return nil, false
	}
	return b, true
}

func TestFrameRoundTripParsing(t *testing.T) {
	hb := HeartbeatBody{First: 5, Last: 9, StreamID: 0x80}
	f := HeartbeatFrame(0x80, ReliableStreamID, 7, hb)
	got, ok := ParseHeartbeat(f)
	if !ok || got != hb {
		t.Fatalf("ParseHeartbeat round-trip = %+v, ok=%v, want %+v", got, ok, hb)
	}

	an := AckNackBody{FirstUnacked: 5, NackLo: 0x0F, NackHi: 0x00, StreamID: 0x80}
	f2 := AckNackFrame(0x80, ReliableStreamID, 7, an)
	got2, ok2 := ParseAckNack(f2)
	if !ok2 || got2 != an {
		t.Fatalf("ParseAckNack round-trip = %+v, ok=%v, want %+v", got2, ok2, an)
	}

	frame := ReliableWriteFrame(42, u32le(0xCAFEBABE))
	seq, sample, ok3 := ReliableReadFrame(frame)
	if !ok3 || seq != 42 || !bytes.Equal(sample, u32le(0xCAFEBABE)) {
		t.Fatalf("ReliableReadFrame round-trip: seq=%d sample=%v ok=%v", seq, sample, ok3)
	}
}

// --- async-decoupled writer: in-process loopback loss recovery ---

// loopLossyTransport is an in-process Transport with induced loss (drop each
// n-th distinct WRITE_DATA once) that replies ACKNACK from a driven
// ReliableReceiver -- the same protocol shape as the live Rust ReliablePeer,
// without a socket.
type loopLossyTransport struct {
	toReceiver chan []byte
	toSender   chan []byte
	receiver   *ReliableReceiver
	dropEvery  int
	seen       map[uint16]bool
	recvCount  int
	delivered  chan ReliableSample
	stopOnce   chan struct{}
}

func newLoopLossyTransport(dropEvery int) *loopLossyTransport {
	lt := &loopLossyTransport{
		toReceiver: make(chan []byte, 256),
		toSender:   make(chan []byte, 256),
		receiver:   NewReliableReceiver(),
		dropEvery:  dropEvery,
		seen:       make(map[uint16]bool),
		delivered:  make(chan ReliableSample, 256),
		stopOnce:   make(chan struct{}),
	}
	go lt.run()
	return lt
}

func (lt *loopLossyTransport) run() {
	for {
		select {
		case frame, ok := <-lt.toReceiver:
			if !ok {
				return
			}
			if seq, sample, ok := ReliableReadFrame(frame); ok {
				if lt.dropEvery > 0 {
					lt.recvCount++
					if lt.recvCount%lt.dropEvery == 0 && !lt.seen[seq] {
						lt.seen[seq] = true
						continue // simulate loss
					}
				}
				lt.receiver.RecvData(seq, sample)
				for _, s := range lt.receiver.DrainInOrder() {
					lt.delivered <- s
				}
				ack := lt.receiver.PendingAcknack(nil)
				lt.toSender <- AckNackFrame(XrceSessionNoKey, ReliableStreamID, 0, ack)
			} else if hb, ok := ParseHeartbeat(frame); ok {
				_ = hb
				ack := lt.receiver.PendingAcknack(nil)
				lt.toSender <- AckNackFrame(XrceSessionNoKey, ReliableStreamID, 0, ack)
			}
		case <-lt.stopOnce:
			return
		}
	}
}

func (lt *loopLossyTransport) Deliver(frame []byte) error {
	cp := append([]byte(nil), frame...)
	lt.toReceiver <- cp
	return nil
}

func (lt *loopLossyTransport) Receive(buf []byte) (int, bool, error) {
	select {
	case frame := <-lt.toSender:
		return copy(buf, frame), false, nil
	default:
		return 0, true, nil
	}
}

func TestReliableAsyncWriterLossRecovery(t *testing.T) {
	const n = 12
	tr := newLoopLossyTransport(3) // drop every 3rd distinct sample once
	w := NewReliableAsyncWriter(tr, 0)
	for i := 0; i < n; i++ {
		w.Enqueue(u32le(uint32(i)))
	}
	w.Close() // blocks until the whole window is submitted + acknowledged

	got := make([]ReliableSample, 0, n)
	timeout := time.After(10 * time.Second)
drain:
	for len(got) < n {
		select {
		case s := <-tr.delivered:
			got = append(got, s)
		case <-timeout:
			break drain
		}
	}
	if len(got) != n {
		t.Fatalf("delivered %d/%d samples", len(got), n)
	}
	for i, s := range got {
		if s.Seq != uint16(i) {
			t.Fatalf("sample %d: seq = %d, want %d (gap/misorder)", i, s.Seq, i)
		}
		want := u32le(uint32(i))
		if !bytes.Equal(s.Payload, want) {
			t.Fatalf("sample %d: payload = %v, want %v", i, s.Payload, want)
		}
	}
}
