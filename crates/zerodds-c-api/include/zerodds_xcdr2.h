/*
 * zerodds_xcdr2.h — C-FFI typed dispatch (vendor spec
 *                   `zerodds-xcdr2-c-1.0`).
 *
 * The `zerodds_typesupport_t` dispatch contract: a per-type table of
 * encode/decode/key-hash/free function pointers that
 * `zerodds_writer_write_typed` and `zerodds_reader_take_typed` call
 * through. `zerodds-idlc generate --lang c` writes one table per topic
 * type into `<stem>_typesupport.h`; its functions call the C wire
 * runtime `runtime/c/include/zerodds_wire.h` (`zdw_*`), the same one the
 * generated codec header calls.
 *
 * The table, the function-pointer types and the typed functions are
 * declared once, by `zerodds.h`. This header gives them the names the
 * vendor spec uses and adds what a table's functions answer with. It
 * used to declare the typed functions a second time over a struct of
 * its own (`struct zerodds_typesupport_s`), and a translation unit that
 * included both headers did not compile: "conflicting types for
 * 'zerodds_topic_create_typed'".
 *
 * Memory ownership: see vendor spec §6.
 *
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 */

#ifndef ZERODDS_XCDR2_H
#define ZERODDS_XCDR2_H

#include "zerodds.h"

/* ========================================================================
 * Vendor spec §2 — the TypeSupport table and its function types.
 * ====================================================================== */

typedef struct zerodds_ZeroDdsTypeSupport zerodds_typesupport_t;

typedef zerodds_Option_ZeroDdsEncodeFn zerodds_encode_fn;
typedef zerodds_Option_ZeroDdsDecodeFn zerodds_decode_fn;
typedef zerodds_Option_ZeroDdsDecodeReprFn zerodds_decode_repr_fn;
typedef zerodds_Option_ZeroDdsKeyHashFn zerodds_key_hash_fn;
typedef zerodds_Option_ZeroDdsSampleFreeFn zerodds_sample_free_fn;

/* ========================================================================
 * What a table's functions answer: `ZeroDdsStatus` in the C API.
 * ====================================================================== */

#define ZERODDS_TS_OK 0
#define ZERODDS_TS_BAD_PARAMETER (-6)
#define ZERODDS_TS_OUT_OF_RESOURCES (-8)
#define ZERODDS_TS_UNSUPPORTED (-13)
/* An encoder's answer to `out_buf == NULL` or a buffer too short, with the
 * length it needs in `*out_len`. The same value as UNSUPPORTED, which is
 * what `zerodds_writer_write_typed` reads it as. */
#define ZERODDS_TS_BUFFER_TOO_SMALL (-13)

/* ========================================================================
 * The `representation` byte `decode_repr` is handed: the XCDR version,
 * with ZERODDS_TS_REPR_BIG_ENDIAN set for a big-endian sample — the
 * version and the byte order of the encapsulation identifier
 * (DDSI-RTPS 2.5 §10.5), as `zerodds_reader_take_endian` reports them.
 * ====================================================================== */

#define ZERODDS_TS_REPR_XCDR1 0u
#define ZERODDS_TS_REPR_XCDR2 1u
#define ZERODDS_TS_REPR_BIG_ENDIAN 0x80u

#endif /* ZERODDS_XCDR2_H */
