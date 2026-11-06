// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// transport.ts — WebSocket transport for the browser DCPS client.
//
// Speaks the binary frames of zerodds-ws-bridge-1.0 (the daemon
// `zerodds-ws-bridged`, crates/websocket-bridge/src/daemon):
//
//   §4.2  one connection per topic, on the topic's path `/topics/<slug>`
//         (§5.1); the daemon subscribes a connection to the topic its path
//         names, and writes the binary frames a client sends on it into DDS.
//   §4.3  every binary frame carries one sample: the magic `ZDB1`, a flags
//         octet (bit 0 a dispose, bit 2 an unregister), the encapsulation
//         header the sample came under (DDSI-RTPS 2.5 §10.5) and the CDR
//         payload byte-exact as over RTPS. A client sends samples with flags
//         0; this transport writes them under XCDR2 little endian
//         (`00 07 00 00`) unless the caller names another header.
//   §4.4  control messages are text frames; an `error` from the daemon is
//         kept per topic (`errors`).
//
// The JSON text frames of §4.3 (the `+json` subprotocol) need the topic's
// type on the daemon and carry the sample as JSON; the DCPS client exchanges
// XCDR2 octets (DDS-TS Annex C.3.1), so it uses the binary frames.

/// Minimal structural view of a WHATWG `WebSocket`. The browser global, the
/// global of Node 22 and the Node `ws` package all satisfy it, so the
/// transport runs off-browser with an injected constructor.
export interface WebSocketLike {
  binaryType?: string;
  send(data: string | Uint8Array): void;
  close(): void;
  addEventListener(type: "open", cb: () => void): void;
  addEventListener(type: "close", cb: () => void): void;
  addEventListener(type: "error", cb: (ev: unknown) => void): void;
  addEventListener(type: "message", cb: (ev: { data: unknown }) => void): void;
}

/// Constructs a `WebSocketLike` for `url`, asking for `protocols` (RFC 6455
/// §4.1). Defaults to the global `WebSocket`; a test or Node host can supply
/// its own (e.g. the `ws` package).
export type WebSocketFactory = (url: string, protocols?: string[]) => WebSocketLike;

function defaultFactory(url: string, protocols?: string[]): WebSocketLike {
  const ctor = (globalThis as {
    WebSocket?: new (url: string, protocols?: string[]) => WebSocketLike;
  }).WebSocket;
  if (!ctor) {
    throw new Error(
      "no global WebSocket; pass a WebSocketFactory to createParticipantWebSocket",
    );
  }
  return new ctor(url, protocols);
}

/// The subprotocol of the binary frames (zerodds-ws-bridge-1.0 §4.1).
export const SUBPROTOCOL = "zerodds-ws-bridge-1.0";

/// `ZDB1`, the magic of a binary frame (§4.3).
const MAGIC = [0x5a, 0x44, 0x42, 0x31];
/// Flag bit 0 of a binary frame: a dispose (§4.3).
export const FLAG_DISPOSE = 0x01;
/// Flag bit 2 of a binary frame: an unregister (§4.3).
export const FLAG_UNREGISTER = 0x04;

/// The encapsulation header of XCDR2 little endian, `PLAIN_CDR2` (DDSI-RTPS
/// 2.5 §10.5, DDS-XTypes 1.3 Table 60): what a sample of a `@final` type is
/// written under. An `@appendable` type is `00 09 00 00`, a `@mutable` one
/// `00 0b 00 00`.
export const XCDR2_LE = new Uint8Array([0x00, 0x07, 0x00, 0x00]);

/// The path the daemon gives a topic by default (§4.2, the slug of §5.1):
/// lowercase, `::` a `/`, every other character outside `[a-z0-9/_-]` a `_`.
export function topicPath(topic: string): string {
  const lower = topic.toLowerCase();
  let slug = "";
  for (let i = 0; i < lower.length; i++) {
    if (lower[i] === ":" && lower[i + 1] === ":") {
      slug += "/";
      i++;
      continue;
    }
    slug += /[a-z0-9/_-]/.test(lower[i]) ? lower[i] : "_";
  }
  return "/topics/" + slug;
}

