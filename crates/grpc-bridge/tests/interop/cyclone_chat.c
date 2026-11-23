/* A Cyclone DDS participant for crates/grpc-bridge/tests/cross_vendor.rs
 * (zerodds-grpc-bridge-1.0 §10): `sub DOMAIN SECONDS KIND` prints every
 * sample it takes as one JSON line on stdout; `pub DOMAIN SEQ TEXT KIND`
 * writes the sample once a reader matched, then again every 200 ms for 5 s,
 * so that a reader that matches later gets it too. KIND `message` is the
 * topic ChatMessage of the keyed type Chat::Message, `note` the topic
 * ChatNote of the keyless type Chat::Note. Both RELIABLE. The types come
 * from chat.idl through idlc.
 * `echo DOMAIN SECONDS` is the Cyclone DDS service of the Compose file of
 * §12.3 (crates/grpc-bridge/tests/compose/): a reader and a writer of
 * ChatMessage; each sample whose text does not begin with "echo: " is
 * answered with { seq, "echo: " text } and printed as one JSON line. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dds/dds.h"
#include "chat.h"

static dds_qos_t *reliable(void)
{
  dds_qos_t *qos = dds_create_qos();
  dds_qset_reliability(qos, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));
  return qos;
}

/* Chat::Message and Chat::Note have the same members. */
static dds_entity_t topic_of(dds_entity_t participant, const char *kind)
{
  if (strcmp(kind, "note") == 0)
    return dds_create_topic(participant, &Chat_Note_desc, "ChatNote", NULL, NULL);
  return dds_create_topic(participant, &Chat_Message_desc, "ChatMessage", NULL, NULL);
}

static int sub(dds_entity_t participant, int seconds, const char *kind)
{
  dds_entity_t topic = topic_of(participant, kind);
  dds_qos_t *qos = reliable();
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
      Chat_Message *m = samples[0];
      printf("{\"seq\":%d,\"text\":\"%s\"}\n", m->seq, m->text);
      fflush(stdout);
    }
    if (n > 0)
      dds_return_loan(reader, samples, n);
    else
      dds_sleepfor(DDS_MSECS(20));
  }
  return 0;
}

static int pub(dds_entity_t participant, int seq, const char *text, const char *kind)
{
  dds_entity_t topic = topic_of(participant, kind);
  dds_qos_t *qos = reliable();
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
  Chat_Message m = { .seq = seq, .text = (char *)text };
  for (int i = 0; i < 25; i++) {
    dds_return_t rc = dds_write(writer, &m);
    if (rc != DDS_RETCODE_OK) {
      fprintf(stderr, "dds_write: %s\n", dds_strretcode(-rc));
      return 1;
    }
    if (i == 0) {
      printf("{\"event\":\"written\"}\n");
      fflush(stdout);
    }
    dds_sleepfor(DDS_MSECS(200));
  }
  return 0;
}

static int echo(dds_entity_t participant, int seconds)
{
  dds_entity_t topic = topic_of(participant, "message");
  dds_qos_t *qos = reliable();
  dds_entity_t reader = dds_create_reader(participant, topic, qos, NULL);
  dds_entity_t writer = dds_create_writer(participant, topic, qos, NULL);
  dds_delete_qos(qos);
  if (reader < 0 || writer < 0) {
    fprintf(stderr, "dds_create_reader/writer: %s\n", dds_strretcode(-(reader < 0 ? reader : writer)));
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
      Chat_Message *m = samples[0];
      if (strncmp(m->text, "echo: ", 6) != 0) {
        char text[512];
        snprintf(text, sizeof text, "echo: %s", m->text);
        Chat_Message answer = { .seq = m->seq, .text = text };
        dds_return_t rc = dds_write(writer, &answer);
        if (rc != DDS_RETCODE_OK) {
          fprintf(stderr, "dds_write: %s\n", dds_strretcode(-rc));
          dds_return_loan(reader, samples, n);
          return 1;
        }
        printf("{\"seq\":%d,\"text\":\"%s\"}\n", m->seq, m->text);
        fflush(stdout);
      }
    }
    if (n > 0)
      dds_return_loan(reader, samples, n);
    else
      dds_sleepfor(DDS_MSECS(20));
  }
  return 0;
}

int main(int argc, char **argv)
{
  if (argc < 3) {
    fprintf(stderr, "usage: %s sub DOMAIN SECONDS KIND | pub DOMAIN SEQ TEXT KIND | echo DOMAIN SECONDS\n",
            argv[0]);
    return 2;
  }
  dds_entity_t participant = dds_create_participant((dds_domainid_t)atoi(argv[2]), NULL, NULL);
  if (participant < 0) {
    fprintf(stderr, "dds_create_participant: %s\n", dds_strretcode(-participant));
    return 1;
  }
  int rc;
  if (strcmp(argv[1], "sub") == 0 && argc == 5)
    rc = sub(participant, atoi(argv[3]), argv[4]);
  else if (strcmp(argv[1], "pub") == 0 && argc == 6)
    rc = pub(participant, atoi(argv[3]), argv[4], argv[5]);
  else if (strcmp(argv[1], "echo") == 0 && argc == 4)
    rc = echo(participant, atoi(argv[3]));
  else
    rc = 2;
  dds_delete(participant);
  return rc;
}
