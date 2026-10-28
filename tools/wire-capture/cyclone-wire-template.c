/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Cyclone DDS over RTPS/UDP: a real `dds_write` through a real
 * `dds_entity_t` writer, so what a capture holds is what the product
 * sends and not what its serialisation entry point returns.
 *
 * `wire-golden.sh` substitutes @TYPE@ and compiles this against
 * `idlc`'s output, twice into one binary: `probe pub` and `probe sub`.
 *
 * ## The alternation
 *
 * The writer sends `poison, sample, poison, sample, …` through **one**
 * writer, and the poison sample is the longer of the two wherever the
 * type has a variable-length member. A fill span of the short sample
 * then lands on octets the long one wrote, so a `00` in the capture is
 * an octet Cyclone wrote and not an octet that was already there. A
 * type with no variable-length member cannot carry that control and
 * `cases.tsv` marks it; its fill columns say `undetermined`.
 *
 * ## Why two processes and not two participants
 *
 * Cyclone delivers between endpoints of the same domain inside one
 * process without going near a socket. A capture of that is empty, and
 * an empty capture is the quietest possible false green.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <stdint.h>
#include "dds/dds.h"
#include "gen.h"

static void zd_fill_sample(@TYPE@ *out) {
    @TYPE@ sample;
    memset(&sample, 0, sizeof(sample));
#include "fill-sample.inc"
    *out = sample;
}

static void zd_fill_poison(@TYPE@ *out) {
    @TYPE@ sample;
    memset(&sample, 0, sizeof(sample));
#include "fill-poison.inc"
    *out = sample;
}

static dds_qos_t *zd_qos(uint32_t version) {
    dds_data_representation_id_t representation =
        version == 1 ? DDS_DATA_REPRESENTATION_XCDR1
                     : DDS_DATA_REPRESENTATION_XCDR2;
    dds_qos_t *qos = dds_create_qos();
    dds_qset_reliability(qos, DDS_RELIABILITY_RELIABLE, DDS_SECS(10));
    dds_qset_history(qos, DDS_HISTORY_KEEP_ALL, 0);
    dds_qset_data_representation(qos, 1, &representation);
    return qos;
}

/* `dds_write` returns a code and not a byte count, so a refusal is
 * reported by name rather than by an absent sample. */
static int zd_publish(dds_entity_t writer, int count) {
    for (int i = 0; i < count; i++) {
        @TYPE@ value;
        if (i % 2 == 0) zd_fill_poison(&value); else zd_fill_sample(&value);
        dds_return_t rc = dds_write(writer, &value);
        if (rc < 0) {
            fprintf(stderr, "cyclone refused the sample: %s\n", dds_strretcode(rc));
            return 1;
        }
        printf("wrote %d %s\n", i, i % 2 == 0 ? "poison" : "sample");
        fflush(stdout);
        dds_sleepfor(DDS_MSECS(120));
    }
    return 0;
}

int main(int argc, char **argv) {
    if (argc != 6) {
        fprintf(stderr, "usage: probe <pub|sub> <domain> <1|2> <count> <topic>\n");
        return 2;
    }
    const char *role = argv[1];
    dds_domainid_t domain = (dds_domainid_t)atoi(argv[2]);
    uint32_t version = (uint32_t)atoi(argv[3]);
    int count = atoi(argv[4]);
    const char *topic_name = argv[5];

    dds_entity_t participant = dds_create_participant(domain, NULL, NULL);
    if (participant < 0) {
        fprintf(stderr, "no participant: %s\n", dds_strretcode(participant));
        return 1;
    }
    dds_qos_t *qos = zd_qos(version);
    dds_entity_t topic =
        dds_create_topic(participant, &@TYPE@_desc, topic_name, qos, NULL);
    if (topic < 0) {
        fprintf(stderr, "no topic: %s\n", dds_strretcode(topic));
        return 1;
    }

    int status = 0;
    if (strcmp(role, "pub") == 0) {
        dds_entity_t writer = dds_create_writer(participant, topic, qos, NULL);
        if (writer < 0) {
            fprintf(stderr, "no writer: %s\n", dds_strretcode(writer));
            return 1;
        }
        /* A write before the reader is matched goes nowhere and the
           capture would be short by exactly that many samples. */
        uint32_t matched = 0;
        for (int i = 0; i < 300 && matched == 0; i++) {
            dds_publication_matched_status_t match;
            if (dds_get_publication_matched_status(writer, &match) == 0)
                matched = match.current_count;
            if (matched == 0) dds_sleepfor(DDS_MSECS(50));
        }
        if (matched == 0) {
            fprintf(stderr, "no reader matched in 15 s\n");
            return 1;
        }
        status = zd_publish(writer, count);
        dds_wait_for_acks(writer, DDS_SECS(10));
        printf("matched %u\n", matched);
    } else {
        dds_entity_t reader = dds_create_reader(participant, topic, qos, NULL);
        if (reader < 0) {
            fprintf(stderr, "no reader: %s\n", dds_strretcode(reader));
            return 1;
        }
        int seen = 0;
        void *samples[1] = { NULL };
        dds_sample_info_t info[1];
        for (int i = 0; i < 600 && seen < count; i++) {
            dds_return_t n = dds_take(reader, samples, info, 1, 1);
            if (n > 0) {
                if (info[0].valid_data) seen++;
                dds_return_loan(reader, samples, n);
            } else {
                dds_sleepfor(DDS_MSECS(50));
            }
        }
        printf("read %d\n", seen);
        status = seen == count ? 0 : 1;
    }
    dds_delete_qos(qos);
    dds_delete(participant);
    return status;
}
