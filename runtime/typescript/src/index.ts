// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The TypeScript wire runtime, and the descriptor runtime beside it.
 *
 * Two halves under one import path. The first is XCDR itself — the
 * reader, the writer, the policies the dialects differ on — written
 * from DDS-XTypes 1.3 §7.4. The second is the DDS-TS 1.0 chapter 8
 * descriptor runtime: the side-table values the generated code
 * registers, the reflection functions that read them, and the Annex C
 * WASM bindings.
 *
 * The spec names the descriptor package `@zerodds/types` and says the
 * import path is implementation-defined while the exported names are
 * not (§8, Runtime Package Name). Every name chapter 8 defines is
 * exported here, unprefixed and unrenamed; the path is this one.
 */
export * from "./endianness.js";
export * from "./error.js";
export * from "./float128.js";
export * from "./policy.js";
export * from "./reader.js";
export * from "./writer.js";

/** The descriptor runtime — DDS-TS 1.0 chapter 8 and Annex C. */
export * from "./branded.js";
export * from "./dds_any.js";
export * from "./equal.js";
export * from "./registry.js";

/** The operations runtime — DDS-TS 1.0 chapter 11. */
export * from "./operations.js";
export * from "./test_backend.js";
export * from "./types.js";
export * from "./wasm.js";
