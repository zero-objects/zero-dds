// SPDX-License-Identifier: Apache-2.0
// CycloneDDS TRANSIENT_LOCAL publisher of the alignment-sensitive `al::AlignS`
// on topic "AlignTopic". Writes one sample (tag=7, val=0x1122334455667788),
// stays briefly so the ZeroDDS durability service ingests it, then exits — so a
// later subscriber can only get the sample from the ZeroDDS replay writer.
// Build: see README.md. Domain via DOM env (default 0).
#include "dds/dds.h"
#include "al.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

int main(void) {
  int dom = getenv("DOM") ? atoi(getenv("DOM")) : 0;
  dds_entity_t pp = dds_create_participant(dom, NULL, NULL);
  dds_entity_t tp = dds_create_topic(pp, &al_AlignS_desc, "AlignTopic", NULL, NULL);
  dds_qos_t *q = dds_create_qos();
  dds_qset_durability(q, DDS_DURABILITY_TRANSIENT_LOCAL);
  dds_qset_reliability(q, DDS_RELIABILITY_RELIABLE, DDS_SECS(10));
  dds_qset_history(q, DDS_HISTORY_KEEP_ALL, 0);
  dds_entity_t wr = dds_create_writer(pp, tp, q, NULL);
  for (int i = 0; i < 200; i++) {
    dds_publication_matched_status_t s; memset(&s, 0, sizeof s);
    dds_get_publication_matched_status(wr, &s);
    if (s.current_count > 0) break;
    dds_sleepfor(DDS_MSECS(50));
  }
  al_AlignS v; memset(&v, 0, sizeof v);
  v.tag = 7; v.val = 0x1122334455667788LL;
  dds_write(wr, &v);
  printf("PUB wrote tag=%u val=%llx\n", v.tag, (unsigned long long)v.val);
  fflush(stdout);
  dds_sleepfor(DDS_SECS(7));
  dds_delete(pp);
  return 0;
}
