// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// facade.ts — fluent browser DCPS API over the websocket-bridge transport.
//
// This is the ergonomic instance-method facade used by the @zerodds/wasm
// quickstart (DomainParticipantFactory.instance() ->
// createParticipantWebSocket -> participant.createBytesTopic ->
// publisher.createBytesWriter / subscriber.createBytesReader). The flat,
// signature-for-signature DDS-TS Annex C.2 surface is in `operations.ts`; both
// drive the same BridgeTransport.

import {
  BridgeTransport,
  type WebSocketFactory,
} from "./transport.js";
import { type DataAvailableCallback, type Sample } from "./handles.js";
import { sampleFromBytes } from "./operations.js";

/// Entry point: the DDS DomainParticipantFactory singleton, browser flavour.
export class DomainParticipantFactory {
  private static singleton: DomainParticipantFactory | null = null;

  private constructor() {}

  /// Returns the process-wide factory instance.
  static instance(): DomainParticipantFactory {
    if (!DomainParticipantFactory.singleton) {
      DomainParticipantFactory.singleton = new DomainParticipantFactory();
    }
    return DomainParticipantFactory.singleton;
  }

  /// A participant on `domainId` through the ZeroDDS websocket bridge at
  /// `url` (`ws://host:port`). The bridge fronts a native DDS domain; the
  /// browser speaks DCPS to it over WebSocket (Annex C.4.1: browser transport
  /// SHALL be WebSocket / WebTransport / HTTP-3, never native UDP-multicast),
  /// a connection per topic (zerodds-ws-bridge-1.0 §4.2). `domainId` is the
  /// domain the bridge was started on; the bridge's configuration fixes it.
  async createParticipantWebSocket(
    url: string,
    domainId: number,
    factory?: WebSocketFactory,
  ): Promise<DomainParticipant> {
    const transport = await BridgeTransport.connect(url, factory);
    return new DomainParticipant(transport, domainId);
  }
}

/// A browser DomainParticipant bound to one bridge connection.
export class DomainParticipant {
  private disposed = false;

  constructor(
    private readonly transport: BridgeTransport,
    private readonly domain: number,
  ) {}

  /// The DDS domain id this participant joined.
  domainId(): number {
    return this.domain;
  }

  /// Creates a bytes topic (raw XCDR2 octets, no codec).
  createBytesTopic(name: string): Topic {
    this.ensureLive();
    return new Topic(this.transport, name, "DDS::Bytes");
  }

  /// Creates a topic with an explicit DDS type-name.
  createTopic(name: string, typeName: string): Topic {
    this.ensureLive();
    return new Topic(this.transport, name, typeName);
  }

  /// Creates a Publisher.
  createPublisher(): Publisher {
    this.ensureLive();
    return new Publisher(this.transport);
  }

  /// Creates a Subscriber.
  createSubscriber(): Subscriber {
    this.ensureLive();
    return new Subscriber(this.transport);
  }

  /// Closes the bridge connection and invalidates derived entities.
  destroy(): void {
    if (this.disposed) throw new RangeError("DomainParticipant already deleted");
    this.disposed = true;
    this.transport.close();
  }

  private ensureLive(): void {
    if (this.disposed) throw new RangeError("DomainParticipant deleted");
  }
}

/// A browser Topic (name + DDS type-name).
export class Topic {
  constructor(
    readonly transport: BridgeTransport,
    readonly name: string,
    readonly typeName: string,
  ) {}
}

/// A browser Publisher.
export class Publisher {
  constructor(private readonly transport: BridgeTransport) {}

  /// Creates a bytes DataWriter on `topic`.
  createBytesWriter(topic: Topic): DataWriter {
    return new DataWriter(this.transport, topic);
  }
}

/// A browser Subscriber.
export class Subscriber {
  constructor(private readonly transport: BridgeTransport) {}

