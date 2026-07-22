/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * zerodds_wire.h -- ZeroDDS endpoint wire-core, ISO C89.
 *
 * The `wire-core` unit of the native endpoint SDK (ADR 0013): the XCDR
 * primitive layer, byte-identical to the Rust core's `zerodds-cdr`. No malloc
 * (the caller owns the buffer), no external libs, no C99. Serialization is
 * byte-by-byte with explicit shifts, so the output is independent of the host
 * byte order -- a big-endian 68k/PA-RISC/SPARC endpoint and a little-endian
 * x86-64 host produce the SAME wire bytes. The WIRE byte order is an explicit
 * parameter (LE/BE), honoring the XCDR encapsulation byte-order flag
 * (DDSI-RTPS 2.5 section 10.5).
 *
 * XCDR2 alignment (OMG XTypes 1.3 section 7.4.1.1.1): alignment is relative to
 * the stream start and capped at 4, so a 64-bit value aligns to 4, not 8.
 */

#ifndef ZERODDS_WIRE_H
#define ZERODDS_WIRE_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Wire byte order (the XCDR encapsulation byte-order flag, not the host's). */
#define ZDW_LE 0
#define ZDW_BE 1

/* Alignment cap: XCDR2 = 4 (PLAIN_CDR2), XCDR1 = 8 (PLAIN_CDR). */
#define ZDW_XCDR2_MAX_ALIGN 4
#define ZDW_XCDR1_MAX_ALIGN 8

/* Result codes. */
#define ZDW_OK        0
#define ZDW_E_OVERrun 1  /* writer: not enough room / reader: not enough data */
#define ZDW_E_INVALID 2  /* malformed input (e.g. bad string terminator)      */

/* A 64-bit value as two 32-bit halves (portable to true C89, which has no
 * standard 64-bit integer). `hi` is the most significant 32 bits. */
typedef struct {
    unsigned long hi;
    unsigned long lo;
} zdw_u64_t;

/* Write cursor over a caller-owned buffer. */
typedef struct {
    unsigned char *buf; /* caller-owned backing store            */
    size_t cap;         /* capacity in bytes                     */
    size_t len;         /* bytes written so far (== position)    */
    int endian;         /* ZDW_LE or ZDW_BE (wire byte order)    */
    int max_align;      /* alignment cap (4 = XCDR2, 8 = XCDR1)  */
    int error;          /* sticky error code (ZDW_OK until set)  */
} zdw_writer;

/* Read cursor over a caller-owned buffer. */
typedef struct {
    const unsigned char *buf;
    size_t len;         /* total bytes available                 */
    size_t pos;         /* current position                      */
    int endian;
    int max_align;
    int error;
} zdw_reader;

/* --- lifecycle --- */

void zdw_writer_init(zdw_writer *w, unsigned char *buf, size_t cap, int endian);
void zdw_reader_init(zdw_reader *r, const unsigned char *buf, size_t len, int endian);

/* --- writer: aligned primitives (return ZDW_OK or set + return the error) --- */

int zdw_put_u8(zdw_writer *w, unsigned char v);
int zdw_put_u16(zdw_writer *w, unsigned int v);
int zdw_put_u32(zdw_writer *w, unsigned long v);
int zdw_put_u64(zdw_writer *w, zdw_u64_t v);
int zdw_put_bool(zdw_writer *w, int v);
/* f32/f64 are written as their IEEE-754 bit pattern via the u32/u64 path,
 * exactly like the Rust core (`f32::to_bits`/`f64::to_bits`). */
int zdw_put_f32(zdw_writer *w, float v);
int zdw_put_f64(zdw_writer *w, double v);
/* CDR string: u32 length (incl. NUL) + bytes + one NUL. `s` is NUL-terminated. */
int zdw_put_string(zdw_writer *w, const char *s);
/* sequence<octet>: u32 length + raw bytes. */
int zdw_put_seq_u8(zdw_writer *w, const unsigned char *data, size_t n);
/* raw bytes, no alignment (building block for generated codecs). */
int zdw_put_bytes(zdw_writer *w, const unsigned char *data, size_t n);

