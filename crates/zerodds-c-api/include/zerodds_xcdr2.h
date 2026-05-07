/*
 * zerodds_xcdr2.h — C-FFI XCDR2-TypeSupport (Vendor-Spec
 *                   `zerodds-xcdr2-c-1.0`).
 *
 * Hand-maintained C99 header (NICHT cbindgen-generiert). Wird von
 * Codegen-Output-Headern (idl-cpp c_mode) ueber `#include` gezogen.
 *
 * Definiert:
 *   - `zerodds_typesupport_t` Function-Table-Struct.
 *   - FFI-Function-Prototypen (`zerodds_topic_create_typed`,
 *     `zerodds_writer_write_typed`, `zerodds_reader_take_typed`,
 *     `zerodds_xcdr2_encode/decode`).
 *   - Inline-Helper fuer Codegen-Encoder/Decoder (write_iN/uN/fN,
 *     write_string, read_iN/uN/fN, read_string, grow, put_u32_at,
 *     compute_key_hash).
 *
 * Memory-Ownership: siehe Vendor-Spec §6.
 *
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 */

#ifndef ZERODDS_XCDR2_H
#define ZERODDS_XCDR2_H

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ========================================================================
 * Vendor-Spec §2 — TypeSupport-Tabelle.
 * ====================================================================== */

typedef int (*zerodds_encode_fn)(const void* sample,
                                 uint8_t* out_buf,
                                 size_t out_cap,
                                 size_t* out_len);

typedef int (*zerodds_decode_fn)(const uint8_t* buf,
                                 size_t len,
                                 void* out_sample);

typedef int (*zerodds_key_hash_fn)(const void* sample, uint8_t out_hash[16]);

typedef void (*zerodds_sample_free_fn)(void* sample);

typedef struct zerodds_typesupport_s {
    uint8_t                 type_hash[16];
    const char*             type_name;
    uint8_t                 is_keyed;
    uint8_t                 extensibility; /* 0=Final, 1=Appendable, 2=Mutable */
    uint8_t                 _reserved[6];
    zerodds_encode_fn       encode;
    zerodds_decode_fn       decode;
    zerodds_key_hash_fn     key_hash;
    zerodds_sample_free_fn  sample_free;
} zerodds_typesupport_t;

/* ========================================================================
 * Vendor-Spec §3 — FFI-Funktionen (Pointer-Forward, weil cbindgen die
 * Strings nicht emittiert).
 * ====================================================================== */

struct zerodds_ZeroDdsRuntime;
struct zerodds_ZeroDdsWriter;
struct zerodds_ZeroDdsReader;
struct ZeroDdsTopic;

int zerodds_topic_create_typed(struct zerodds_ZeroDdsRuntime* participant,
                               const char* topic_name,
                               const zerodds_typesupport_t* type_support,
                               struct ZeroDdsTopic** out_topic);

void zerodds_topic_destroy_typed(struct ZeroDdsTopic* topic);

int zerodds_writer_write_typed(struct zerodds_ZeroDdsWriter* w,
                               const zerodds_typesupport_t* ts,
                               const void* sample);

int zerodds_reader_take_typed(struct zerodds_ZeroDdsReader* r,
                              const zerodds_typesupport_t* ts,
                              void* out_sample,
                              void* out_info);

int zerodds_xcdr2_encode(const zerodds_typesupport_t* ts,
                         const void* sample,
                         uint8_t* out_buf,
                         size_t out_cap,
                         size_t* out_len);

int zerodds_xcdr2_decode(const zerodds_typesupport_t* ts,
                         const uint8_t* buf,
                         size_t len,
                         void* out_sample);

/* ========================================================================
 * Inline-Helper fuer Codegen-Encoder/Decoder.
 *
 * Konventionen:
 *   - Buffer ist `(uint8_t** buf, size_t* len, size_t* cap)`-Triple,
 *     dynamisch wachsend via `realloc`.
 *   - Returns 0 = OK, !=0 = OOM oder Fehler.
 *   - Bytes werden in **Little-Endian** geschrieben (XCDR2 LE default).
 *   - Alignment: `pad_to(align)` springt auf `align`-Byte-Grenze
 *     relativ zum Buffer-Start (XTypes §7.4.1.5).
 * ====================================================================== */

