/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors */

#include "zerodds_reflect.h"

static void encode_one(zdw_writer *w, const zdw_dyn_field *f)
{
    switch (f->kind) {
    case ZDW_K_U8:     zdw_put_u8(w, f->u8); break;
    case ZDW_K_U16:    zdw_put_u16(w, f->u16); break;
    case ZDW_K_U32:    zdw_put_u32(w, f->u32); break;
    case ZDW_K_U64:    zdw_put_u64(w, f->u64); break;
    case ZDW_K_F32:    zdw_put_f32(w, f->f32); break;
    case ZDW_K_F64:    zdw_put_f64(w, f->f64); break;
    case ZDW_K_BOOL:   zdw_put_bool(w, f->boolean); break;
    case ZDW_K_STRING: zdw_put_string(w, f->str); break;
    case ZDW_K_SEQ_U8: zdw_put_seq_u8(w, f->bytes, f->bytes_len); break;
    default: w->error = ZDW_E_INVALID; break;
    }
}

static void decode_one(zdw_reader *r, zdw_dyn_field *f)
{
    switch (f->kind) {
    case ZDW_K_U8:     zdw_get_u8(r, &f->u8); break;
    case ZDW_K_U16:    zdw_get_u16(r, &f->u16); break;
    case ZDW_K_U32:    zdw_get_u32(r, &f->u32); break;
    case ZDW_K_U64:    zdw_get_u64(r, &f->u64); break;
    case ZDW_K_F32:    zdw_get_f32(r, &f->f32); break;
    case ZDW_K_F64:    zdw_get_f64(r, &f->f64); break;
    case ZDW_K_BOOL:   zdw_get_bool(r, &f->boolean); break;
    case ZDW_K_STRING: zdw_get_string(r, f->str, f->str_cap); break;
    case ZDW_K_SEQ_U8: zdw_get_seq_u8(r, f->bytes, f->bytes_cap, &f->bytes_len); break;
    default: r->error = ZDW_E_INVALID; break;
    }
}

int zdw_reflect_encode(zdw_writer *w, const zdw_dyn_field *fields, size_t n)
{
    size_t i;
    for (i = 0; i < n; i++) {
        encode_one(w, &fields[i]);
        if (w->error != ZDW_OK) {
            return w->error;
        }
    }
    return ZDW_OK;
}

int zdw_reflect_decode(zdw_reader *r, zdw_dyn_field *fields, size_t n)
{
    size_t i;
    for (i = 0; i < n; i++) {
        decode_one(r, &fields[i]);
        if (r->error != ZDW_OK) {
            return r->error;
        }
    }
    return ZDW_OK;
}

int zdw_reflect_encode_ext(zdw_writer *w, zdw_ext ext,
                           const zdw_dyn_field *fields, const unsigned long *ids,
                           size_t n)
{
    size_t i, bs;
    switch (ext) {
    case ZDW_X_FINAL:
        return zdw_reflect_encode(w, fields, n);
    case ZDW_X_APPENDABLE:
        bs = zdw_dheader_begin(w);
        for (i = 0; i < n; i++) {
            encode_one(w, &fields[i]);
        }
        return zdw_dheader_end(w, bs);
    case ZDW_X_MUTABLE:
        bs = zdw_dheader_begin(w);
        for (i = 0; i < n; i++) {
            size_t es = zdw_emheader_begin(w, ids ? ids[i] : (unsigned long)i, 0);
            encode_one(w, &fields[i]);
            zdw_emheader_end(w, es);
        }
        return zdw_dheader_end(w, bs);
    default:
        w->error = ZDW_E_INVALID;
        return ZDW_E_INVALID;
    }
}

int zdw_reflect_decode_ext(zdw_reader *r, zdw_ext ext, zdw_dyn_field *fields,
                           size_t n)
{
    size_t i;
    unsigned long dh = 0;
    switch (ext) {
    case ZDW_X_FINAL:
        return zdw_reflect_decode(r, fields, n);
    case ZDW_X_APPENDABLE:
        zdw_dheader_read(r, &dh);
        return zdw_reflect_decode(r, fields, n);
    case ZDW_X_MUTABLE:
        zdw_dheader_read(r, &dh);
        for (i = 0; i < n; i++) {
            unsigned long id = 0, ni = 0;
            int mu = 0;
            zdw_emheader_read(r, &id, &mu, &ni); /* in wire order */
            decode_one(r, &fields[i]);
            if (r->error != ZDW_OK) {
                return r->error;
            }
        }
        return r->error;
    default:
        r->error = ZDW_E_INVALID;
        return ZDW_E_INVALID;
    }
}