/* --- reader: aligned primitives --- */

int zdw_get_u8(zdw_reader *r, unsigned char *out);
int zdw_get_u16(zdw_reader *r, unsigned int *out);
int zdw_get_u32(zdw_reader *r, unsigned long *out);
int zdw_get_u64(zdw_reader *r, zdw_u64_t *out);
int zdw_get_bool(zdw_reader *r, int *out);
int zdw_get_f32(zdw_reader *r, float *out);
int zdw_get_f64(zdw_reader *r, double *out);
/* Reads a CDR string into `out` (cap bytes incl. NUL). Fails on overflow or a
 * missing terminator. */
int zdw_get_string(zdw_reader *r, char *out, size_t cap);
/* sequence<octet>: reads the u32 length, then min(len,cap) bytes into `out`;
 * *n receives the announced length (may exceed cap -> ZDW_E_OVERrun). */
int zdw_get_seq_u8(zdw_reader *r, unsigned char *out, size_t cap, size_t *n);
int zdw_get_bytes(zdw_reader *r, unsigned char *out, size_t n);

/* --- DHEADER (XCDR2 delimited, OMG XTypes 1.3 section 7.4.3.5) ---
 *
 * An `@appendable`/`@mutable` struct and a `sequence<T>`/`T[N]` with a
 * NON-primitive element type carry a 4-byte DHEADER: a u32 (4-aligned) whose
 * value is the byte length of the content that follows (the DHEADER itself
 * excluded). Emitted only under XCDR2. */

/* Reserves a 4-byte DHEADER slot (u32-aligned) and returns the body-start
 * position to pass back to zdw_dheader_end. */
size_t zdw_dheader_begin(zdw_writer *w);
/* Back-patches the DHEADER reserved by zdw_dheader_begin with the byte length
 * written since `body_start`. */
int zdw_dheader_end(zdw_writer *w, size_t body_start);
/* Reads a DHEADER; *len receives the announced body byte length. */
int zdw_dheader_read(zdw_reader *r, unsigned long *len);

/* --- EMHEADER (XCDR2 @mutable, OMG XTypes 1.3 section 7.4.3.4.2) ---
 *
 * A @mutable struct is a DHEADER-delimited block whose members each carry an
 * EMHEADER: a u32 = (must_understand << 31) + (length_code << 28) + member_id
 * (member_id is 28-bit). These helpers use length code LC4 (the universal
 * default the ZeroDDS codegen emits): EMHEADER + NEXTINT (u32 = member body
 * byte length) + body. Frame the whole @mutable struct with
 * zdw_dheader_begin/end around the members. */

/* Writes the LC4 EMHEADER for `member_id` and reserves the NEXTINT slot;
 * returns the body-start position for zdw_emheader_end. */
size_t zdw_emheader_begin(zdw_writer *w, unsigned long member_id,
                          int must_understand);
/* Back-patches the NEXTINT with the member body byte length. */
int zdw_emheader_end(zdw_writer *w, size_t body_start);
/* Reads an LC4 member header: *member_id / *must_understand from the EMHEADER,
 * *nextint = the following body byte length. */
int zdw_emheader_read(zdw_reader *r, unsigned long *member_id,
                      int *must_understand, unsigned long *nextint);

/* --- helpers --- */

/* Padding to push `pos` to the next multiple of `align` (a power of two). */
size_t zdw_padding_for(size_t pos, size_t align);
/* Builds a zdw_u64_t from a C `unsigned long` value (high half = 0). */
zdw_u64_t zdw_u64_from_ul(unsigned long v);

#ifdef __cplusplus
}
#endif

#endif /* ZERODDS_WIRE_H */