static inline int zerodds_xcdr2_c_grow(uint8_t** buf, size_t* cap, size_t need) {
    if (*cap >= need) return 0;
    size_t new_cap = *cap < 16 ? 16 : *cap * 2;
    while (new_cap < need) new_cap *= 2;
    uint8_t* p = (uint8_t*)realloc(*buf, new_cap);
    if (p == NULL) return -1;
    *buf = p;
    *cap = new_cap;
    return 0;
}

static inline int zerodds_xcdr2_c_pad_to(uint8_t** buf, size_t* len, size_t* cap, size_t align) {
    size_t pad = (align - (*len % align)) % align;
    if (zerodds_xcdr2_c_grow(buf, cap, *len + pad) != 0) return -1;
    for (size_t i = 0; i < pad; ++i) (*buf)[(*len)++] = 0;
    return 0;
}

static inline int zerodds_xcdr2_c_write_u8(uint8_t** buf, size_t* len, size_t* cap, uint8_t v) {
    if (zerodds_xcdr2_c_grow(buf, cap, *len + 1) != 0) return -1;
    (*buf)[(*len)++] = v;
    return 0;
}

static inline int zerodds_xcdr2_c_write_i8(uint8_t** buf, size_t* len, size_t* cap, int8_t v) {
    return zerodds_xcdr2_c_write_u8(buf, len, cap, (uint8_t)v);
}

static inline int zerodds_xcdr2_c_write_u16(uint8_t** buf, size_t* len, size_t* cap, uint16_t v) {
    if (zerodds_xcdr2_c_pad_to(buf, len, cap, 2) != 0) return -1;
    if (zerodds_xcdr2_c_grow(buf, cap, *len + 2) != 0) return -1;
    (*buf)[(*len)++] = (uint8_t)(v & 0xFFu);
    (*buf)[(*len)++] = (uint8_t)((v >> 8) & 0xFFu);
    return 0;
}

static inline int zerodds_xcdr2_c_write_i16(uint8_t** buf, size_t* len, size_t* cap, int16_t v) {
    return zerodds_xcdr2_c_write_u16(buf, len, cap, (uint16_t)v);
}

static inline int zerodds_xcdr2_c_write_u32(uint8_t** buf, size_t* len, size_t* cap, uint32_t v) {
    if (zerodds_xcdr2_c_pad_to(buf, len, cap, 4) != 0) return -1;
    if (zerodds_xcdr2_c_grow(buf, cap, *len + 4) != 0) return -1;
    (*buf)[(*len)++] = (uint8_t)(v & 0xFFu);
    (*buf)[(*len)++] = (uint8_t)((v >> 8) & 0xFFu);
    (*buf)[(*len)++] = (uint8_t)((v >> 16) & 0xFFu);
    (*buf)[(*len)++] = (uint8_t)((v >> 24) & 0xFFu);
    return 0;
}

static inline int zerodds_xcdr2_c_write_i32(uint8_t** buf, size_t* len, size_t* cap, int32_t v) {
    return zerodds_xcdr2_c_write_u32(buf, len, cap, (uint32_t)v);
}

static inline int zerodds_xcdr2_c_write_u64(uint8_t** buf, size_t* len, size_t* cap, uint64_t v) {
    if (zerodds_xcdr2_c_pad_to(buf, len, cap, 8) != 0) return -1;
    if (zerodds_xcdr2_c_grow(buf, cap, *len + 8) != 0) return -1;
    for (int i = 0; i < 8; ++i) {
        (*buf)[(*len)++] = (uint8_t)((v >> (8 * i)) & 0xFFu);
    }
    return 0;
}

static inline int zerodds_xcdr2_c_write_i64(uint8_t** buf, size_t* len, size_t* cap, int64_t v) {
    return zerodds_xcdr2_c_write_u64(buf, len, cap, (uint64_t)v);
}

static inline int zerodds_xcdr2_c_write_f32(uint8_t** buf, size_t* len, size_t* cap, float v) {
    uint32_t u;
    memcpy(&u, &v, sizeof(u));
    return zerodds_xcdr2_c_write_u32(buf, len, cap, u);
}

