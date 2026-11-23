/* The frame of the Cyclone DDS backends of
 * crates/corba-dds-bridge/tests/omniorb_e2e.rs: `NAME DOMAIN COUNT` creates
 * a participant on DOMAIN, a reader of the request topic and a writer of the
 * reply topic, both RELIABLE and XCDR1, prints `ready` once the endpoints
 * exist, hands every request it takes to `answer`, which writes the reply,
 * and prints `served N` after COUNT replies or 30 s. */
#ifndef ZERODDS_TEST_BACKEND_H
#define ZERODDS_TEST_BACKEND_H

#include <stdio.h>
#include <stdlib.h>
#include "dds/dds.h"

typedef dds_return_t (*backend_answer)(dds_entity_t writer, const void *request);

static int backend_serve(int argc, char **argv,
                         const dds_topic_descriptor_t *request_type, const char *request_topic,
                         const dds_topic_descriptor_t *reply_type, const char *reply_topic,
                         backend_answer answer)
{
  if (argc < 3) {
    fprintf(stderr, "usage: %s DOMAIN COUNT\n", argv[0]);
    return 2;
  }
  dds_entity_t participant = dds_create_participant((dds_domainid_t)atoi(argv[1]), NULL, NULL);
  if (participant < 0) {
    fprintf(stderr, "dds_create_participant: %s\n", dds_strretcode(-participant));
    return 1;
  }
  int count = atoi(argv[2]);
  dds_qos_t *qos = dds_create_qos();
  dds_qset_reliability(qos, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));
  dds_data_representation_id_t xcdr1[] = { DDS_DATA_REPRESENTATION_XCDR1 };
  dds_qset_data_representation(qos, 1, xcdr1);
  dds_entity_t req_topic = dds_create_topic(participant, request_type, request_topic, NULL, NULL);
  dds_entity_t rep_topic = dds_create_topic(participant, reply_type, reply_topic, NULL, NULL);
  dds_entity_t reader = dds_create_reader(participant, req_topic, qos, NULL);
  dds_entity_t writer = dds_create_writer(participant, rep_topic, qos, NULL);
  dds_delete_qos(qos);
  if (reader < 0 || writer < 0) {
    fprintf(stderr, "endpoints: %s %s\n", dds_strretcode(-reader), dds_strretcode(-writer));
    return 1;
  }
  printf("ready\n");
  fflush(stdout);
  void *samples[1] = { NULL };
  dds_sample_info_t infos[1];
  int served = 0;
  dds_time_t end = dds_time() + DDS_SECS(30);
  while (served < count && dds_time() < end) {
    int n = dds_take(reader, samples, infos, 1, 1);
    if (n > 0 && infos[0].valid_data) {
      dds_return_t rc = answer(writer, samples[0]);
      if (rc != DDS_RETCODE_OK)
        fprintf(stderr, "dds_write: %s\n", dds_strretcode(-rc));
      else
        served++;
    }
    if (n > 0)
      dds_return_loan(reader, samples, n);
    else
      dds_sleepfor(DDS_MSECS(10));
  }
  /* The reply is reliable: give it time to arrive before the writer goes. */
  dds_sleepfor(DDS_SECS(1));
  printf("served %d\n", served);
  fflush(stdout);
  dds_delete(participant);
  return 0;
}

#endif
