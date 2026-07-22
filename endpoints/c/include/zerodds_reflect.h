/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * zerodds_reflect.h -- the `wire-variable` unit (ADR 0013): a reflective XCDR
 * codec driven by a runtime field descriptor instead of per-type generated
 * code. Where `wire-fixed` emits straight-line calls per type, `wire-variable`
 * walks a `zdw_dyn_field[]` at runtime -- for evolving/unknown types, a
 * monitor/spy, or characterizing an unknown wire format from captures. Same XCDR
 * bytes as the fixed path (byte-identical to the Rust core). C89, no malloc.
 */

#ifndef ZERODDS_REFLECT_H
#define ZERODDS_REFLECT_H

#include "zerodds_wire.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Extensibility for the reflective codec. */
typedef enum { ZDW_X_FINAL, ZDW_X_APPENDABLE, ZDW_X_MUTABLE } zdw_ext;

/* Field kinds the reflective codec understands (mirroring wire-fixed).
 * ZDW_K_NESTED / ZDW_K_SEQ_STRUCT reference a sub-struct descriptor. */
typedef enum {
    ZDW_K_U8, ZDW_K_U16, ZDW_K_U32, ZDW_K_U64,
    ZDW_K_F32, ZDW_K_F64, ZDW_K_BOOL, ZDW_K_STRING, ZDW_K_SEQ_U8,
    ZDW_K_NESTED, ZDW_K_SEQ_STRUCT
} zdw_kind;

struct zdw_dyn_struct;

/* A runtime field: `kind` selects the active value. On encode the value is
 * read; on decode it is written (string/seq use the caller's buffer + cap). */
typedef struct zdw_dyn_field {
    zdw_kind kind;
    unsigned char u8;
    unsigned int u16;
    unsigned long u32;
    zdw_u64_t u64;
    float f32;
    double f64;
    int boolean;
    char *str;            /* NUL-terminated buffer */
    size_t str_cap;       /* decode: capacity of str */
    unsigned char *bytes; /* sequence<octet> buffer */
    size_t bytes_len;     /* encode: input length; decode: output length */
    size_t bytes_cap;     /* decode: capacity of bytes */
    struct zdw_dyn_struct *nested; /* ZDW_K_NESTED: the sub-struct */
    struct zdw_dyn_struct *elems;  /* ZDW_K_SEQ_STRUCT: array of sub-structs */
    size_t elems_len;              /* number of elements (encode/decode) */
    size_t elems_cap;              /* decode: capacity of elems */
} zdw_dyn_field;

/* A runtime struct descriptor: extensibility + fields (+ member-ids for
 * MUTABLE). Carries both the shape and the values (a reflective value tree). */
typedef struct zdw_dyn_struct {
    zdw_ext ext;
    zdw_dyn_field *fields;
    const unsigned long *ids; /* per-field member-id (MUTABLE only) */
    size_t n;
} zdw_dyn_struct;

/* Recursively encodes/decodes a struct (handles nested + sequence<struct> +
 * the extensibility trio). */
int zdw_reflect_encode_struct(zdw_writer *w, const zdw_dyn_struct *s);
int zdw_reflect_decode_struct(zdw_reader *r, zdw_dyn_struct *s);

/* Reflectively encodes `n` fields (final extensibility). */
int zdw_reflect_encode(zdw_writer *w, const zdw_dyn_field *fields, size_t n);
/* Reflectively decodes `n` fields; each field's `kind` (+ str/bytes buffers)
 * must be set on input. */
int zdw_reflect_decode(zdw_reader *r, zdw_dyn_field *fields, size_t n);

/* Reflectively encodes a struct with the given extensibility. For MUTABLE,
 * `ids` holds each field's member-id (LC4 EMHEADER); ignored otherwise. */
int zdw_reflect_encode_ext(zdw_writer *w, zdw_ext ext,
                           const zdw_dyn_field *fields, const unsigned long *ids,
                           size_t n);
/* Inverse of zdw_reflect_encode_ext (members decoded in wire order). */
int zdw_reflect_decode_ext(zdw_reader *r, zdw_ext ext, zdw_dyn_field *fields,
                           size_t n);

#ifdef __cplusplus
}
#endif

#endif /* ZERODDS_REFLECT_H */
