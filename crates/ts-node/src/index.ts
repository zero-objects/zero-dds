// index.ts — High-Level TypeScript-API über koffi-FFI.

import koffi from "koffi";
import * as N from "./native.js";

export class ZeroDdsError extends Error {
  constructor(public readonly code: number, message: string) {
    super(`${message} (status=${code})`);
    this.name = "ZeroDdsError";
  }
}

// Re-export DDS-PSM-Cxx 1.0 konforme Surface
export {
  DomainParticipantFactory, DomainParticipant, Topic, Publisher, DataWriter,
  Subscriber, DataReader, GuardCondition, WaitSet,
  ByteSeqTraits, StringTraits, type TopicTraits,
} from "./dds.js";

/// Domain-Runtime.
export class Runtime {
  private handle: unknown | null;

  constructor(domainId: number) {
    const h = N.zerodds_runtime_create(domainId);
    if (!h) throw new ZeroDdsError(-1, "zerodds_runtime_create");
    this.handle = h;
  }

  /** Roh-Handle für Friend-Klassen. */
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
    // koffi konvertiert `const char*` automatisch in JS-String.
    return N.zerodds_version() as unknown as string;
  }
}

/// DataWriter — pub-side.
export class Writer {
  private handle: unknown | null;

  constructor(rt: Runtime, topicName: string, typeName: string, reliable = true) {
    const h = N.zerodds_writer_create(rt.raw, topicName, typeName, reliable ? 1 : 0);
    if (!h) throw new ZeroDdsError(-1, "zerodds_writer_create");
    this.handle = h;
  }

  /// Payload kann ein Uint8Array oder Buffer sein.
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

  /// Returnt Uint8Array mit den Sample-Bytes oder null wenn keiner da.
  take(): Uint8Array | null {
    if (!this.handle) throw new Error("Reader disposed");
    const outBuf: { value: unknown } = { value: null };
    const outLen: { value: number } = { value: 0 };
    const rc = N.zerodds_reader_take(this.handle, outBuf, outLen);
    if (rc !== 0) throw new ZeroDdsError(rc, "zerodds_reader_take");
    if (!outBuf.value || outLen.value === 0) return null;
    // Buffer kopieren, dann free.
    const len = outLen.value;
    const copy = new Uint8Array(len);
    const view = koffi.decode(outBuf.value, "uint8_t", len) as Uint8Array;
    copy.set(view);
    N.zerodds_buffer_free(outBuf.value, len);
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
