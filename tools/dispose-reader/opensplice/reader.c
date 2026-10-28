/* SPDX-License-Identifier: Apache-2.0 */
/* Copyright 2026 ZeroDDS Contributors */
/* An OpenSplice 6.9 reader of KeyedPointA that reports instance states, for
 * the vendor tests that take ZERODDS_DISPOSE_READER.
 *
 *   reader DOMAIN TOPIC SECONDS SNAPSHOT_FILE STOP_FILE
 *
 * The domain comes from $OSPL_URI: the wrapper build.sh writes points it
 * at a configuration with <Id>DOMAIN</Id>, since OpenSplice takes the
 * domain from its configuration. Prints READY once the reader exists,
 * MATCHED once it has matched a writer and SEEN id=<id> for the first
 * valid sample of each instance. When SNAPSHOT_FILE appears it prints `SNAP id=<id>
 * state=<state>` per instance (the state of the last sample taken for it)
 * and SNAPDONE; when STOP_FILE appears, or after SECONDS, the same as
 * FINAL lines. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#include "dds_dcps.h"
#include "KeyedPointA.h"

#define MAX_INSTANCES 256

struct instance {
  DDS_InstanceHandle_t handle;
  int has_id;
  DDS_long id;
  const char *state;
};

static struct instance instances[MAX_INSTANCES];
static int instance_count = 0;

static int exists(const char *path) {
  struct stat st;
  return stat(path, &st) == 0;
}

static double now(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (double) ts.tv_sec + (double) ts.tv_nsec / 1e9;
}

static const char *state_name(DDS_InstanceStateKind s) {
  switch (s) {
    case DDS_ALIVE_INSTANCE_STATE: return "ALIVE";
    case DDS_NOT_ALIVE_DISPOSED_INSTANCE_STATE: return "NOT_ALIVE_DISPOSED";
    case DDS_NOT_ALIVE_NO_WRITERS_INSTANCE_STATE: return "NOT_ALIVE_NO_WRITERS";
    default: return "UNKNOWN";
  }
}

/* The entry for `handle`; NULL once MAX_INSTANCES are taken, which the
 * caller reports rather than dropping the sample silently. */
static struct instance *instance_of(DDS_InstanceHandle_t handle) {
  for (int i = 0; i < instance_count; i++) {
    if (instances[i].handle == handle) return &instances[i];
  }
  if (instance_count == MAX_INSTANCES) return NULL;
  struct instance *fresh = &instances[instance_count++];
  fresh->handle = handle;
  fresh->has_id = 0;
  fresh->id = 0;
  fresh->state = "UNKNOWN";
  return fresh;
}

static void report(const char *tag) {
  for (int i = 0; i < instance_count; i++) {
    if (instances[i].has_id) {
      printf("%s id=%d state=%s\n", tag, (int) instances[i].id, instances[i].state);
    } else {
      printf("%s id=? state=%s\n", tag, instances[i].state);
    }
  }
}

int main(int argc, char **argv) {
  if (argc != 6) {
    fprintf(stderr, "usage: reader DOMAIN TOPIC SECONDS SNAPSHOT_FILE STOP_FILE\n");
    return 2;
  }
  setvbuf(stdout, NULL, _IOLBF, 0);
  const char *topic_name = argv[2];
  const double seconds = atof(argv[3]);
  const char *snapshot = argv[4], *stop = argv[5];

  DDS_DomainParticipantFactory f = DDS_DomainParticipantFactory_get_instance();
  DDS_DomainParticipant p = DDS_DomainParticipantFactory_create_participant(
      f, DDS_DOMAIN_ID_DEFAULT, DDS_PARTICIPANT_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
  if (!p) { fprintf(stderr, "no participant\n"); return 1; }
  KeyedPointATypeSupport ts = KeyedPointATypeSupport__alloc();
  char *type_name = KeyedPointATypeSupport_get_type_name(ts);
  if (KeyedPointATypeSupport_register_type(ts, p, type_name) != DDS_RETCODE_OK) {
    fprintf(stderr, "register_type failed\n");
    return 1;
  }
  DDS_TopicQos *tqos = DDS_TopicQos__alloc();
  DDS_DomainParticipant_get_default_topic_qos(p, tqos);
  tqos->reliability.kind = DDS_RELIABLE_RELIABILITY_QOS;
  tqos->history.kind = DDS_KEEP_ALL_HISTORY_QOS;
  DDS_Topic topic = DDS_DomainParticipant_create_topic(
      p, topic_name, type_name, tqos, NULL, DDS_STATUS_MASK_NONE);
  if (!topic) { fprintf(stderr, "no topic\n"); return 1; }
  DDS_Subscriber sub = DDS_DomainParticipant_create_subscriber(
      p, DDS_SUBSCRIBER_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
  if (!sub) { fprintf(stderr, "no subscriber\n"); return 1; }
  DDS_DataReader rd = DDS_Subscriber_create_datareader(
      sub, (DDS_TopicDescription) topic, DDS_DATAREADER_QOS_USE_TOPIC_QOS, NULL,
      DDS_STATUS_MASK_NONE);
  if (!rd) { fprintf(stderr, "no reader\n"); return 1; }
  printf("READY\n");

  int matched = 0, snapped = 0;
  const double end = now() + seconds;
  DDS_sequence_KeyedPointA *samples = DDS_sequence_KeyedPointA__alloc();
  DDS_SampleInfoSeq *infos = DDS_SampleInfoSeq__alloc();
  while (now() < end && !exists(stop)) {
    DDS_SubscriptionMatchedStatus st;
    if (!matched && DDS_DataReader_get_subscription_matched_status(rd, &st) == DDS_RETCODE_OK
        && st.current_count >= 1) {
      printf("MATCHED\n");
      matched = 1;
    }
    if (KeyedPointADataReader_take(rd, samples, infos, DDS_LENGTH_UNLIMITED,
            DDS_ANY_SAMPLE_STATE, DDS_ANY_VIEW_STATE, DDS_ANY_INSTANCE_STATE)
        == DDS_RETCODE_OK) {
      for (DDS_unsigned_long i = 0; i < samples->_length; i++) {
        DDS_SampleInfo *info = &infos->_buffer[i];
        struct instance *entry = instance_of(info->instance_handle);
        if (!entry) {
          fprintf(stderr, "more than %d instances\n", MAX_INSTANCES);
          return 1;
        }
        if (info->valid_data) {
          if (!entry->has_id) printf("SEEN id=%d\n", (int) samples->_buffer[i].id);
          entry->has_id = 1;
          entry->id = samples->_buffer[i].id;
        }
        entry->state = state_name(info->instance_state);
      }
      KeyedPointADataReader_return_loan(rd, samples, infos);
    }
    if (!snapped && exists(snapshot)) {
      report("SNAP");
      printf("SNAPDONE\n");
      snapped = 1;
    }
    usleep(20000);
  }
  report("FINAL");
  DDS_DomainParticipant_delete_contained_entities(p);
  DDS_DomainParticipantFactory_delete_participant(f, p);
  return 0;
}