static inline int zerodds_xcdr2_c_write_f64(uint8_t** buf, size_t* len, size_t* cap, double v) {
    uint64_t u;
    memcpy(&u, &v, sizeof(u));
    return zerodds_xcdr2_c_write_u64(buf, len, cap, u);
}

static inline int zerodds_xcdr2_c_write_string(uint8_t** buf, size_t* len, size_t* cap, const char* s) {
    size_t n = (s == NULL) ? 0 : strlen(s);
    if (zerodds_xcdr2_c_write_u32(buf, len, cap, (uint32_t)(n + 1)) != 0) return -1;
    if (zerodds_xcdr2_c_grow(buf, cap, *len + n + 1) != 0) return -1;
    if (n > 0) memcpy(*buf + *len, s, n);
    *len += n;
    (*buf)[(*len)++] = 0;
    return 0;
}

static inline void zerodds_xcdr2_c_put_u32_at(uint8_t* buf, size_t pos, uint32_t v) {
    buf[pos]     = (uint8_t)(v & 0xFFu);
    buf[pos + 1] = (uint8_t)((v >> 8) & 0xFFu);
    buf[pos + 2] = (uint8_t)((v >> 16) & 0xFFu);
    buf[pos + 3] = (uint8_t)((v >> 24) & 0xFFu);
}

/* ---- Reader-Helpers ---- */

static inline int zerodds_xcdr2_c_pad_read(const uint8_t* buf, size_t len, size_t* pos, size_t align) {
    (void)buf;
    size_t pad = (align - (*pos % align)) % align;
    if (*pos + pad > len) return -1;
    *pos += pad;
    return 0;
}

static inline int zerodds_xcdr2_c_read_u8(const uint8_t* buf, size_t len, size_t* pos, uint8_t* out) {
    if (*pos + 1 > len) return -1;
    *out = buf[(*pos)++];
    return 0;
}

static inline int zerodds_xcdr2_c_read_i8(const uint8_t* buf, size_t len, size_t* pos, int8_t* out) {
    uint8_t u; int rc = zerodds_xcdr2_c_read_u8(buf, len, pos, &u);
    if (rc != 0) return rc;
    *out = (int8_t)u;
    return 0;
}

static inline int zerodds_xcdr2_c_read_u16(const uint8_t* buf, size_t len, size_t* pos, uint16_t* out) {
    if (zerodds_xcdr2_c_pad_read(buf, len, pos, 2) != 0) return -1;
    if (*pos + 2 > len) return -1;
    *out = (uint16_t)buf[*pos] | ((uint16_t)buf[*pos + 1] << 8);
    *pos += 2;
    return 0;
}

static inline int zerodds_xcdr2_c_read_i16(const uint8_t* buf, size_t len, size_t* pos, int16_t* out) {
    uint16_t u; int rc = zerodds_xcdr2_c_read_u16(buf, len, pos, &u);
    if (rc != 0) return rc;
    *out = (int16_t)u;
    return 0;
}

static inline int zerodds_xcdr2_c_read_u32(const uint8_t* buf, size_t len, size_t* pos, uint32_t* out) {
    if (zerodds_xcdr2_c_pad_read(buf, len, pos, 4) != 0) return -1;
    if (*pos + 4 > len) return -1;
    *out = (uint32_t)buf[*pos]
         | ((uint32_t)buf[*pos + 1] << 8)
         | ((uint32_t)buf[*pos + 2] << 16)
         | ((uint32_t)buf[*pos + 3] << 24);
    *pos += 4;
    return 0;
}

static inline int zerodds_xcdr2_c_read_i32(const uint8_t* buf, size_t len, size_t* pos, int32_t* out) {
    uint32_t u; int rc = zerodds_xcdr2_c_read_u32(buf, len, pos, &u);
    if (rc != 0) return rc;
    *out = (int32_t)u;
    return 0;
}

static inline int zerodds_xcdr2_c_read_u64(const uint8_t* buf, size_t len, size_t* pos, uint64_t* out) {
    if (zerodds_xcdr2_c_pad_read(buf, len, pos, 8) != 0) return -1;
    if (*pos + 8 > len) return -1;
    uint64_t v = 0;
    for (int i = 0; i < 8; ++i) {
        v |= (uint64_t)buf[*pos + i] << (8 * i);
    }
    *out = v;
    *pos += 8;
    return 0;
}

