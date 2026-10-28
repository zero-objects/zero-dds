/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The oracle for `tests/oracle.rs`: GCC's own binary128.
 *
 * A crate that implements a format cannot check itself against its own
 * arithmetic. `__float128` on x86-64 GCC is a real binary128 with a
 * printer and a parser nobody here wrote, so it is the second opinion.
 *
 * Reads `patterns.txt` on stdin — one line per value, the 32-hex-digit
 * bit pattern and the decimal this crate printed for it — and writes one
 * line per value:
 *
 *     <pattern> <what quadmath prints> <what quadmath reads our text as>
 *
 * Column two checks our *parser* against GCC's printer; column three
 * checks our *printer* against GCC's parser. A crate that agreed with
 * itself in both directions and with GCC in neither would pass every
 * test in `binary128.rs` and be wrong on the wire.
 *
 *   gcc -O2 -o quad_oracle quad_oracle.c -lquadmath
 *   ./quad_oracle < patterns.txt > x86_64-gcc-14.2.txt
 *
 * Requires a host whose GCC has `__float128` and libquadmath, which on
 * this project's fleet means the Linux test host and not the arm64 Macs.
 */
#include <quadmath.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    char line[4096];
    while (fgets(line, sizeof line, stdin) != NULL) {
        char pattern[64];
        char text[3072];
        if (sscanf(line, "%63s %3071s", pattern, text) != 2) {
            continue;
        }

        /* The pattern, as GCC sees the same sixteen bytes. */
        unsigned __int128 bits = 0;
        for (const char *p = pattern; *p != '\0'; p++) {
            unsigned digit;
            if (*p >= '0' && *p <= '9') {
                digit = (unsigned) (*p - '0');
            } else if (*p >= 'a' && *p <= 'f') {
                digit = (unsigned) (*p - 'a') + 10u;
            } else {
                digit = 0u;
            }
            bits = (bits << 4) | digit;
        }
        __float128 value;
        memcpy(&value, &bits, sizeof value);

        /* Thirty-six digits after the point is thirty-seven
         * significant, one more than binary128 needs to round-trip, so
         * our parser has no room to be right by luck. */
        char printed[256];
        quadmath_snprintf(printed, sizeof printed, "%.36Qe", value);

        /* And GCC's parser on our text, back as the same sixteen bytes. */
        __float128 reparsed = strtoflt128(text, NULL);
        unsigned __int128 back;
        memcpy(&back, &reparsed, sizeof back);
        unsigned long long high = (unsigned long long) (back >> 64);
        unsigned long long low = (unsigned long long) back;

        printf("%s %s %016llx%016llx\n", pattern, printed, high, low);
    }
    return 0;
}
