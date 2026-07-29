// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable reliable-stream demo (in-process, no sockets): an aggregator
// submits N samples over a lossy link (every 3rd first-delivery dropped); the
// receiver recovers the gaps via ACKNACK + retransmit and prints the
// contiguous, gap-free sequence it actually delivered. Run: `go run ./example_reliable`
package main

import (
	"fmt"
	"strings"

	zerodds "zeroddsendpoint"
)

func main() {
	const n = 12
	sender := zerodds.NewReliableSender()
	receiver := zerodds.NewReliableReceiver()

	for i := 0; i < n; i++ {
		v := uint32(i)
		payload := []byte{byte(v), byte(v >> 8), byte(v >> 16), byte(v >> 24)}
		if _, status := sender.Submit(payload); status != zerodds.SubmitOK {
			panic(fmt.Sprintf("submit %d failed: %v", i, status))
		}
	}

	var delivered []uint32
	droppedOnce := map[uint16]bool{}
	pass := 0
	for len(delivered) < n && pass < 100 {
		seqs := sender.InFlightSeqs()
		for idx, seq := range seqs {
			if pass == 0 && (idx+1)%3 == 0 && !droppedOnce[seq] {
				droppedOnce[seq] = true // simulate loss on the first pass
				continue
			}
			payload, ok := sender.GetInFlight(seq)
			if !ok {
				continue
			}
			receiver.RecvData(seq, payload)
		}
		for _, s := range receiver.DrainInOrder() {
			v := uint32(s.Payload[0]) | uint32(s.Payload[1])<<8 | uint32(s.Payload[2])<<16 | uint32(s.Payload[3])<<24
			delivered = append(delivered, v)
		}
		ack := receiver.PendingAcknack(nil)
		sender.RecvAcknack(ack)
		pass++
	}

	parts := make([]string, len(delivered))
	for i, v := range delivered {
		parts[i] = fmt.Sprintf("%d", v)
	}
	fmt.Printf("delivered: %s\n", strings.Join(parts, " "))
	fmt.Printf("dropped on first pass: %d, recovered after %d passes\n", len(droppedOnce), pass)

	ok := len(delivered) == n
	for i := 0; i < n && ok; i++ {
		if delivered[i] != uint32(i) {
			ok = false
		}
	}
	if ok {
		fmt.Printf("sequence 0..%d verified in order\n", n-1)
		fmt.Println("RELIABLE OK")
	} else {
		fmt.Println("RELIABLE FAIL")
	}
}
