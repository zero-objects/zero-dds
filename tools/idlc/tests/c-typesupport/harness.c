/*
 * One sample of `t::Reading` through the generated TypeSupport table.
 *
 * Variant 0 is the sample `probe.json` describes; its XCDR2 octets and its
 * KeyHash are printed for the test to hold against the Rust wire probe.
 * Variant 1 takes the other union arms and leaves `note` out. For both:
 * the table's size probe and encode, a decode of the codec header's own
 * octets in either byte order, the decoded sample written again through
 * the table and compared octet for octet, a decode of a truncated buffer
 * refused, and every decoded sample released. Built with AddressSanitizer
 * where the compiler has it, so a walk that frees what it does not own,
 * or misses what it does, fails the run.
 *
 * MAIN_REPR and MAIN_ALIGN name the version the codec header was
 * generated for.
 */
#include "probe_typesupport.h"

#include <stdio.h>

static zd_string tags[3] = {"alpha", "beta", ""};
static StringInt32MapElement entries[2] = {{"one", 1}, {"two", 2}};
static int32_t points[4] = {1, -2, 3, -4};
static t_Node tail = {9, NULL};
static zd_string inner0[2] = {"x", "yz"};
static zd_string inner1[1] = {"\xc3\xbc"};
static zd_seq_zd_string nested[2] = {{2u, 2u, inner0, false}, {1u, 1u, inner1, false}};

static void fill(t_Reading *v, int variant) {
    memset(v, 0, sizeof *v);
    v->parent.sensor = "probe";
    v->parent.id = 42;
    v->parent.origin = "lab";
    v->text = "\xe6\x97\xa5\xe6\x9c\xac\xe8\xaa\x9e text";
    v->tags._maximum = 3u;
    v->tags._length = 3u;
    v->tags._buffer = tags;
    v->pair[0] = "left";
    v->pair[1] = "right";
    v->counts._maximum = 2u;
    v->counts._length = 2u;
    v->counts._buffer = entries;
    if (variant == 0) {
        v->outline._d = t_RED;
        v->outline._u.label = "square";
        v->tiny._d = 1;
        v->tiny._u.a = 7;
        v->note = "a note";
    } else {
        v->outline._d = t_GREEN;
        v->outline._u.points._maximum = 4u;
        v->outline._u.points._length = 4u;
        v->outline._u.points._buffer = points;
        v->tiny._d = 5;
        v->tiny._u.d = "the default arm";
        v->note = NULL;
    }
    v->chain.value = 1;
    v->chain.next = &tail;
    v->price = "123.45";
    v->nested._maximum = 2u;
    v->nested._length = 2u;
    v->nested._buffer = nested;
}

/* The table's octets for `v`, into `out`; the length, or 0 on a failure. */
static size_t encode(const t_Reading *v, uint8_t *out, size_t cap) {
    const zerodds_typesupport_t *ts = &t_Reading_typesupport;
    size_t needed = 0u;
    size_t written = 0u;
    if (ts->encode(v, NULL, 0u, &needed) != ZERODDS_TS_BUFFER_TOO_SMALL || needed == 0u) {
        fprintf(stderr, "the size probe did not answer BUFFER_TOO_SMALL with a length\n");
        return 0u;
    }
    if (needed > cap || ts->encode(v, out, cap, &written) != ZERODDS_TS_OK || written != needed) {
        fprintf(stderr, "the encode did not write the length the probe gave\n");
        return 0u;
    }
    return written;
}

static int run(int variant) {
    const zerodds_typesupport_t *ts = &t_Reading_typesupport;
    static uint8_t reference[4096];
    static uint8_t again[4096];
    static uint8_t main_octets[4096];
    t_Reading sample;
    t_Reading decoded;
    size_t length;
    size_t main_length;
    size_t again_length;
    int order;
    zdw_writer w;

    fill(&sample, variant);
    length = encode(&sample, reference, sizeof reference);
    if (length == 0u) {
        return 1;
    }
    if (variant == 0) {
        size_t i;
        uint8_t hash[16];
        printf("xcdr2 ");
        for (i = 0u; i < length; i++) {
            printf("%02x", reference[i]);
        }
        printf("\n");
        if (ts->key_hash == NULL || ts->key_hash(&sample, hash) != ZERODDS_TS_OK) {
            fprintf(stderr, "no KeyHash\n");
            return 1;
        }
        printf("keyhash ");
        for (i = 0u; i < 16u; i++) {
            printf("%02x", hash[i]);
        }
        printf("\n");
    }

    for (order = 0; order < 2; order++) {
        uint8_t representation = (uint8_t)MAIN_REPR;
        zdw_writer_init(&w, main_octets, sizeof main_octets, order == 0 ? ZDW_LITTLE : ZDW_BIG,
                        MAIN_ALIGN);
        zdw_writer_set_pl_cdr1(&w, zd_ts_framing);
        zd_enc_t_Reading(&w, &sample);
        if (zdw_writer_status(&w) != ZDW_OK) {
            fprintf(stderr, "the codec header did not encode the sample\n");
            return 1;
        }
        main_length = zdw_writer_length(&w);
        if (order == 1) {
            representation = (uint8_t)(representation | ZERODDS_TS_REPR_BIG_ENDIAN);
        }
        memset(&decoded, 0, sizeof decoded);
        if (ts->decode_repr(main_octets, main_length, representation, &decoded) != ZERODDS_TS_OK) {
            fprintf(stderr, "variant %d, order %d: the decode refused the octets\n", variant, order);
            return 1;
        }
        again_length = encode(&decoded, again, sizeof again);
        if (again_length != length || memcmp(again, reference, length) != 0) {
            fprintf(stderr, "variant %d, order %d: the decoded sample writes other octets\n",
                    variant, order);
            ts->sample_free(&decoded);
            return 1;
        }
        /* The sample owns what it decoded: [C-MAP] _maximum is the capacity and _release says
           the sequence frees its buffer. */
        if (decoded.tags._length != 3u || decoded.tags._maximum != 3u || !decoded.tags._release) {
            fprintf(stderr, "the decoded sequence does not own its buffer\n");
            ts->sample_free(&decoded);
            return 1;
        }
        ts->sample_free(&decoded);
        if (decoded.tags._maximum != 0u || decoded.tags._release) {
            fprintf(stderr, "sample_free left the capacity or the release flag behind\n");
            return 1;
        }
        if (decoded.parent.sensor != NULL || decoded.tags._buffer != NULL || decoded.chain.next != NULL) {
            fprintf(stderr, "sample_free left a pointer behind\n");
            return 1;
        }
    }

    memset(&decoded, 0, sizeof decoded);
    if (ts->decode_repr(reference, length - 3u, ZERODDS_TS_REPR_XCDR2, &decoded) == ZERODDS_TS_OK) {
        fprintf(stderr, "variant %d: a truncated sample decoded\n", variant);
        ts->sample_free(&decoded);
        return 1;
    }
    return 0;
}

int main(void) {
    if (run(0) != 0 || run(1) != 0) {
        return 1;
    }
    printf("OK\n");
    return 0;
}