static int encode_field(zdw_writer *w, const zdw_dyn_field *f);
static void decode_field(zdw_reader *r, zdw_dyn_field *f);

int zdw_reflect_encode_struct(zdw_writer *w, const zdw_dyn_struct *s)
{
    size_t i, bs;
    switch (s->ext) {
    case ZDW_X_FINAL:
        for (i = 0; i < s->n; i++) {
            encode_field(w, &s->fields[i]);
            if (w->error != ZDW_OK) { return w->error; }
        }
        return ZDW_OK;
    case ZDW_X_APPENDABLE:
        bs = zdw_dheader_begin(w);
        for (i = 0; i < s->n; i++) {
            encode_field(w, &s->fields[i]);
        }
        return zdw_dheader_end(w, bs);
    case ZDW_X_MUTABLE:
        bs = zdw_dheader_begin(w);
        for (i = 0; i < s->n; i++) {
            size_t es = zdw_emheader_begin(w, s->ids ? s->ids[i] : (unsigned long)i, 0);
            encode_field(w, &s->fields[i]);
            zdw_emheader_end(w, es);
        }
        return zdw_dheader_end(w, bs);
    default:
        w->error = ZDW_E_INVALID;
        return ZDW_E_INVALID;
    }
}

static int encode_field(zdw_writer *w, const zdw_dyn_field *f)
{
    if (f->kind == ZDW_K_NESTED) {
        return zdw_reflect_encode_struct(w, f->nested);
    }
    if (f->kind == ZDW_K_SEQ_STRUCT) {
        size_t i, cbs = zdw_dheader_begin(w);
        zdw_put_u32(w, (unsigned long)f->elems_len);
        for (i = 0; i < f->elems_len; i++) {
            zdw_reflect_encode_struct(w, &f->elems[i]);
        }
        return zdw_dheader_end(w, cbs);
    }
    encode_one(w, f);
    return w->error;
}

int zdw_reflect_decode_struct(zdw_reader *r, zdw_dyn_struct *s)
{
    size_t i;
    unsigned long dh = 0;
    switch (s->ext) {
    case ZDW_X_FINAL:
        for (i = 0; i < s->n; i++) {
            decode_field(r, &s->fields[i]);
            if (r->error != ZDW_OK) { return r->error; }
        }
        return ZDW_OK;
    case ZDW_X_APPENDABLE:
        zdw_dheader_read(r, &dh);
        for (i = 0; i < s->n; i++) {
            decode_field(r, &s->fields[i]);
            if (r->error != ZDW_OK) { return r->error; }
        }
        return ZDW_OK;
    case ZDW_X_MUTABLE:
        zdw_dheader_read(r, &dh);
        for (i = 0; i < s->n; i++) {
            unsigned long id = 0, ni = 0;
            int mu = 0;
            zdw_emheader_read(r, &id, &mu, &ni);
            decode_field(r, &s->fields[i]);
            if (r->error != ZDW_OK) { return r->error; }
        }
        return ZDW_OK;
    default:
        r->error = ZDW_E_INVALID;
        return ZDW_E_INVALID;
    }
}

static void decode_field(zdw_reader *r, zdw_dyn_field *f)
{
    if (f->kind == ZDW_K_NESTED) {
        zdw_reflect_decode_struct(r, f->nested);
    } else if (f->kind == ZDW_K_SEQ_STRUCT) {
        unsigned long dh = 0, c = 0;
        size_t i;
        zdw_dheader_read(r, &dh);
        zdw_get_u32(r, &c);
        f->elems_len = (size_t)c;
        for (i = 0; i < (size_t)c && i < f->elems_cap; i++) {
            zdw_reflect_decode_struct(r, &f->elems[i]);
        }
    } else {
        decode_one(r, f);
    }
}
