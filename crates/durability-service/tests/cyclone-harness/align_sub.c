// SPDX-License-Identifier: Apache-2.0
// CycloneDDS subscriber of `al::AlignS` on "AlignTopic". Started AFTER the
// publisher has exited, so any sample it receives came from the ZeroDDS
// durability replay writer. Prints the decoded value: a correct
// tag=7/val=1122334455667788 proves the replay encap matched the XCDR1 body
// (a wrong encap would mis-align `val` or fail to match). Domain via DOM env.
#include "dds/dds.h"
#include "al.h"
#include <stdio.h>
#include <stdlib.h>

int main(void) {
  int dom = getenv("DOM") ? atoi(getenv("DOM")) : 0;
  dds_entity_t pp = dds_create_participant(dom, NULL, NULL);
  dds_entity_t tp = dds_create_topic(pp, &al_AlignS_desc, "AlignTopic", NULL, NULL);
  dds_qos_t *q = dds_create_qos();
  dds_qset_durability(q, DDS_DURABILITY_TRANSIENT_LOCAL);
  dds_qset_reliability(q, DDS_RELIABILITY_RELIABLE, DDS_SECS(10));
  dds_qset_history(q, DDS_HISTORY_KEEP_ALL, 0);
  dds_entity_t rd = dds_create_reader(pp, tp, q, NULL);
  void *samples[1] = {NULL};
  dds_sample_info_t si[1];
  for (int i = 0; i < 400; i++) {
    dds_return_t n = dds_take(rd, samples, si, 1, 1);
    if (n > 0 && si[0].valid_data) {
      al_AlignS *v = samples[0];
      printf("SUB GOT tag=%u val=%llx\n", v->tag, (unsigned long long)v->val);
      fflush(stdout);
      dds_return_loan(rd, samples, n);
      dds_delete(pp);
      return 0;
    }
    dds_sleepfor(DDS_MSECS(50));
  }
  printf("SUB GOT nothing\n");
  dds_delete(pp);
  return 2;
}
