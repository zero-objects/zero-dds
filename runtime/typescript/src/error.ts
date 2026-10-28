// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * What can go wrong on the wire.
 *
 * The variants of `crates/wire/src/error.rs`, as errors. Every one
 * names what was expected and where: a reader that returned a zero for
 * a truncated stream would hand the caller a value that was never
 * sent, and a writer that clamped an over-wide integer would put a
 * different number on the wire than the caller asked for.
 */

/** Base of every wire failure. */
export class WireError extends Error {
  constructor(message: string) {
    super(message);
    // `Error` is not a normal base in older targets and the prototype
    // is otherwise lost, which makes `instanceof` answer false for a
    // subclass. Set explicitly rather than left to the target.
    this.name = new.target.name;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

/** The stream ended before the value did. */
export class Truncated extends WireError {
  constructor(
    readonly at: number,
    readonly needed: number,
    readonly available: number,
  ) {
    super(`at ${at}: ${needed} bytes needed, ${available} available`);
  }
}

/**
 * A length field claims more than the stream holds.
 *
 * Separate from {@link Truncated}: a length that cannot be true is a
 * malformed stream, not a short one, and allocating for it is how a
 * reader turns a bad message into an out-of-memory.
 */
export class ImpossibleLength extends WireError {
  constructor(
    readonly at: number,
    readonly claimed: number,
    readonly available: number,
  ) {
    super(`at ${at}: a length of ${claimed} with ${available} bytes left`);
  }
}

/** A string held octets that are not UTF-8. */
export class NotUtf8 extends WireError {
  constructor(readonly at: number) {
    super(`at ${at}: the string is not UTF-8`);
  }
}

/**
 * A value outside what its type admits.
 *
 * Thrown by the writer as well as the reader, which is where
 * TypeScript is like Python and unlike Rust: an integer there is `i32`
 * or `u64` and the compiler has already refused the value, while a
 * `number` here holds anything a binary64 holds and a `bigint` is
 * unbounded, so the check has to happen at the point of the write.
 */
export class OutOfRange extends WireError {
  constructor(
    readonly at: number,
    readonly expected: string,
  ) {
    super(`at ${at}: expected ${expected}`);
  }
}
