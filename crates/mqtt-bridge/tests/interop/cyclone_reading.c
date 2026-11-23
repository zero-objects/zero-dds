/* A Cyclone DDS participant for crates/mqtt-bridge/tests/cross_vendor.rs
 * (zerodds-mqtt-bridge-1.0 §10, §12.3). `sub DOMAIN TOPIC XCDR SECONDS`
 * prints `{"event":"matched"}` once its reader matched a writer and every
 * sample of TOPIC (type Sensor::Reading from reading.idl through idlc) it
 * takes as one JSON line on stdout; `pub DOMAIN TOPIC XCDR
 * ID VALUE UNIT` writes one sample once a reader matched. XCDR is 1 or 2,
 * the one data representation of the endpoint (DDS-XTypes 1.3 §7.6.3.1). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dds/dds.h"
#include "reading.h"

static dds_qos_t *endpoint_qos(int xcdr)
{
  dds_qos_t *qos = dds_create_qos();
  dds_qset_reliability(qos, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));
  dds_data_representation_id_t rep[1] = {
    xcdr == 1 ? DDS_DATA_REPRESENTATION_XCDR1 : DDS_DATA_REPRESENTATION_XCDR2
  };
  dds_qset_data_representation(qos, 1, rep);
  return qos;
}

static int sub(dds_entity_t participant, const char *name, int xcdr, int seconds)
{
  dds_entity_t topic = dds_create_topic(participant, &Sensor_Reading_desc, name, NULL, NULL);
  dds_qos_t *qos = endpoint_qos(xcdr);
  dds_entity_t reader = dds_create_reader(participant, topic, qos, NULL);
  dds_delete_qos(qos);
  if (reader < 0) {
    fprintf(stderr, "dds_create_reader: %s\n", dds_strretcode(-reader));
    return 1;
  }
  printf("{\"event\":\"ready\"}\n");
  fflush(stdout);
  void *samples[1] = { NULL };
  dds_sample_info_t infos[1];
  int matched = 0;
  dds_time_t end = dds_time() + DDS_SECS(seconds);
  while (dds_time() < end) {
    dds_subscription_matched_status_t status;
    if (!matched && dds_get_subscription_matched_status(reader, &status) == DDS_RETCODE_OK
        && status.current_count > 0) {
      matched = 1;
      printf("{\"event\":\"matched\"}\n");
      fflush(stdout);
    }
    int n = dds_take(reader, samples, infos, 1, 1);
    if (n > 0 && infos[0].valid_data) {
      Sensor_Reading *r = samples[0];
      printf("{\"id\":%d,\"value\":%.17g,\"unit\":\"%s\"}\n", r->id, r->value, r->unit);
      fflush(stdout);
    }
    if (n > 0)
      dds_return_loan(reader, samples, n);
    else
      dds_sleepfor(DDS_MSECS(20));
  }
  return 0;
}

static int pub(dds_entity_t participant, const char *name, int xcdr, int id, double value,
               const char *unit)
{
  dds_entity_t topic = dds_create_topic(participant, &Sensor_Reading_desc, name, NULL, NULL);
  dds_qos_t *qos = endpoint_qos(xcdr);
  dds_entity_t writer = dds_create_writer(participant, topic, qos, NULL);
  dds_delete_qos(qos);
  if (writer < 0) {
    fprintf(stderr, "dds_create_writer: %s\n", dds_strretcode(-writer));
    return 1;
  }
  uint32_t status = 0;
  dds_set_status_mask(writer, DDS_PUBLICATION_MATCHED_STATUS);
  for (int i = 0; i < 1000 && !(status & DDS_PUBLICATION_MATCHED_STATUS); i++) {
    dds_get_status_changes(writer, &status);
    dds_sleepfor(DDS_MSECS(10));
  }
  if (!(status & DDS_PUBLICATION_MATCHED_STATUS)) {
    fprintf(stderr, "no reader matched\n");
    return 1;
  }
  Sensor_Reading r = { .id = id, .value = value, .unit = (char *)unit };
  dds_return_t rc = dds_write(writer, &r);
  if (rc != DDS_RETCODE_OK) {
    fprintf(stderr, "dds_write: %s\n", dds_strretcode(-rc));
    return 1;
  }
  printf("{\"event\":\"written\"}\n");
  fflush(stdout);
  dds_sleepfor(DDS_SECS(2));
  return 0;
}

int main(int argc, char **argv)
{
  if (argc < 5) {
    fprintf(stderr,
            "usage: %s sub DOMAIN TOPIC XCDR SECONDS | pub DOMAIN TOPIC XCDR ID VALUE UNIT\n",
            argv[0]);
    return 2;
  }
  int xcdr = atoi(argv[4]);
  if (xcdr != 1 && xcdr != 2) {
    fprintf(stderr, "XCDR is 1 or 2\n");
    return 2;
  }
  dds_entity_t participant = dds_create_participant((dds_domainid_t)atoi(argv[2]), NULL, NULL);
  if (participant < 0) {
    fprintf(stderr, "dds_create_participant: %s\n", dds_strretcode(-participant));
    return 1;
  }
  int rc;
  if (strcmp(argv[1], "sub") == 0 && argc == 6)
    rc = sub(participant, argv[3], xcdr, atoi(argv[5]));
  else if (strcmp(argv[1], "pub") == 0 && argc == 8)
    rc = pub(participant, argv[3], xcdr, atoi(argv[5]), atof(argv[6]), argv[7]);
  else
    rc = 2;
  dds_delete(participant);
  return rc;
}
