/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors */

#include "sample_nested.h"

static int inner_encode(zdw_writer *w, const inner_t *i)
{
    size_t bs = zdw_dheader_begin(w);
    zdw_put_u16(w, i->a);
    zdw_put_u32(w, i->b);
    return zdw_dheader_end(w, bs);
}

static int inner_decode(zdw_reader *r, inner_t *i)
{
    unsigned long dh = 0;
    zdw_dheader_read(r, &dh);
    zdw_get_u16(r, &i->a);
    zdw_get_u32(r, &i->b);
    return r->error;
}

int outer_encode(zdw_writer *w, const outer_t *o)
{
    size_t bs, cbs, k;
    bs = zdw_dheader_begin(w);          /* outer @appendable DHEADER */
    zdw_put_u32(w, o->id);
    inner_encode(w, &o->one);           /* nested @appendable */
    cbs = zdw_dheader_begin(w);         /* sequence<Inner> collection DHEADER */
    zdw_put_u32(w, (unsigned long)o->many_len);
    for (k = 0; k < o->many_len; k++) {
        inner_encode(w, &o->many[k]);
    }
    zdw_dheader_end(w, cbs);
    zdw_put_string(w, o->label);
    return zdw_dheader_end(w, bs);
}

int outer_decode(zdw_reader *r, outer_t *o)
{
    unsigned long dh = 0, count = 0, k;
    zdw_dheader_read(r, &dh);           /* outer */
    zdw_get_u32(r, &o->id);
    inner_decode(r, &o->one);
    zdw_dheader_read(r, &dh);           /* collection */
    zdw_get_u32(r, &count);
    o->many_len = (size_t)count;
    for (k = 0; k < count && k < NESTED_MANY_CAP; k++) {
        inner_decode(r, &o->many[k]);
    }
    zdw_get_string(r, o->label, NESTED_LABEL_CAP);
    return r->error;
}
