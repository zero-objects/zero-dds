// SPDX-License-Identifier: Apache-2.0
// CycloneDDS TRANSIENT_LOCAL publisher for the cross-vendor durability INGEST
// test. Publishes `dur::Sample` on topic "DurVendorX" so the ZeroDDS durability
// service (external_cyclone_ingest, ZD_TYPE="dur::Sample") ingests real
// foreign-vendor wire bytes. Domain via the DOM env var (default 0).
//
// Build (codepit): see README.md in this directory.
#include "dds/dds.h"
#include "dur.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

int main(void) {
  int dom = getenv("DOM") ? atoi(getenv("DOM")) : 0;
  dds_entity_t pp = dds_create_participant(dom, NULL, NULL);
  dds_entity_t tp = dds_create_topic(pp, &dur_Sample_desc, "DurVendorX", NULL, NULL);
  dds_qos_t *q = dds_create_qos();
  dds_qset_durability(q, DDS_DURABILITY_TRANSIENT_LOCAL);
  dds_qset_reliability(q, DDS_RELIABILITY_RELIABLE, DDS_SECS(10));
  dds_qset_history(q, DDS_HISTORY_KEEP_ALL, 0);
  dds_entity_t wr = dds_create_writer(pp, tp, q, NULL);
  if (wr < 0) { printf("writer err %s\n", dds_strretcode(-wr)); return 1; }
  printf("cyclone writer up on DurVendorX (dur::Sample), waiting for match...\n");
  fflush(stdout);
  int matched = 0;
  for (int i = 0; i < 300; i++) {
    dds_publication_matched_status_t st; memset(&st, 0, sizeof st);
    dds_get_publication_matched_status(wr, &st);
    if (st.current_count > 0) { matched = 1; break; }
    dds_sleepfor(DDS_MSECS(100));
  }
  printf("matched=%d\n", matched); fflush(stdout);
  for (int i = 0; i < 10; i++) {
    dur_Sample s; memset(&s, 0, sizeof s);
    uint8_t buf[4]; buf[0] = (uint8_t)i; buf[1] = 10; buf[2] = 20; buf[3] = 30;
    s.data._length = 4; s.data._maximum = 4; s.data._buffer = buf; s.data._release = false;
    dds_return_t rc = dds_write(wr, &s);
    if (rc != DDS_RETCODE_OK) printf("write %d err %s\n", i, dds_strretcode(-rc));
    dds_sleepfor(DDS_MSECS(400));
  }
  printf("cyclone published 10 samples\n"); fflush(stdout);
  dds_sleepfor(DDS_SECS(6));
  dds_delete(pp);
  return 0;
}
