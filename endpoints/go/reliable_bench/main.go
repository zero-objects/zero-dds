// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Producer-latency micro-bench: the decoupled zerodds.ReliableAsyncWriter's
// Enqueue (channel push, never touches the socket) vs. an inline UDP send
// (the same syscall the drain goroutine eventually performs). No live peer is
// required -- frames are fired at an arbitrary loopback port; only the local
// dispatch cost is measured, matching every other endpoint SDK's reliable
// producer-latency bench.
//
// usage: reliable_bench <port>
package main

import (
	"fmt"
	"net"
	"os"
	"strconv"
	"time"

	zerodds "zeroddsendpoint"
)

type udpTransport struct{ conn *net.UDPConn }

func (u *udpTransport) Deliver(frame []byte) error {
	_, err := u.conn.Write(frame)
	return err
}

func (u *udpTransport) Receive(buf []byte) (int, bool, error) {
	_ = u.conn.SetReadDeadline(time.Now().Add(5 * time.Millisecond))
	n, err := u.conn.Read(buf)
	if err != nil {
		if ne, ok := err.(net.Error); ok && ne.Timeout() {
			return 0, true, nil
		}
		return 0, false, err
	}
	return n, false, nil
}

func main() {
	port := 9
	if len(os.Args) > 1 {
		if p, err := strconv.Atoi(os.Args[1]); err == nil {
			port = p
		}
	}
	const iters = 20000
	sample := []byte{0, 0, 0, 0}

	addr, err := net.ResolveUDPAddr("udp", fmt.Sprintf("127.0.0.1:%d", port))
	if err != nil {
		fmt.Fprintf(os.Stderr, "resolve: %v\n", err)
		os.Exit(1)
	}
	conn, err := net.DialUDP("udp", nil, addr)
	if err != nil {
		fmt.Fprintf(os.Stderr, "dial: %v\n", err)
		os.Exit(1)
	}
	defer conn.Close()

	// Inline: the caller does the send syscall itself, every time.
	t0 := time.Now()
	for i := 0; i < iters; i++ {
		frame := zerodds.ReliableWriteFrame(uint16(i), sample)
		_, _ = conn.Write(frame)
	}
	inlineNs := time.Since(t0).Nanoseconds() / int64(iters)

	// Decoupled: the producer only enqueues; a drain goroutine owns the socket.
	tr := &udpTransport{conn: conn}
	w := zerodds.NewReliableAsyncWriter(tr, iters)
	t1 := time.Now()
	for i := 0; i < iters; i++ {
		w.Enqueue(sample)
	}
	decoupledNs := time.Since(t1).Nanoseconds() / int64(iters)
	// No live peer acknowledges these (an arbitrary port), so the send window
	// never drains -- don't block on Close(); the process exit reclaims the
	// drain goroutine. Only Enqueue's return latency is under measurement here.
	_ = w

	fmt.Printf("BENCH decoupled_ns=%d inline_ns=%d\n", decoupledNs, inlineNs)
}
