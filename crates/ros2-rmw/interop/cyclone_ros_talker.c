/* SPDX-License-Identifier: Apache-2.0
 * CycloneDDS publisher on rt/chatter with std_msgs/msg/String —
 * wire-identical to a real ROS-2 talker (rmw_cyclonedds).
 * RMW default QoS: RELIABLE + VOLATILE + KEEP_LAST(10).
 *
 * Usage: cyclone_ros_talker [count]   (default 20, interval 200 ms)
 */
#include "dds/dds.h"
#include "std_msgs_string.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
  int n = argc > 1 ? atoi(argv[1]) : 20;

  dds_entity_t pp = dds_create_participant(DDS_DOMAIN_DEFAULT, NULL, NULL);
  if (pp < 0) { fprintf(stderr, "participant: %s\n", dds_strretcode(-pp)); return 1; }

  /* ROS-2 wire: DDS topic "rt/chatter", type std_msgs::msg::dds_::String_ */
  dds_entity_t tp = dds_create_topic(pp, &std_msgs_msg_dds__String__desc,
                                     "rt/chatter", NULL, NULL);
  if (tp < 0) { fprintf(stderr, "topic: %s\n", dds_strretcode(-tp)); return 1; }

  dds_qos_t *qos = dds_create_qos();
  dds_qset_reliability(qos, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));
  dds_qset_history(qos, DDS_HISTORY_KEEP_LAST, 10);
  dds_qset_durability(qos, DDS_DURABILITY_VOLATILE);
  dds_entity_t wr = dds_create_writer(pp, tp, qos, NULL);
  dds_delete_qos(qos);
  if (wr < 0) { fprintf(stderr, "writer: %s\n", dds_strretcode(-wr)); return 1; }

  /* Wait for a matched subscriber BEFORE publishing volatile —
   * otherwise samples are lost if discovery (ZeroDDS↔Cyclone) takes longer
   * than the publish window. Up to 12 s. */
  dds_publication_matched_status_t pm;
  int matched = 0;
  for (int i = 0; i < 240; i++) {
    if (dds_get_publication_matched_status(wr, &pm) == DDS_RETCODE_OK &&
        pm.current_count > 0) { matched = 1; break; }
    dds_sleepfor(DDS_MSECS(50));
  }
  fprintf(stderr, "talker: matched subscriber=%d (current_count=%u)\n",
          matched, matched ? pm.current_count : 0u);
  dds_sleepfor(DDS_MSECS(200)); /* Reliable-Handshake settlen lassen */

  for (int i = 0; i < n; i++) {
    std_msgs_msg_dds__String_ msg;
    char buf[64];
    snprintf(buf, sizeof buf, "Hello ZeroDDS from ROS wire %d", i);
    msg.data = buf;
    dds_return_t rc = dds_write(wr, &msg);
    if (rc != DDS_RETCODE_OK) fprintf(stderr, "write: %s\n", dds_strretcode(-rc));
    else printf("published rt/chatter: %s\n", buf);
    fflush(stdout);
    dds_sleepfor(DDS_MSECS(200));
  }

  dds_delete(pp);
  return 0;
}
