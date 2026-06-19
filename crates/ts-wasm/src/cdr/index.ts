// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// index.ts — ts-wasm @zerodds/cdr re-export.
// Conformance: zerodds-xcdr2-ts-1.0 §8 — binary-identical to
// crates/ts-node/src/cdr/. The WASM layer does NOT need the codec
// itself; the TS layer serializes outside the wasm bindings.

export { Xcdr2Writer } from './writer.js';
export { Xcdr2Reader } from './reader.js';
export { md5 } from './md5.js';
export { XcdrError } from './errors.js';
export type { DdsTopicType, ExtensibilityKind, EndianMode } from './types.js';
