/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * A representative @final type + its fixed (static) XCDR2 codec. Stands in for
 * what the `wire-fixed` codegen will emit per IDL type (ADR 0013). Exercises
 * alignment (u32/u16/u8/f32/u64-capped-to-4), a string, and sequence<octet>.
 */

#ifndef SAMPLE_SENSOR_H
#define SAMPLE_SENSOR_H

#include "zerodds_wire.h"

#define SENSOR_LABEL_CAP 64
#define SENSOR_RAW_CAP   64

typedef struct {
    unsigned long id;                        /* uint32 */
    unsigned int  kind;                      /* uint16 */
    unsigned char flags;                     /* uint8  */
    float         value;                     /* float  */
    zdw_u64_t     stamp;                      /* uint64 */
    char          label[SENSOR_LABEL_CAP];   /* string */
    unsigned char raw[SENSOR_RAW_CAP];       /* sequence<octet> */
    size_t        raw_len;
} sensor_reading;

int sensor_encode(zdw_writer *w, const sensor_reading *s);
int sensor_decode(zdw_reader *r, sensor_reading *s);

#endif /* SAMPLE_SENSOR_H */
