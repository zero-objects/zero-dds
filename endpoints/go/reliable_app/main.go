// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Reliable-stream UDP sender app for the live E2E (crates/endpoint-e2e). Plays
// the reliable SENDER against the shared Rust ReliablePeer, via the
// async-decoupled zerodds.ReliableAsyncWriter: the producer enqueues N typed
// samples (never touching the socket); the writer's drain goroutine submits,
// sends WRITE_DATA, emits periodic HEARTBEATs, drains ACKNACKs, and
// retransmits until the whole send window has been acknowledged. Never exits
// before the window empties.
//
// usage: reliable_app <peer-port> <N>
package main

import (
	"fmt"
	"net"
	"os"
	"strconv"
	"time"

	zerodds "zeroddsendpoint"
)

// udpTransport is a live, connected, non-blocking UDP zerodds.Transport.
type udpTransport struct {
	conn *net.UDPConn
}

func (u *udpTransport) Deliver(frame []byte) error {
	_, err := u.conn.Write(frame)
	return err
}

func (u *udpTransport) Receive(buf []byte) (int, bool, error) {
	_ = u.conn.SetReadDeadline(time.Now().Add(20 * time.Millisecond))
	n, err := u.conn.Read(buf)
	if err != nil {
		if ne, ok := err.(net.Error); ok && ne.Timeout() {
			return 0, true, nil
		}
		return 0, false, err
	}
	return n, false, nil
}

func u32le(v uint32) []byte {
	return []byte{byte(v), byte(v >> 8), byte(v >> 16), byte(v >> 24)}
}

func main() {
	if len(os.Args) < 3 {
		fmt.Fprintln(os.Stderr, "usage: reliable_app <peer-port> <N>")
		os.Exit(2)
	}
	port, err := strconv.Atoi(os.Args[1])
	if err != nil {
		fmt.Fprintf(os.Stderr, "bad port %q: %v\n", os.Args[1], err)
		os.Exit(2)
	}
	n, err := strconv.Atoi(os.Args[2])
	if err != nil {
		fmt.Fprintf(os.Stderr, "bad N %q: %v\n", os.Args[2], err)
		os.Exit(2)
	}

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

	tr := &udpTransport{conn: conn}
	w := zerodds.NewReliableAsyncWriter(tr, 0)

	// Producer: enqueue N samples (payload = the sample index, u32 LE). Never
	// touches the socket -- the drain goroutine owns the Transport.
	for i := 0; i < n; i++ {
		w.Enqueue(u32le(uint32(i)))
	}
	// Blocks until every sample has been submitted AND acknowledged (the send
	// window fully drains) -- never exits before that.
	w.Close()

	fmt.Printf("SENT count=%d\n", n)
}
