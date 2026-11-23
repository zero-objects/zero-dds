/* A Cyclone DDS participant for crates/coap-bridge/tests/cross_vendor.rs
 * (zerodds-coap-bridge-1.0 §12.3): `sub DOMAIN SECONDS` prints every
 * sample of the topic SensorReading (type Sensor::Reading) it takes as one JSON line on stdout; `pub DOMAIN ID VALUE
 * UNIT` writes one sample once a reader matched. The type comes from
 * reading.idl through idlc. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dds/dds.h"
#include "reading.h"

static int sub(dds_entity_t participant, int seconds)
{
  dds_entity_t topic = dds_create_topic(participant, &Sensor_Reading_desc, "SensorReading", NULL, NULL);
  dds_qos_t *qos = dds_create_qos();
  dds_qset_reliability(qos, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));
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
  dds_time_t end = dds_time() + DDS_SECS(seconds);
  while (dds_time() < end) {
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

static int pub(dds_entity_t participant, int id, double value, const char *unit)
{
  dds_entity_t topic = dds_create_topic(participant, &Sensor_Reading_desc, "SensorReading", NULL, NULL);
  dds_qos_t *qos = dds_create_qos();
  dds_qset_reliability(qos, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));
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
  if (argc < 3) {
    fprintf(stderr, "usage: %s sub DOMAIN SECONDS | pub DOMAIN ID VALUE UNIT\n", argv[0]);
    return 2;
  }
  dds_entity_t participant = dds_create_participant((dds_domainid_t)atoi(argv[2]), NULL, NULL);
  if (participant < 0) {
    fprintf(stderr, "dds_create_participant: %s\n", dds_strretcode(-participant));
    return 1;
  }
  int rc;
  if (strcmp(argv[1], "sub") == 0 && argc == 4)
    rc = sub(participant, atoi(argv[3]));
  else if (strcmp(argv[1], "pub") == 0 && argc == 6)
    rc = pub(participant, atoi(argv[3]), atof(argv[4]), argv[5]);
  else
    rc = 2;
  dds_delete(participant);
  return rc;
}
