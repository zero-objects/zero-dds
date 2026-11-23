/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * A Cyclone DDS participant at DCPS level on topic STATION of
 * Legacy::StationType (station.idl), for tests/cyclone_fixed_model.rs:
 *
 *   station write <domain> <site> <num> <level>
 *       waits for a matched reader, writes one sample, prints WROTE, and
 *       stays three seconds so that the sample is delivered;
 *   station read <domain> <seconds>
 *       prints READY once its reader exists, then SAMPLE <site> <num>
 *       <level> for every sample it takes, until <seconds> are over.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "dds/dds.h"
#include "station.h"

static int fail(const char *what, dds_return_t rc)
{
    fprintf(stderr, "%s: %s\n", what, dds_strretcode(rc));
    return 1;
}

int main(int argc, char **argv)
{
    if (argc < 4) {
        fprintf(stderr, "usage: station write <domain> <site> <num> <level> | station read <domain> <seconds>\n");
        return 2;
    }
    dds_entity_t pp = dds_create_participant((dds_domainid_t)atoi(argv[2]), NULL, NULL);
    if (pp < 0)
        return fail("participant", pp);
    dds_entity_t tp = dds_create_topic(pp, &Legacy_StationType_desc, "STATION", NULL, NULL);
    if (tp < 0)
        return fail("topic", tp);
    dds_qos_t *qos = dds_create_qos();
    dds_qset_reliability(qos, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));

    if (strcmp(argv[1], "write") == 0) {
        if (argc < 6)
            return 2;
        dds_entity_t wr = dds_create_writer(pp, tp, qos, NULL);
        if (wr < 0)
            return fail("writer", wr);
        dds_publication_matched_status_t st = {0};
        for (int i = 0; i < 400 && st.current_count == 0; i++) {
            dds_get_publication_matched_status(wr, &st);
            if (st.current_count == 0)
                dds_sleepfor(DDS_MSECS(50));
        }
        if (st.current_count == 0) {
            fprintf(stderr, "no reader matched\n");
            return 1;
        }
        Legacy_StationType s;
        s.SITE = argv[3];
        s.NUM = atoi(argv[4]);
        s.LEVEL = atof(argv[5]);
        dds_return_t rc = dds_write(wr, &s);
        if (rc != DDS_RETCODE_OK)
            return fail("write", rc);
        printf("WROTE\n");
        fflush(stdout);
        dds_sleepfor(DDS_SECS(3));
    } else {
        dds_entity_t rd = dds_create_reader(pp, tp, qos, NULL);
        if (rd < 0)
            return fail("reader", rd);
        printf("READY\n");
        fflush(stdout);
        dds_time_t end = dds_time() + DDS_SECS(atoi(argv[3]));
        while (dds_time() < end) {
            void *samples[1] = {NULL};
            dds_sample_info_t infos[1];
            dds_return_t n = dds_take(rd, samples, infos, 1, 1);
            if (n > 0) {
                const Legacy_StationType *s = samples[0];
                if (infos[0].valid_data) {
                    printf("SAMPLE %s %d %g\n", s->SITE, (int)s->NUM, s->LEVEL);
                    fflush(stdout);
                }
                dds_return_loan(rd, samples, n);
            } else {
                dds_sleepfor(DDS_MSECS(20));
            }
        }
    }
    dds_delete_qos(qos);
    dds_delete(pp);
    return 0;
}
