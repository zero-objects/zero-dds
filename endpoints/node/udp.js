// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// udp.js -- a real `dgram` UDP transport for the native Node endpoint, so the
// sync `Client` (pull/poll) and the async `AsyncReader` (async iterator) can run
// live against a peer (the Rust `bind_peer`/`ping_pong` harness), not just the
// in-memory `MemTransport`. Received datagrams land in an inbox the sync
// `receive()` drains; `recvAsync()` awaits the next one. Mirrors the role of the
// Java `ZdwEndpoint` DatagramSocket path.

'use strict';

const dgram = require('node:dgram');

class UdpTransport {
  constructor(peerHost, peerPort) {
    this.peerHost = peerHost;
    this.peerPort = peerPort;
    this._inbox = [];
    this._waiters = [];
    this._closed = false;
    this.socket = dgram.createSocket('udp4');
    this.socket.on('message', (msg) => {
      const frame = Buffer.from(msg);
      const w = this._waiters.shift();
      if (w) {
        w(frame);
      } else {
        this._inbox.push(frame);
      }
    });
  }

  // Client.write(frame) hook: send one framed message to the peer.
  deliver(frame) {
    if (this._closed) return false;
    this.socket.send(frame, this.peerPort, this.peerHost);
    return true;
  }

  // Client.poll()/AsyncReader hook: one non-blocking receive, or null.
  receive() {
    return this._inbox.length ? this._inbox.shift() : null;
  }

  // Awaits the next datagram (up to timeoutMs), or resolves null on timeout.
  recvAsync(timeoutMs = 20000) {
    const queued = this._inbox.shift();
    if (queued) return Promise.resolve(queued);
    return new Promise((resolve) => {
      let done = false;
      const t = setTimeout(() => {
        if (done) return;
        done = true;
        const i = this._waiters.indexOf(w);
        if (i >= 0) this._waiters.splice(i, 1);
        resolve(null);
      }, timeoutMs);
      const w = (frame) => {
        if (done) return;
        done = true;
        clearTimeout(t);
        resolve(frame);
      };
      this._waiters.push(w);
    });
  }

  close() {
    this._closed = true;
    try {
      this.socket.close();
    } catch (_e) {
      // already closed
    }
  }
}

module.exports = { UdpTransport };