/// A binary frame of §4.3 for one sample: `ZDB1`, flags 0, the encapsulation
/// header, the CDR payload.
export function encodeFrame(payload: Uint8Array, encapsulation: Uint8Array = XCDR2_LE): Uint8Array {
  if (encapsulation.length !== 4) {
    throw new RangeError("an encapsulation header is four octets (DDSI-RTPS 2.5 §10.5)");
  }
  const out = new Uint8Array(9 + payload.length);
  out.set(MAGIC, 0);
  out[4] = 0;
  out.set(encapsulation, 5);
  out.set(payload, 9);
  return out;
}

/// The parts of a binary frame of §4.3.
export interface DecodedFrame {
  readonly flags: number;
  /// The representation identifier of the encapsulation header, big endian
  /// (DDSI-RTPS 2.5 §10.5): `0x0007` is XCDR2 little endian.
  readonly identifier: number;
  /// The CDR payload; for a dispose or an unregister the key holder.
  readonly payload: Uint8Array;
}

/// Reads a binary frame of §4.3, or `null` for octets that are none: shorter
/// than magic, flags and header, another magic, or a header that names no CDR
/// representation (DDS-XTypes 1.3 Table 60).
export function decodeFrame(frame: Uint8Array): DecodedFrame | null {
  if (frame.length < 9) return null;
  for (let i = 0; i < 4; i++) if (frame[i] !== MAGIC[i]) return null;
  const identifier = (frame[5] << 8) | frame[6];
  if (!(identifier <= 0x0003 || (identifier >= 0x0006 && identifier <= 0x000b))) return null;
  return { flags: frame[4], identifier, payload: frame.subarray(9) };
}

/// What a sample frame says of its instance (§4.3 flags).
export type SampleKind = "alive" | "disposed" | "unregistered" | "disposed_unregistered";

function kindOf(flags: number): SampleKind {
  const disposed = (flags & FLAG_DISPOSE) !== 0;
  const unregistered = (flags & FLAG_UNREGISTER) !== 0;
  if (disposed && unregistered) return "disposed_unregistered";
  if (disposed) return "disposed";
  if (unregistered) return "unregistered";
  return "alive";
}

/// A delivered sample of one topic: its payload, the byte order its
/// encapsulation header names (the identifiers of big endian are even), and
/// what it says of its instance.
export interface BridgeNotification {
  readonly topic: string;
  readonly bytes: Uint8Array;
  readonly bigEndian: boolean;
  readonly kind: SampleKind;
}

/// One buffered sample.
export interface QueuedSample {
  readonly bytes: Uint8Array;
  readonly bigEndian: boolean;
  readonly kind: SampleKind;
}

type NotifyHandler = (n: BridgeNotification) => void;

/// The connection of one topic.
interface Channel {
  ws: WebSocketLike;
  open: boolean;
  /// Frames written before the connection opened, sent once it is.
  unsent: Uint8Array[];
  ready: Promise<void>;
}

function asOctets(data: unknown): Uint8Array | null {
  if (data instanceof Uint8Array) return data;
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (ArrayBuffer.isView(data)) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  }
  return null;
}

/// The connections to one bridge, a connection per topic (§4.2).
export class BridgeTransport {
  private readonly channels = new Map<string, Channel>();
  private readonly subscribed = new Set<string>();
  private readonly handlers = new Set<NotifyHandler>();
  /// Samples received for a subscribed topic and not yet taken. The browser
  /// DCPS reader pulls from here on `take`.
  private readonly inbox = new Map<string, QueuedSample[]>();
  /// The `error` control messages of the daemon (§4.4), per topic.
  readonly errors = new Map<string, string[]>();
  private closed = false;

  private constructor(
    private readonly base: string,
    private readonly factory: WebSocketFactory,
  ) {}

  /// A transport to the bridge at `url` (`ws://host:port`, or with a path
  /// prefix the topic paths are appended to). It opens a topic's connection
  /// when the topic is first subscribed or written.
  static connect(url: string, factory: WebSocketFactory = defaultFactory): Promise<BridgeTransport> {
    let base: string;
    try {
      const parsed = new URL(url);
      if (parsed.protocol !== "ws:" && parsed.protocol !== "wss:") {
        throw new Error(`not a WebSocket URL: ${url}`);
      }
      base = url.replace(/\/+$/, "");
    } catch (e) {
      return Promise.reject(e);
    }
    return Promise.resolve(new BridgeTransport(base, factory));
  }

