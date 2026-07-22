/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * @mutable type + its fixed XCDR2 codec (ADR 0013). Exercises the EMHEADER
 * path (LC4): a DHEADER-delimited struct whose members carry member-IDs.
 * Mirrors endpoints/golden-gen encode_mutable.
 */

#ifndef SAMPLE_MUTABLE_H
#define SAMPLE_MUTABLE_H

#include "zerodds_wire.h"

#define MUTABLE_S_CAP 64

typedef struct {         /* @mutable */
    unsigned long x;     /* @id(10) uint32 */
    char          s[MUTABLE_S_CAP]; /* @id(20) string */
    unsigned int  k;     /* @id(30) uint16 */
} mutable_m;

int mutable_encode(zdw_writer *w, const mutable_m *m);
int mutable_decode(zdw_reader *r, mutable_m *m);

#endif /* SAMPLE_MUTABLE_H */
