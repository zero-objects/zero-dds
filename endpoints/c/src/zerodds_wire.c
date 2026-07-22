/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * zerodds_wire.c -- ISO C89 implementation of the endpoint wire-core.
 * See zerodds_wire.h. No malloc, no C99, byte-by-byte, host-endian-safe.
 */

#include "zerodds_wire.h"

/* ------------------------------------------------------------------ */
/* helpers                                                            */
/* ------------------------------------------------------------------ */

size_t zdw_padding_for(size_t pos, size_t align)
{
    size_t mask = align - 1u;
    return (align - (pos & mask)) & mask;
}

zdw_u64_t zdw_u64_from_ul(unsigned long v)
{
    zdw_u64_t r;
    r.hi = 0u;
    r.lo = v;
    return r;
}

/* Returns nonzero if the host stores multi-byte scalars big-endian first. */
static int host_is_be(void)
{
    unsigned long x = 1u;
    unsigned char *p = (unsigned char *)&x;
    return p[0] == 0; /* first byte is MSB on BE, LSB on LE */
}

/* Reverses n bytes in place. */
static void reverse_bytes(unsigned char *b, size_t n)
{
    size_t i = 0, j = (n == 0 ? 0 : n - 1);
    while (i < j) {
        unsigned char t = b[i];
        b[i] = b[j];
        b[j] = t;
        i++;
        j--;
    }
}

/* Normalizes host-memory bytes of a scalar to logical little-endian order. */
static void host_bytes_to_le(unsigned char *b, size_t n)
{
    if (host_is_be()) {
        reverse_bytes(b, n);
    }
}

/* ------------------------------------------------------------------ */
/* writer                                                             */
/* ------------------------------------------------------------------ */

void zdw_writer_init(zdw_writer *w, unsigned char *buf, size_t cap, int endian)
{
    w->buf = buf;
    w->cap = cap;
    w->len = 0u;
    w->endian = endian;
    w->max_align = ZDW_XCDR2_MAX_ALIGN;
    w->error = ZDW_OK;
}

static int w_align(zdw_writer *w, size_t align)
{
    size_t eff = align;
    size_t pad;
    if (eff > (size_t)w->max_align) {
        eff = (size_t)w->max_align;
    }
    pad = zdw_padding_for(w->len, eff);
    if (w->len + pad > w->cap) {
        w->error = ZDW_E_OVERrun;
        return ZDW_E_OVERrun;
    }
    while (pad-- > 0u) {
        w->buf[w->len++] = 0;
    }
    return ZDW_OK;
}

int zdw_put_bytes(zdw_writer *w, const unsigned char *data, size_t n)
{
    size_t i;
    if (w->error != ZDW_OK) {
        return w->error;
    }
    if (w->len + n > w->cap) {
        w->error = ZDW_E_OVERrun;
        return ZDW_E_OVERrun;
    }
    for (i = 0; i < n; i++) {
        w->buf[w->len++] = data[i];
    }
    return ZDW_OK;
}

/* Writes `n` logical-LE bytes at the given alignment, in wire byte order. */
static int w_put_uint(zdw_writer *w, size_t align, const unsigned char *le, size_t n)
{
    unsigned char tmp[8];
    size_t i;
    if (w->error != ZDW_OK) {
        return w->error;
    }
    if (w_align(w, align) != ZDW_OK) {
        return w->error;
    }
    for (i = 0; i < n; i++) {
        tmp[i] = le[i];
    }
    if (w->endian == ZDW_BE) {
        reverse_bytes(tmp, n);
    }
    return zdw_put_bytes(w, tmp, n);
}

int zdw_put_u8(zdw_writer *w, unsigned char v)
{
    if (w->error != ZDW_OK) {
        return w->error;
    }
    return zdw_put_bytes(w, &v, 1);
}

int zdw_put_u16(zdw_writer *w, unsigned int v)
{
    unsigned char le[2];
    le[0] = (unsigned char)(v & 0xffu);
    le[1] = (unsigned char)((v >> 8) & 0xffu);
    return w_put_uint(w, 2, le, 2);
}

int zdw_put_u32(zdw_writer *w, unsigned long v)
{
    unsigned char le[4];
    le[0] = (unsigned char)(v & 0xffu);
    le[1] = (unsigned char)((v >> 8) & 0xffu);
    le[2] = (unsigned char)((v >> 16) & 0xffu);
    le[3] = (unsigned char)((v >> 24) & 0xffu);
    return w_put_uint(w, 4, le, 4);
}

int zdw_put_u64(zdw_writer *w, zdw_u64_t v)
{
    unsigned char le[8];
    le[0] = (unsigned char)(v.lo & 0xffu);
    le[1] = (unsigned char)((v.lo >> 8) & 0xffu);
    le[2] = (unsigned char)((v.lo >> 16) & 0xffu);
    le[3] = (unsigned char)((v.lo >> 24) & 0xffu);
    le[4] = (unsigned char)(v.hi & 0xffu);
    le[5] = (unsigned char)((v.hi >> 8) & 0xffu);
    le[6] = (unsigned char)((v.hi >> 16) & 0xffu);
    le[7] = (unsigned char)((v.hi >> 24) & 0xffu);
    return w_put_uint(w, 8, le, 8);
}