  private channel(topic: string): Channel {
    const existing = this.channels.get(topic);
    if (existing) return existing;
    if (this.closed) throw new Error("BridgeTransport closed");
    const url = this.base + topicPath(topic);
    const ws = this.factory(url, [SUBPROTOCOL]);
    // Binary frames as ArrayBuffer, not as the browser's default Blob.
    try {
      ws.binaryType = "arraybuffer";
    } catch {
      // A socket without the property delivers octets already.
    }
    let opened: () => void = () => {};
    let failed: (e: Error) => void = () => {};
    const ready = new Promise<void>((resolve, reject) => {
      opened = resolve;
      failed = reject;
    });
    // A connection nobody waits for must not report an unhandled rejection.
    ready.catch(() => {});
    const channel: Channel = { ws, open: false, unsent: [], ready };
    ws.addEventListener("open", () => {
      channel.open = true;
      for (const frame of channel.unsent.splice(0)) ws.send(frame);
      opened();
    });
    ws.addEventListener("error", (ev) => {
      if (!channel.open) failed(new Error(`websocket error connecting to ${url}: ${String(ev)}`));
    });
    ws.addEventListener("close", () => {
      if (!channel.open) failed(new Error(`websocket to ${url} closed before it opened`));
      channel.open = false;
    });
    ws.addEventListener("message", (ev) => this.onMessage(topic, ev.data));
    this.channels.set(topic, channel);
    return channel;
  }

  private onMessage(topic: string, data: unknown): void {
    if (typeof data === "string") {
      let msg: { op?: string; code?: number; message?: string };
      try {
        msg = JSON.parse(data);
      } catch {
        return;
      }
      if (msg.op === "error") {
        const list = this.errors.get(topic) ?? [];
        list.push(`${msg.code ?? ""} ${msg.message ?? ""}`.trim());
        this.errors.set(topic, list);
      }
      return;
    }
    const octets = asOctets(data);
    if (!octets) return;
    const frame = decodeFrame(octets);
    if (!frame || !this.subscribed.has(topic)) return;
    const sample: QueuedSample = {
      // A copy: the payload is a view into the frame's buffer.
      bytes: frame.payload.slice(),
      bigEndian: (frame.identifier & 0x0001) === 0,
      kind: kindOf(frame.flags),
    };
    const queue = this.inbox.get(topic) ?? [];
    queue.push(sample);
    this.inbox.set(topic, queue);
    const n: BridgeNotification = { topic, ...sample };
    for (const h of this.handlers) h(n);
  }

  /// Registers a notification handler (used to drive data-available listeners).
  onNotify(h: NotifyHandler): () => void {
    this.handlers.add(h);
    return () => this.handlers.delete(h);
  }

  /// Subscribes to `topic`: opens its connection, whose path the daemon
  /// subscribes (idempotent per transport). `subId` names the reader and is
  /// not on the wire.
  subscribe(topic: string, _subId?: string): void {
    this.subscribed.add(topic);
    this.channel(topic);
  }

  /// Writes `bytes`, the CDR payload of one sample, to `topic` under
  /// `encapsulation` (default XCDR2 little endian). A frame written before
  /// the topic's connection is open goes out when it opens.
  publish(topic: string, bytes: Uint8Array, encapsulation: Uint8Array = XCDR2_LE): void {
    const channel = this.channel(topic);
    const frame = encodeFrame(bytes, encapsulation);
    if (channel.open) channel.ws.send(frame);
    else channel.unsent.push(frame);
  }

  /// Resolves once `topic`'s connection is open: the daemon has subscribed
  /// it and takes its frames. Rejects if it fails, or after `timeoutMs`.
  whenOpen(topic: string, timeoutMs: number): Promise<void> {
    const channel = this.channel(topic);
    if (channel.open) return Promise.resolve();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error(`no connection for topic ${topic} within ${timeoutMs} ms`)),
        timeoutMs,
      );
      channel.ready.then(
        () => {
          clearTimeout(timer);
          resolve();
        },
        (e) => {
          clearTimeout(timer);
          reject(e);
        },
      );
    });
  }

  /// Drains up to `max` buffered samples for `topic` (all for `max <= 0`).
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

  close(): void {
    this.closed = true;
    for (const channel of this.channels.values()) channel.ws.close();
    this.channels.clear();
    this.subscribed.clear();
    this.handlers.clear();
    this.inbox.clear();
  }
}
