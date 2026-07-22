/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors */

#include "sample_sensor.h"

int sensor_encode(zdw_writer *w, const sensor_reading *s)
{
    zdw_put_u32(w, s->id);
    zdw_put_u16(w, s->kind);
    zdw_put_u8(w, s->flags);
    zdw_put_f32(w, s->value);
    zdw_put_u64(w, s->stamp);
    zdw_put_string(w, s->label);
    zdw_put_seq_u8(w, s->raw, s->raw_len);
    return w->error;
}

int sensor_decode(zdw_reader *r, sensor_reading *s)
{
    zdw_get_u32(r, &s->id);
    zdw_get_u16(r, &s->kind);
    zdw_get_u8(r, &s->flags);
    zdw_get_f32(r, &s->value);
    zdw_get_u64(r, &s->stamp);
    zdw_get_string(r, s->label, SENSOR_LABEL_CAP);
    zdw_get_seq_u8(r, s->raw, SENSOR_RAW_CAP, &s->raw_len);
    return r->error;
}