static inline int zerodds_xcdr2_c_read_i64(const uint8_t* buf, size_t len, size_t* pos, int64_t* out) {
    uint64_t u; int rc = zerodds_xcdr2_c_read_u64(buf, len, pos, &u);
    if (rc != 0) return rc;
    *out = (int64_t)u;
    return 0;
}

static inline int zerodds_xcdr2_c_read_f32(const uint8_t* buf, size_t len, size_t* pos, float* out) {
    uint32_t u; int rc = zerodds_xcdr2_c_read_u32(buf, len, pos, &u);
    if (rc != 0) return rc;
    memcpy(out, &u, sizeof(*out));
    return 0;
}

static inline int zerodds_xcdr2_c_read_f64(const uint8_t* buf, size_t len, size_t* pos, double* out) {
    uint64_t u; int rc = zerodds_xcdr2_c_read_u64(buf, len, pos, &u);
    if (rc != 0) return rc;
    memcpy(out, &u, sizeof(*out));
    return 0;
}

static inline int zerodds_xcdr2_c_read_string(const uint8_t* buf, size_t len, size_t* pos, char** out) {
    uint32_t n;
    if (zerodds_xcdr2_c_read_u32(buf, len, pos, &n) != 0) return -1;
    if (n == 0 || *pos + n > len) return -1;
    char* s = (char*)malloc(n);
    if (s == NULL) return -1;
    memcpy(s, buf + *pos, n);
    s[n - 1] = 0; /* NUL guarantee */
    *pos += n;
    *out = s;
    return 0;
}

/* ========================================================================
 * Key-Hash-Helpers.
 *
 * `kh_write_*` schreiben **Big-Endian** (PlainCdr2BeKeyHolder, XTypes
 * §7.6.8.3). `compute_key_hash` zero-padded oder MD5'd auf 16 Byte.
 * ====================================================================== */

static inline int zerodds_xcdr2_c_kh_pad(uint8_t** buf, size_t* len, size_t* cap, size_t align) {
    /* PLAIN_CDR2 max-Align = 4 (XTypes §7.4.2.5.4). */
    if (align > 4) align = 4;
    size_t pad = (align - (*len % align)) % align;
    if (zerodds_xcdr2_c_grow(buf, cap, *len + pad) != 0) return -1;
    for (size_t i = 0; i < pad; ++i) (*buf)[(*len)++] = 0;
    return 0;
}

static inline int zerodds_xcdr2_c_kh_write_u8(uint8_t** buf, size_t* len, size_t* cap, uint8_t v) {
    if (zerodds_xcdr2_c_grow(buf, cap, *len + 1) != 0) return -1;
    (*buf)[(*len)++] = v;
    return 0;
}

static inline int zerodds_xcdr2_c_kh_write_i8(uint8_t** buf, size_t* len, size_t* cap, int8_t v) {
    return zerodds_xcdr2_c_kh_write_u8(buf, len, cap, (uint8_t)v);
}

static inline int zerodds_xcdr2_c_kh_write_u16(uint8_t** buf, size_t* len, size_t* cap, uint16_t v) {
    if (zerodds_xcdr2_c_kh_pad(buf, len, cap, 2) != 0) return -1;
    if (zerodds_xcdr2_c_grow(buf, cap, *len + 2) != 0) return -1;
    (*buf)[(*len)++] = (uint8_t)((v >> 8) & 0xFFu);
    (*buf)[(*len)++] = (uint8_t)(v & 0xFFu);
    return 0;
}

static inline int zerodds_xcdr2_c_kh_write_i16(uint8_t** buf, size_t* len, size_t* cap, int16_t v) {
    return zerodds_xcdr2_c_kh_write_u16(buf, len, cap, (uint16_t)v);
}

static inline int zerodds_xcdr2_c_kh_write_u32(uint8_t** buf, size_t* len, size_t* cap, uint32_t v) {
    if (zerodds_xcdr2_c_kh_pad(buf, len, cap, 4) != 0) return -1;
    if (zerodds_xcdr2_c_grow(buf, cap, *len + 4) != 0) return -1;
    (*buf)[(*len)++] = (uint8_t)((v >> 24) & 0xFFu);
    (*buf)[(*len)++] = (uint8_t)((v >> 16) & 0xFFu);
    (*buf)[(*len)++] = (uint8_t)((v >> 8) & 0xFFu);
    (*buf)[(*len)++] = (uint8_t)(v & 0xFFu);
    return 0;
}

