/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * @appendable nested type + its fixed XCDR2 codec (ADR 0013). Exercises the
 * DHEADER path: a nested @appendable struct and a sequence<@appendable>.
 * Mirrors endpoints/golden-gen encode_nested.
 */

#ifndef SAMPLE_NESTED_H
#define SAMPLE_NESTED_H

#include "zerodds_wire.h"

#define NESTED_MANY_CAP  8
#define NESTED_LABEL_CAP 64

typedef struct {         /* @appendable */
    unsigned int  a;     /* uint16 */
    unsigned long b;     /* uint32 */
} inner_t;

typedef struct {         /* @appendable */
    unsigned long id;                    /* uint32 */
    inner_t       one;                   /* nested @appendable */
    inner_t       many[NESTED_MANY_CAP]; /* sequence<Inner> */
    size_t        many_len;
    char          label[NESTED_LABEL_CAP];
} outer_t;

int outer_encode(zdw_writer *w, const outer_t *o);
int outer_decode(zdw_reader *r, outer_t *o);

#endif /* SAMPLE_NESTED_H */