  /// Creates a bytes DataReader on `topic`. The reader subscribes to the bridge
  /// immediately so samples published after this call are buffered.
  createBytesReader(topic: Topic): DataReader {
    const reader = new DataReader(this.transport, topic);
    reader.subscribe();
    return reader;
  }
}

const POLL_MS = 10;
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

/// A browser DataWriter publishing raw bytes over the bridge.
export class DataWriter {
  constructor(
    private readonly transport: BridgeTransport,
    private readonly topic: Topic,
  ) {}

  /// Publishes XCDR2 `bytes` to the topic. Synchronous (Annex C.2.3
  /// `writeSample`): listener delivery never occurs inside this call.
  write(bytes: Uint8Array): void {
    this.transport.publish(this.topic.name, bytes);
  }

  /// Promise-returning publish (mirrors the Node binding's writeAsync).
  async writeAsync(bytes: Uint8Array): Promise<void> {
    await Promise.resolve();
    this.write(bytes);
  }

  /// Resolves once the topic's connection to the bridge is open: the bridge
  /// writes the samples of this writer into DDS through its own DataWriter of
  /// the topic. The bridge reports no discovery count (zerodds-ws-bridge-1.0
  /// §4.4), so `min` is not checked against DDS readers. Rejects after
  /// `timeoutMs`.
  async waitForMatchedSubscription(_min: number, timeoutMs: number): Promise<void> {
    await this.transport.whenOpen(this.topic.name, timeoutMs);
  }
}

/// A browser DataReader pulling raw bytes from the bridge.
export class DataReader {
  private subscribed = false;
  private subId = `sub-${Math.random().toString(36).slice(2)}`;
  private listenerUnsub: (() => void) | null = null;

  constructor(
    private readonly transport: BridgeTransport,
    private readonly topic: Topic,
  ) {}

  /// Subscribes this reader's topic on the bridge (idempotent).
  subscribe(): void {
    if (this.subscribed) return;
    this.subscribed = true;
    this.transport.subscribe(this.topic.name, this.subId);
  }

  /// Takes all currently-buffered samples and returns the payloads of those
  /// with data; a dispose or an unregister carries none (its info is in
  /// `takeSamples`).
  take(): Uint8Array[] {
    return this.transport
      .drain(this.topic.name, 0)
      .filter((s) => s.kind === "alive")
      .map((s) => s.bytes);
  }

  /// Annex C.2.3 `takeSamples`: returns up to `max` full Samples (bytes+info).
  takeSamples(max: number): ReadonlyArray<Sample> {
    return this.transport.drain(this.topic.name, max).map(sampleFromBytes);
  }

  /// Resolves once the topic's connection to the bridge is open: from then on
  /// the bridge forwards the samples its DataReader of the topic takes (see
  /// the writer's note on `min`). Rejects after `timeoutMs`.
  async waitForMatchedPublication(_min: number, timeoutMs: number): Promise<void> {
    await this.transport.whenOpen(this.topic.name, timeoutMs);
  }

  /// Resolves once at least one sample is buffered, or after `timeoutMs`.
  async waitForData(timeoutMs: number): Promise<boolean> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      if (this.transport.pending(this.topic.name) > 0) return true;
      if (Date.now() >= deadline) return false;
      await sleep(POLL_MS);
    }
  }

  /// Async iterator over byte payloads as they arrive.
  async *streamSamples(): AsyncIterableIterator<Uint8Array> {
    for (;;) {
      const batch = this.take();
      if (batch.length > 0) {
        yield* batch;
        continue;
      }
      await sleep(POLL_MS);
    }
  }

  /// Annex C.2.4 — registers a data-available listener; `null` unregisters.
  /// Delivery is on the host event loop (the transport's message callback).
  setDataAvailableListener(cb: DataAvailableCallback | null): void {
    if (this.listenerUnsub) {
      this.listenerUnsub();
      this.listenerUnsub = null;
    }
    if (!cb) return;
    this.listenerUnsub = this.transport.onNotify((n) => {
      if (n.topic === this.topic.name) cb(0 as never);
    });
  }
}
