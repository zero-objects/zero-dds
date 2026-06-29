// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// transport.ts — WebSocket transport for the browser DCPS client.
//
// Speaks the ZeroDDS websocket-bridge JSON protocol (crates/websocket-bridge,
// `dds_bridge.rs`):
//
//   Subscribe  C->S : {"op":"subscribe",  "topic":"<t>", "id":"<sub-id>"}
//   Publish    C->S : {"op":"publish",    "topic":"<t>", "data":"<payload>"}
//   Notify     S->C : {"op":"notify",     "topic":"<t>", "data":"<payload>", "subscription_id":"<sub-id>"}
//
// XCDR2 sample bytes are carried base64-encoded in the JSON `data` string so
// arbitrary octets survive the text frame (Annex C.3.1: the bytes cross the
// boundary unchanged; base64 is a reversible transport framing, not a wire
// re-encoding).

/// Minimal structural view of a WHATWG `WebSocket`. The browser global and the
/// Node `ws` package both satisfy this, so the transport is testable off-browser
/// by injecting a constructor.
export interface WebSocketLike {
  send(data: string): void;
  close(): void;
  addEventListener(type: "open", cb: () => void): void;
  addEventListener(type: "close", cb: () => void): void;
  addEventListener(type: "error", cb: (ev: unknown) => void): void;
  addEventListener(type: "message", cb: (ev: { data: unknown }) => void): void;
}

/// Constructs a `WebSocketLike` for `url`. Defaults to the browser `WebSocket`
/// global; a test or Node host can supply its own (e.g. the `ws` package).
export type WebSocketFactory = (url: string) => WebSocketLike;

function defaultFactory(url: string): WebSocketLike {
  const ctor = (globalThis as { WebSocket?: new (url: string) => WebSocketLike }).WebSocket;
  if (!ctor) {
    throw new Error(
      "no global WebSocket; pass a WebSocketFactory to createParticipantWebSocket",
    );
  }
  return new ctor(url);
}

/// base64 of arbitrary bytes, runtime-agnostic (browser `btoa`/`atob` or Node
/// `Buffer`).
export function bytesToBase64(bytes: Uint8Array): string {
  const g = globalThis as { btoa?: (s: string) => string; Buffer?: { from(b: Uint8Array): { toString(enc: string): string } } };
  if (g.Buffer) return g.Buffer.from(bytes).toString("base64");
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return g.btoa!(bin);
}

export function base64ToBytes(b64: string): Uint8Array {
  const g = globalThis as { atob?: (s: string) => string; Buffer?: { from(s: string, enc: string): Uint8Array } };
  if (g.Buffer) return new Uint8Array(g.Buffer.from(b64, "base64"));
  const bin = g.atob!(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/// A delivered notification: the raw XCDR2 bytes for one topic, plus the wire
/// byte order. The forwarded bytes carry no encapsulation header, so the
/// bridge reports the order out-of-band (`"be":true` on the notify frame); the
/// browser dispatches the big-endian decoder on it. `false` = little-endian.
export interface BridgeNotification {
  readonly topic: string;
  readonly bytes: Uint8Array;
  readonly bigEndian: boolean;
}

/// One buffered sample: payload bytes + their wire byte order.
export interface QueuedSample {
  readonly bytes: Uint8Array;
  readonly bigEndian: boolean;
}

type NotifyHandler = (n: BridgeNotification) => void;

/// Owns one WebSocket connection to the bridge and multiplexes
/// subscribe/publish/notify over it.
export class BridgeTransport {
  private ws: WebSocketLike | null = null;
  private opened = false;
  private readonly subscribed = new Set<string>();
  private readonly handlers = new Set<NotifyHandler>();
  /// Notifications received before any subscriber drained them, per topic. The
  /// browser DCPS reader pulls from here on `take`.
  private readonly inbox = new Map<string, QueuedSample[]>();

  private constructor(private readonly url: string) {}

  /// Connects to the bridge at `url`. Resolves once the socket is open.
  static connect(url: string, factory: WebSocketFactory = defaultFactory): Promise<BridgeTransport> {
    const t = new BridgeTransport(url);
    return new Promise((resolve, reject) => {
      let ws: WebSocketLike;
      try {
        ws = factory(url);
      } catch (e) {
        reject(e);
        return;
      }
      t.ws = ws;
      ws.addEventListener("open", () => {
        t.opened = true;
        resolve(t);
      });
      ws.addEventListener("error", (ev) => {
        if (!t.opened) reject(new Error(`websocket error connecting to ${url}: ${String(ev)}`));
      });
      ws.addEventListener("close", () => {
        t.opened = false;
      });
      ws.addEventListener("message", (ev) => t.onMessage(ev.data));
    });
  }

  private onMessage(data: unknown): void {
    if (typeof data !== "string") return;
    let msg: { op?: string; topic?: string; data?: string; be?: boolean };
    try {
      msg = JSON.parse(data);
    } catch {
      return;
    }
    if (msg.op !== "notify" || !msg.topic || typeof msg.data !== "string") return;
    const bytes = base64ToBytes(msg.data);
    // `be` is present (true) only for a big-endian payload; absent ⇒ LE.
    const bigEndian = msg.be === true;
    const queue = this.inbox.get(msg.topic) ?? [];
    queue.push({ bytes, bigEndian });
    this.inbox.set(msg.topic, queue);
    const n: BridgeNotification = { topic: msg.topic, bytes, bigEndian };
    for (const h of this.handlers) h(n);
  }

  /// Registers a notification handler (used to drive data-available listeners).
  onNotify(h: NotifyHandler): () => void {
    this.handlers.add(h);
    return () => this.handlers.delete(h);
  }

  /// Subscribes to `topic` (idempotent per connection).
  subscribe(topic: string, subId: string): void {
    if (this.subscribed.has(topic)) return;
    this.subscribed.add(topic);
    this.send({ op: "subscribe", topic, id: subId });
  }

  /// Publishes XCDR2 `bytes` to `topic`.
  publish(topic: string, bytes: Uint8Array): void {
    this.send({ op: "publish", topic, data: bytesToBase64(bytes) });
  }

  /// Drains up to `max` buffered samples for `topic`.
  drain(topic: string, max: number): QueuedSample[] {
    const queue = this.inbox.get(topic);
    if (!queue || queue.length === 0) return [];
    const take = max <= 0 ? queue.length : Math.min(max, queue.length);
    return queue.splice(0, take);
  }

  /// Number of buffered samples for `topic`.
  pending(topic: string): number {
    return this.inbox.get(topic)?.length ?? 0;
  }

  private send(obj: Record<string, string>): void {
    if (!this.ws) throw new Error("BridgeTransport closed");
    this.ws.send(JSON.stringify(obj));
  }

  close(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.opened = false;
    this.subscribed.clear();
    this.handlers.clear();
    this.inbox.clear();
  }
}
