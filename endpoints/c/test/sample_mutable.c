/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors */

#include "sample_mutable.h"

int mutable_encode(zdw_writer *w, const mutable_m *m)
{
    size_t bs, e;
    bs = zdw_dheader_begin(w);              /* @mutable struct DHEADER */
    e = zdw_emheader_begin(w, 10, 0); zdw_put_u32(w, m->x);    zdw_emheader_end(w, e);
    e = zdw_emheader_begin(w, 20, 0); zdw_put_string(w, m->s); zdw_emheader_end(w, e);
    e = zdw_emheader_begin(w, 30, 0); zdw_put_u16(w, m->k);    zdw_emheader_end(w, e);
    return zdw_dheader_end(w, bs);
}

int mutable_decode(zdw_reader *r, mutable_m *m)
{
    unsigned long dh = 0, id = 0, ni = 0;
    int mu = 0;
    size_t start;
    zdw_dheader_read(r, &dh);
    start = r->pos;
    /* @mutable: members in any order, dispatch by member-ID, skip unknown. */
    while (r->pos - start < (size_t)dh && r->error == ZDW_OK) {
        if (zdw_emheader_read(r, &id, &mu, &ni) != ZDW_OK) {
            break;
        }
        if (id == 10) {
            zdw_get_u32(r, &m->x);
        } else if (id == 20) {
            zdw_get_string(r, m->s, MUTABLE_S_CAP);
        } else if (id == 30) {
            zdw_get_u16(r, &m->k);
        } else {
            unsigned long j;
            unsigned char scratch;
            for (j = 0; j < ni; j++) {
                zdw_get_u8(r, &scratch);
            }
        }
    }
    return r->error;
}