int zdw_put_bool(zdw_writer *w, int v)
{
    return zdw_put_u8(w, (unsigned char)(v ? 1 : 0));
}

int zdw_put_f32(zdw_writer *w, float v)
{
    union { float f; unsigned char b[4]; } u;
    u.f = v;
    host_bytes_to_le(u.b, 4);
    return w_put_uint(w, 4, u.b, 4);
}

int zdw_put_f64(zdw_writer *w, double v)
{
    union { double d; unsigned char b[8]; } u;
    u.d = v;
    host_bytes_to_le(u.b, 8);
    return w_put_uint(w, 8, u.b, 8);
}

int zdw_put_string(zdw_writer *w, const char *s)
{
    size_t n = 0;
    unsigned char nul = 0;
    while (s[n] != '\0') {
        n++;
    }
    if (zdw_put_u32(w, (unsigned long)(n + 1u)) != ZDW_OK) {
        return w->error;
    }
    if (zdw_put_bytes(w, (const unsigned char *)s, n) != ZDW_OK) {
        return w->error;
    }
    return zdw_put_bytes(w, &nul, 1);
}

int zdw_put_seq_u8(zdw_writer *w, const unsigned char *data, size_t n)
{
    if (zdw_put_u32(w, (unsigned long)n) != ZDW_OK) {
        return w->error;
    }
    return zdw_put_bytes(w, data, n);
}

size_t zdw_dheader_begin(zdw_writer *w)
{
    unsigned char zero4[4];
    zero4[0] = 0; zero4[1] = 0; zero4[2] = 0; zero4[3] = 0;
    /* A DHEADER is a u32: 4-align, then reserve 4 bytes to back-patch. */
    if (w_align(w, 4) != ZDW_OK) {
        return w->len;
    }
    zdw_put_bytes(w, zero4, 4);
    return w->len; /* body starts here; the DHEADER is the 4 bytes before it */
}

int zdw_dheader_end(zdw_writer *w, size_t body_start)
{
    unsigned long len;
    unsigned char le[4];
    unsigned char *dst;
    if (w->error != ZDW_OK) {
        return w->error;
    }
    len = (unsigned long)(w->len - body_start);
    le[0] = (unsigned char)(len & 0xffu);
    le[1] = (unsigned char)((len >> 8) & 0xffu);
    le[2] = (unsigned char)((len >> 16) & 0xffu);
    le[3] = (unsigned char)((len >> 24) & 0xffu);
    if (w->endian == ZDW_BE) {
        reverse_bytes(le, 4);
    }
    dst = w->buf + (body_start - 4u);
    dst[0] = le[0]; dst[1] = le[1]; dst[2] = le[2]; dst[3] = le[3];
    return ZDW_OK;
}

int zdw_dheader_read(zdw_reader *r, unsigned long *len)
{
    return zdw_get_u32(r, len);
}

size_t zdw_emheader_begin(zdw_writer *w, unsigned long member_id,
                          int must_understand)
{
    /* EMHEADER = (M << 31) + (LC4 << 28) + member_id (28-bit). */
    unsigned long em = (must_understand ? 0x80000000uL : 0uL)
                     + (4uL << 28)
                     + (member_id & 0x0FFFFFFFuL);
    if (zdw_put_u32(w, em) != ZDW_OK) {
        return w->len;
    }
    /* LC4 NEXTINT = member body byte length: reuse the DHEADER back-patch. */
    return zdw_dheader_begin(w);
}

int zdw_emheader_end(zdw_writer *w, size_t body_start)
{
    return zdw_dheader_end(w, body_start);
}

int zdw_emheader_read(zdw_reader *r, unsigned long *member_id,
                      int *must_understand, unsigned long *nextint)
{
    unsigned long em = 0;
    if (zdw_get_u32(r, &em) != ZDW_OK) {
        return r->error;
    }
    *must_understand = ((em & 0x80000000uL) != 0uL);
    *member_id = em & 0x0FFFFFFFuL;
    return zdw_get_u32(r, nextint);
}

/* ------------------------------------------------------------------ */
/* reader                                                             */
/* ------------------------------------------------------------------ */

void zdw_reader_init(zdw_reader *r, const unsigned char *buf, size_t len, int endian)
{
    r->buf = buf;
    r->len = len;
    r->pos = 0u;
    r->endian = endian;
    r->max_align = ZDW_XCDR2_MAX_ALIGN;
    r->error = ZDW_OK;
}

static int r_align(zdw_reader *r, size_t align)
{
    size_t eff = align;
    size_t pad;
    if (eff > (size_t)r->max_align) {
        eff = (size_t)r->max_align;
    }
    pad = zdw_padding_for(r->pos, eff);
    if (r->pos + pad > r->len) {
        r->error = ZDW_E_OVERrun;
        return ZDW_E_OVERrun;
    }
    r->pos += pad;
    return ZDW_OK;
}

