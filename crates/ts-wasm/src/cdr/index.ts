// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// index.ts — ts-wasm @zerodds/cdr re-export.
// Konformanz: zerodds-xcdr2-ts-1.0 §8 — binary-identisch zu
// crates/ts-node/src/cdr/. WASM-Layer braucht den Codec NICHT
// selbst; TS-Layer serialisiert ausserhalb des wasm-Bindings.

export { Xcdr2Writer } from './writer.js';
export { Xcdr2Reader } from './reader.js';
export { md5 } from './md5.js';
export { XcdrError } from './errors.js';
export type { DdsTopicType, ExtensibilityKind, EndianMode } from './types.js';