static inline int zerodds_xcdr2_c_kh_write_i32(uint8_t** buf, size_t* len, size_t* cap, int32_t v) {
    return zerodds_xcdr2_c_kh_write_u32(buf, len, cap, (uint32_t)v);
}

static inline int zerodds_xcdr2_c_kh_write_u64(uint8_t** buf, size_t* len, size_t* cap, uint64_t v) {
    /* PLAIN_CDR2 max-Align = 4. */
    if (zerodds_xcdr2_c_kh_pad(buf, len, cap, 4) != 0) return -1;
    if (zerodds_xcdr2_c_grow(buf, cap, *len + 8) != 0) return -1;
    for (int i = 7; i >= 0; --i) {
        (*buf)[(*len)++] = (uint8_t)((v >> (8 * i)) & 0xFFu);
    }
    return 0;
}

static inline int zerodds_xcdr2_c_kh_write_i64(uint8_t** buf, size_t* len, size_t* cap, int64_t v) {
    return zerodds_xcdr2_c_kh_write_u64(buf, len, cap, (uint64_t)v);
}

static inline int zerodds_xcdr2_c_kh_write_f32(uint8_t** buf, size_t* len, size_t* cap, float v) {
    uint32_t u; memcpy(&u, &v, sizeof(u));
    return zerodds_xcdr2_c_kh_write_u32(buf, len, cap, u);
}

static inline int zerodds_xcdr2_c_kh_write_f64(uint8_t** buf, size_t* len, size_t* cap, double v) {
    uint64_t u; memcpy(&u, &v, sizeof(u));
    return zerodds_xcdr2_c_kh_write_u64(buf, len, cap, u);
}

static inline int zerodds_xcdr2_c_kh_write_string(uint8_t** buf, size_t* len, size_t* cap, const char* s) {
    /* String-Members in Key-Hash: laenge + bytes + NUL (BE). */
    size_t n = (s == NULL) ? 0 : strlen(s);
    if (zerodds_xcdr2_c_kh_write_u32(buf, len, cap, (uint32_t)(n + 1)) != 0) return -1;
    if (zerodds_xcdr2_c_grow(buf, cap, *len + n + 1) != 0) return -1;
    if (n > 0) memcpy(*buf + *len, s, n);
    *len += n;
    (*buf)[(*len)++] = 0;
    return 0;
}

/* MD5 (RFC 1321) — kompakte freie Implementation, wird nur fuer
 * Key-Hash-Fallback genutzt (XTypes §7.6.8 Step 5.2).
 *
 * Konstanten + Algorithmus aus dem RFC; keine externe Lib um die
 * C99-FFI ohne weitere Build-Abhaengigkeiten kompilierbar zu halten.
 */
static inline uint32_t zerodds_md5_lr(uint32_t x, uint32_t n) { return (x << n) | (x >> (32 - n)); }