int zdw_get_bytes(zdw_reader *r, unsigned char *out, size_t n)
{
    size_t i;
    if (r->error != ZDW_OK) {
        return r->error;
    }
    if (r->pos + n > r->len) {
        r->error = ZDW_E_OVERrun;
        return ZDW_E_OVERrun;
    }
    for (i = 0; i < n; i++) {
        out[i] = r->buf[r->pos++];
    }
    return ZDW_OK;
}

/* Reads `n` bytes at the given alignment into logical-LE order. */
static int r_get_uint(zdw_reader *r, size_t align, unsigned char *le, size_t n)
{
    if (r->error != ZDW_OK) {
        return r->error;
    }
    if (r_align(r, align) != ZDW_OK) {
        return r->error;
    }
    if (zdw_get_bytes(r, le, n) != ZDW_OK) {
        return r->error;
    }
    if (r->endian == ZDW_BE) {
        reverse_bytes(le, n);
    }
    return ZDW_OK;
}

int zdw_get_u8(zdw_reader *r, unsigned char *out)
{
    return zdw_get_bytes(r, out, 1);
}

int zdw_get_u16(zdw_reader *r, unsigned int *out)
{
    unsigned char le[2];
    if (r_get_uint(r, 2, le, 2) != ZDW_OK) {
        return r->error;
    }
    *out = (unsigned int)le[0] | ((unsigned int)le[1] << 8);
    return ZDW_OK;
}

int zdw_get_u32(zdw_reader *r, unsigned long *out)
{
    unsigned char le[4];
    if (r_get_uint(r, 4, le, 4) != ZDW_OK) {
        return r->error;
    }
    *out = (unsigned long)le[0] | ((unsigned long)le[1] << 8) |
           ((unsigned long)le[2] << 16) | ((unsigned long)le[3] << 24);
    return ZDW_OK;
}

int zdw_get_u64(zdw_reader *r, zdw_u64_t *out)
{
    unsigned char le[8];
    if (r_get_uint(r, 8, le, 8) != ZDW_OK) {
        return r->error;
    }
    out->lo = (unsigned long)le[0] | ((unsigned long)le[1] << 8) |
              ((unsigned long)le[2] << 16) | ((unsigned long)le[3] << 24);
    out->hi = (unsigned long)le[4] | ((unsigned long)le[5] << 8) |
              ((unsigned long)le[6] << 16) | ((unsigned long)le[7] << 24);
    return ZDW_OK;
}

int zdw_get_bool(zdw_reader *r, int *out)
{
    unsigned char v;
    if (zdw_get_u8(r, &v) != ZDW_OK) {
        return r->error;
    }
    *out = (v != 0);
    return ZDW_OK;
}

int zdw_get_f32(zdw_reader *r, float *out)
{
    union { float f; unsigned char b[4]; } u;
    if (r_get_uint(r, 4, u.b, 4) != ZDW_OK) {
        return r->error;
    }
    /* u.b is logical-LE; put back into host memory order. */
    host_bytes_to_le(u.b, 4); /* symmetric: reverses again on BE host */
    *out = u.f;
    return ZDW_OK;
}

int zdw_get_f64(zdw_reader *r, double *out)
{
    union { double d; unsigned char b[8]; } u;
    if (r_get_uint(r, 8, u.b, 8) != ZDW_OK) {
        return r->error;
    }
    host_bytes_to_le(u.b, 8);
    *out = u.d;
    return ZDW_OK;
}

int zdw_get_string(zdw_reader *r, char *out, size_t cap)
{
    unsigned long len = 0u;
    size_t body;
    if (zdw_get_u32(r, &len) != ZDW_OK) {
        return r->error;
    }
    if (len == 0u) {
        r->error = ZDW_E_INVALID; /* length must include the NUL */
        return ZDW_E_INVALID;
    }
    if (len > (unsigned long)cap) {
        r->error = ZDW_E_OVERrun;
        return ZDW_E_OVERrun;
    }
    if (zdw_get_bytes(r, (unsigned char *)out, (size_t)len) != ZDW_OK) {
        return r->error;
    }
    body = (size_t)len - 1u;
    if (out[body] != '\0') {
        r->error = ZDW_E_INVALID; /* missing NUL terminator */
        return ZDW_E_INVALID;
    }
    return ZDW_OK;
}

int zdw_get_seq_u8(zdw_reader *r, unsigned char *out, size_t cap, size_t *n)
{
    unsigned long len = 0u;
    if (zdw_get_u32(r, &len) != ZDW_OK) {
        return r->error;
    }
    *n = (size_t)len;
    if ((size_t)len > cap) {
        r->error = ZDW_E_OVERrun;
        return ZDW_E_OVERrun;
    }
    return zdw_get_bytes(r, out, (size_t)len);
}
