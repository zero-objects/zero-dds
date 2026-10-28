// index.ts — high-level TypeScript API over the koffi FFI.

import koffi from "koffi";
import * as N from "./native.js";

export class ZeroDdsError extends Error {
  constructor(public readonly code: number, message: string) {
    super(`${message} (status=${code})`);
    this.name = "ZeroDdsError";
  }
}

// Re-export the DDS-PSM-Cxx 1.0 conform surface
export {
  DomainParticipantFactory, DomainParticipantFactoryHandle, DomainParticipant,
  Topic, ContentFilteredTopic, CftFieldKind, Publisher, DataWriter,
  Subscriber, DataReader, GuardCondition, WaitSet,
  ByteSeqTraits, StringTraits,
  type TopicTraits, type TypeSupport, type Sample, type SampleWithInfo,
  // QoS surface (OMG DDS-DCPS 1.4 §2.2.3)
  type DataWriterQos, type DataReaderQos, type PublisherQos,
  type SubscriberQos, type TopicQos,
  type ReliabilityPolicy, type DurabilityPolicy, type HistoryPolicy,
  type DeadlinePolicy, type LivelinessPolicy, type OwnershipPolicy,
  type OwnershipStrengthPolicy, type PartitionPolicy, type Duration,
  ReliabilityKind, DurabilityKind, HistoryKind, OwnershipKind,
  LivelinessKind, DestinationOrderKind,
  DURATION_ZERO, DURATION_INFINITE,
  defaultDataWriterQos, defaultDataReaderQos, defaultPublisherQos,
  defaultSubscriberQos, defaultTopicQos,
} from "./dds.js";

/// Domain runtime.
export class Runtime {
  private handle: unknown | null;

  constructor(domainId: number) {
    const h = N.zerodds_runtime_create(domainId);
    if (!h) throw new ZeroDdsError(-1, "zerodds_runtime_create");
    this.handle = h;
  }

  /** Raw handle for friend classes. */
  get raw(): unknown {
    if (!this.handle) throw new Error("Runtime disposed");
    return this.handle;
  }

  destroy(): void {
    if (this.handle) {
      N.zerodds_runtime_destroy(this.handle);
      this.handle = null;
    }
  }

  static version(): string {
    // koffi automatically converts `const char*` into a JS string.
    return N.zerodds_version() as unknown as string;
  }
}

/// How a type's body is framed, XTypes 1.3 §7.3.1.2.1 — the choice that
/// decides which encapsulation identifier the sample has to carry.
export type Extensibility = "final" | "appendable" | "mutable";

/// The C ABI's own numbering for `zerodds_writer_set_wire_extensibility`.
const WIRE_EXTENSIBILITY: Record<Extensibility, number> = {
  final: 0,
  appendable: 1,
  mutable: 2,
};

/// DataWriter — pub-side.
export class Writer {
  private handle: unknown | null;

  /// `extensibility` names how the payloads handed to {@link write} are
  /// framed. It is not decoration: the encapsulation identifier the runtime
  /// stamps is chosen from it (XTypes 1.3 §7.6.3.1.2 Table 59 / DDSI-RTPS 2.5
  /// §10.5), an `@appendable` body opens with a DHEADER and a `@mutable` one
  /// with an EMHEADER per member, and a writer that leaves this at the C ABI's
  /// default announces `PLAIN_CDR2_LE` (0x0007) over a body that is neither —
  /// measured. Cyclone decodes such a sample anyway (it reads the body with
  /// its own type, not with the header); OpenDDS refuses it in `to_encoding_i`
  /// — "expected APPENDABLE extensibility, but got ..." — and nothing arrives
  /// on the topic.
  constructor(
    rt: Runtime,
    topicName: string,
    typeName: string,
    reliable = true,
    extensibility: Extensibility = "final",
  ) {
    const code = WIRE_EXTENSIBILITY[extensibility];
    // Rejected before the writer exists rather than after: a create followed
    // by a destroy would put an endpoint through discovery that is torn down
    // again a moment later. `undefined` is what an untyped JS caller reaches
    // here with, and reading it as FINAL is the guess this argument exists to
    // stop.
    if (code === undefined) {
      throw new ZeroDdsError(
        -1,
        `Writer: extensibility must be "final", "appendable" or "mutable", got ${JSON.stringify(extensibility)}`,
      );
    }
    const h = N.zerodds_writer_create(rt.raw, topicName, typeName, reliable ? 1 : 0);
    if (!h) throw new ZeroDdsError(-1, "zerodds_writer_create");
    const rc = N.zerodds_writer_set_wire_extensibility(h, code) as number;
    if (rc !== 0) {
      // Destroyed rather than kept: a writer that could not declare its own
      // framing publishes samples no strict peer accepts, and doing that
      // quietly is the failure this call exists to end.
      N.zerodds_writer_destroy(h);
      throw new ZeroDdsError(rc, "zerodds_writer_set_wire_extensibility");
    }
    this.handle = h;
  }

  /// Payload can be a Uint8Array or Buffer.
  write(payload: Uint8Array): void {
    if (!this.handle) throw new Error("Writer disposed");
    const rc = N.zerodds_writer_write(this.handle, payload, payload.byteLength);
    if (rc !== 0) throw new ZeroDdsError(rc, "zerodds_writer_write");
  }

  waitForMatched(minCount: number, timeoutMs: number): boolean {
    if (!this.handle) throw new Error("Writer disposed");
    const rc = N.zerodds_writer_wait_for_matched(this.handle, minCount, timeoutMs);
    return rc === 0;
  }

  destroy(): void {
    if (this.handle) {
      N.zerodds_writer_destroy(this.handle);
      this.handle = null;
    }
  }
}

/// DataReader — sub-side.
export class Reader {
  private handle: unknown | null;

  constructor(rt: Runtime, topicName: string, typeName: string, reliable = true) {
    const h = N.zerodds_reader_create(rt.raw, topicName, typeName, reliable ? 1 : 0);
    if (!h) throw new ZeroDdsError(-1, "zerodds_reader_create");
    this.handle = h;
  }

  /// Returns a Uint8Array with the sample bytes or null if none is available.
  take(): Uint8Array | null {
    if (!this.handle) throw new Error("Reader disposed");
    // koffi `_Out_` parameters use a single-element array as the in/out box:
    // koffi writes the returned pointer / length into element [0]. Passing a
    // plain `{ value }` object instead throws at the call site with
    // "Unexpected Object value, expected void **".
    const outBuf: [unknown] = [null];
    const outLen: [number] = [0];
    const outRepr: [number] = [0]; // XCDR representation byte (discarded here)
    const rc = N.zerodds_reader_take(this.handle, outBuf, outLen, outRepr);
    if (rc !== 0) throw new ZeroDdsError(rc, "zerodds_reader_take");
    const buf = outBuf[0];
    const len = outLen[0];
    if (!buf || len === 0) return null;
    // Copy the buffer, then free.
    const copy = new Uint8Array(len);
    const view = koffi.decode(buf, "uint8_t", len) as Uint8Array;
    copy.set(view);
    N.zerodds_buffer_free(buf, len);
    return copy;
  }

  waitForMatched(minCount: number, timeoutMs: number): boolean {
    if (!this.handle) throw new Error("Reader disposed");
    const rc = N.zerodds_reader_wait_for_matched(this.handle, minCount, timeoutMs);
    return rc === 0;
  }

  destroy(): void {
    if (this.handle) {
      N.zerodds_reader_destroy(this.handle);
      this.handle = null;
    }
  }
}