static inline void zerodds_xcdr2_c_md5(const uint8_t* msg, size_t msg_len, uint8_t out[16]) {
    static const uint32_t K[64] = {
        0xd76aa478,0xe8c7b756,0x242070db,0xc1bdceee,0xf57c0faf,0x4787c62a,0xa8304613,0xfd469501,
        0x698098d8,0x8b44f7af,0xffff5bb1,0x895cd7be,0x6b901122,0xfd987193,0xa679438e,0x49b40821,
        0xf61e2562,0xc040b340,0x265e5a51,0xe9b6c7aa,0xd62f105d,0x02441453,0xd8a1e681,0xe7d3fbc8,
        0x21e1cde6,0xc33707d6,0xf4d50d87,0x455a14ed,0xa9e3e905,0xfcefa3f8,0x676f02d9,0x8d2a4c8a,
        0xfffa3942,0x8771f681,0x6d9d6122,0xfde5380c,0xa4beea44,0x4bdecfa9,0xf6bb4b60,0xbebfbc70,
        0x289b7ec6,0xeaa127fa,0xd4ef3085,0x04881d05,0xd9d4d039,0xe6db99e5,0x1fa27cf8,0xc4ac5665,
        0xf4292244,0x432aff97,0xab9423a7,0xfc93a039,0x655b59c3,0x8f0ccc92,0xffeff47d,0x85845dd1,
        0x6fa87e4f,0xfe2ce6e0,0xa3014314,0x4e0811a1,0xf7537e82,0xbd3af235,0x2ad7d2bb,0xeb86d391
    };
    static const uint32_t S[64] = {
        7,12,17,22, 7,12,17,22, 7,12,17,22, 7,12,17,22,
        5, 9,14,20, 5, 9,14,20, 5, 9,14,20, 5, 9,14,20,
        4,11,16,23, 4,11,16,23, 4,11,16,23, 4,11,16,23,
        6,10,15,21, 6,10,15,21, 6,10,15,21, 6,10,15,21
    };
    uint32_t a0 = 0x67452301, b0 = 0xefcdab89, c0 = 0x98badcfe, d0 = 0x10325476;
    /* Padding: 1-bit + 0..k zeros + 64-bit length. */
    size_t new_len = msg_len + 1;
    while (new_len % 64 != 56) ++new_len;
    new_len += 8;
    uint8_t* m = (uint8_t*)calloc(new_len, 1);
    if (m == NULL) { memset(out, 0, 16); return; }
    memcpy(m, msg, msg_len);
    m[msg_len] = 0x80;
    uint64_t bit_len = (uint64_t)msg_len * 8u;
    for (int i = 0; i < 8; ++i) m[new_len - 8 + i] = (uint8_t)((bit_len >> (8 * i)) & 0xFFu);
    for (size_t off = 0; off < new_len; off += 64) {
        uint32_t M[16];
        for (int i = 0; i < 16; ++i) {
            M[i] = (uint32_t)m[off + i*4]
                 | ((uint32_t)m[off + i*4 + 1] << 8)
                 | ((uint32_t)m[off + i*4 + 2] << 16)
                 | ((uint32_t)m[off + i*4 + 3] << 24);
        }
        uint32_t A = a0, B = b0, C = c0, D = d0;
        for (int i = 0; i < 64; ++i) {
            uint32_t F, g;
            if (i < 16)      { F = (B & C) | (~B & D);            g = (uint32_t)i; }
            else if (i < 32) { F = (D & B) | (~D & C);            g = (uint32_t)((5*i + 1) % 16); }
            else if (i < 48) { F = B ^ C ^ D;                     g = (uint32_t)((3*i + 5) % 16); }
            else             { F = C ^ (B | ~D);                  g = (uint32_t)((7*i) % 16); }
            uint32_t T = D;
            D = C;
            C = B;
            B = B + zerodds_md5_lr(A + F + K[i] + M[g], S[i]);
            A = T;
        }
        a0 += A; b0 += B; c0 += C; d0 += D;
    }
    free(m);
    uint32_t H[4] = {a0, b0, c0, d0};
    for (int i = 0; i < 4; ++i) {
        out[i*4]     = (uint8_t)(H[i] & 0xFFu);
        out[i*4 + 1] = (uint8_t)((H[i] >> 8) & 0xFFu);
        out[i*4 + 2] = (uint8_t)((H[i] >> 16) & 0xFFu);
        out[i*4 + 3] = (uint8_t)((H[i] >> 24) & 0xFFu);
    }
}

static inline void zerodds_xcdr2_c_compute_key_hash(const uint8_t* kh_buf, size_t kh_len,
                                                    size_t max_size_static,
                                                    uint8_t out_hash[16]) {
    if (max_size_static != 0 && max_size_static <= 16) {
        memset(out_hash, 0, 16);
        size_t n = kh_len < 16 ? kh_len : 16;
        if (n > 0) memcpy(out_hash, kh_buf, n);
        return;
    }
    /* Default-Pfad fuer rc1: < 16 byte → zero-pad, sonst MD5. */
    if (kh_len <= 16) {
        memset(out_hash, 0, 16);
        if (kh_len > 0) memcpy(out_hash, kh_buf, kh_len);
        return;
    }
    zerodds_xcdr2_c_md5(kh_buf, kh_len, out_hash);
}

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* ZERODDS_XCDR2_H */
