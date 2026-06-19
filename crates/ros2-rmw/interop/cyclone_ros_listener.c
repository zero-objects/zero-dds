/* SPDX-License-Identifier: Apache-2.0
 * CycloneDDS subscriber on rt/chatter (std_msgs/String) — ROS-2 listener.
 * Matches the talker so the RELIABLE writer puts DATA on the wire.
 * Usage: cyclone_ros_listener [seconds]   (default 8 s)
 */
#include "dds/dds.h"
#include "std_msgs_string.h"
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char **argv) {
  int secs = argc > 1 ? atoi(argv[1]) : 8;
  dds_entity_t pp = dds_create_participant(DDS_DOMAIN_DEFAULT, NULL, NULL);
  dds_entity_t tp = dds_create_topic(pp, &std_msgs_msg_dds__String__desc,
                                     "rt/chatter", NULL, NULL);
  dds_qos_t *qos = dds_create_qos();
  dds_qset_reliability(qos, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));
  dds_qset_history(qos, DDS_HISTORY_KEEP_LAST, 10);
  dds_qset_durability(qos, DDS_DURABILITY_VOLATILE);
  dds_entity_t rd = dds_create_reader(pp, tp, qos, NULL);
  dds_delete_qos(qos);

  void *samples[1] = {0};
  dds_sample_info_t infos[1];
  int got = 0;
  for (int t = 0; t < secs * 100; t++) {
    dds_return_t n = dds_take(rd, samples, infos, 1, 1);
    if (n > 0 && infos[0].valid_data) {
      std_msgs_msg_dds__String_ *m = samples[0];
      printf("received rt/chatter: %s\n", m->data);
      fflush(stdout);
      got++;
      dds_return_loan(rd, samples, n);
    }
    dds_sleepfor(DDS_MSECS(10));
  }
  printf("== listener got %d samples ==\n", got);
  dds_delete(pp);
  return got > 0 ? 0 : 2;
}
