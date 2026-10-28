/* SPDX-License-Identifier: Apache-2.0 */
/* Copyright 2026 ZeroDDS Contributors */
/* One OpenSplice 6.9 participant on the domain of $OSPL_URI. `pub` writes
 * one sample of each type of keyhash.idl every 200 ms on a topic named
 * after the type; `sub` only reads, so that the writer's DDSI service has a
 * matched reader and puts DATA on the wire. OpenSplice computes the KeyHash
 * inside that service (q_osplser.c) and offers no call that returns it, so
 * the measurement is the PID_KEY_HASH on the wire (see ../run-all.sh). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "dds_dcps.h"
#include "keyhash.h"

#define DIE(what) do { fprintf(stderr, "node: %s\n", what); exit(1); } while (0)

#define TOPIC(T)                                                                    \
  T##TypeSupport ts_##T = T##TypeSupport__alloc();                                  \
  char *tn_##T = T##TypeSupport_get_type_name(ts_##T);                              \
  if (T##TypeSupport_register_type(ts_##T, p, tn_##T) != DDS_RETCODE_OK)            \
    DIE("register " #T);                                                            \
  DDS_Topic t_##T = DDS_DomainParticipant_create_topic(p, #T, tn_##T, tqos, NULL,   \
      DDS_STATUS_MASK_NONE);                                                        \
  if (!t_##T) DIE("topic " #T);

int main(int argc, char **argv) {
  if (argc < 3) DIE("usage: node pub|sub SECONDS");
  int pub = strcmp(argv[1], "pub") == 0;
  int seconds = atoi(argv[2]);
  DDS_DomainParticipantFactory f = DDS_DomainParticipantFactory_get_instance();
  DDS_DomainParticipant p = DDS_DomainParticipantFactory_create_participant(
      f, DDS_DOMAIN_ID_DEFAULT, DDS_PARTICIPANT_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
  if (!p) DIE("participant");
  DDS_TopicQos *tqos = DDS_TopicQos__alloc();
  DDS_DomainParticipant_get_default_topic_qos(p, tqos);
  tqos->reliability.kind = DDS_RELIABLE_RELIABILITY_QOS;
  /* KEEP_ALL: with the default KEEP_LAST 1, two writers of the same
   * instance whose samples land between two takes leave only the later
   * one, and the per-writer counts below undercount one of them. */
  tqos->history.kind = DDS_KEEP_ALL_HISTORY_QOS;
  TOPIC(PadKey) TOPIC(WideKey) TOPIC(CharKey) TOPIC(StrKey) TOPIC(PathKey) TOPIC(BigKey)
  if (pub) {
    DDS_Publisher pb = DDS_DomainParticipant_create_publisher(p, DDS_PUBLISHER_QOS_DEFAULT,
        NULL, DDS_STATUS_MASK_NONE);
    if (!pb) DIE("publisher");
#define W(T) DDS_DataWriter w_##T = DDS_Publisher_create_datawriter(pb, t_##T,          \
        DDS_DATAWRITER_QOS_USE_TOPIC_QOS, NULL, DDS_STATUS_MASK_NONE);                 \
    if (!w_##T) DIE("writer " #T);
    W(PadKey) W(WideKey) W(CharKey) W(StrKey) W(PathKey) W(BigKey)
    PadKey pad = { 0x11, 0x22334455, 1 };
    WideKey wide = { 0x0102, 0x0304050607080910LL, 1 };
    CharKey ch = { 0x11, 0x2233, 1 };
    StrKey str; str.name = DDS_string_dup("ab"); str.id = 0x0a0b0c0d; str.v = 1;
    PathKey path; path.nested.a = 0x55; path.nested.b = 0x66; path.x = 0x77778888; path.v = 1;
    BigKey big = { 0x0101010101010101LL, 0x0202020202020202LL, 0x03030303, 1 };
    for (int i = 0; i < seconds * 5; i++) {
      PadKeyDataWriter_write(w_PadKey, &pad, DDS_HANDLE_NIL);
      WideKeyDataWriter_write(w_WideKey, &wide, DDS_HANDLE_NIL);
      CharKeyDataWriter_write(w_CharKey, &ch, DDS_HANDLE_NIL);
      StrKeyDataWriter_write(w_StrKey, &str, DDS_HANDLE_NIL);
      PathKeyDataWriter_write(w_PathKey, &path, DDS_HANDLE_NIL);
      BigKeyDataWriter_write(w_BigKey, &big, DDS_HANDLE_NIL);
      usleep(200000);
    }
  } else {
    DDS_Subscriber sb = DDS_DomainParticipant_create_subscriber(p, DDS_SUBSCRIBER_QOS_DEFAULT,
        NULL, DDS_STATUS_MASK_NONE);
    if (!sb) DIE("subscriber");
#define R(T) DDS_DataReader r_##T = DDS_Subscriber_create_datareader(sb,               \
        (DDS_TopicDescription) t_##T, DDS_DATAREADER_QOS_USE_TOPIC_QOS, NULL,            \
        DDS_STATUS_MASK_NONE);                                                           \
    if (!r_##T) DIE("reader " #T);
    R(PadKey) R(WideKey) R(CharKey) R(StrKey) R(PathKey) R(BigKey)
    /* PadKey's instances, for a peer that writes the same key with another
     * KeyHash: OpenSplice counts one instance if it keys by the payload,
     * two if by the PID_KEY_HASH it received. */
    unsigned long long handles[16];
    int nhandles = 0, nsamples = 0, from_v1 = 0, from_v2 = 0;
    for (int tick = 0; tick < seconds * 10; tick++) {
      DDS_sequence_PadKey *seq = DDS_sequence_PadKey__alloc();
      DDS_SampleInfoSeq *info = DDS_SampleInfoSeq__alloc();
      if (PadKeyDataReader_take(r_PadKey, seq, info, DDS_LENGTH_UNLIMITED, DDS_ANY_SAMPLE_STATE,
                                DDS_ANY_VIEW_STATE, DDS_ANY_INSTANCE_STATE) == DDS_RETCODE_OK) {
        for (unsigned i = 0; i < seq->_length; i++) {
          if (!info->_buffer[i].valid_data) continue;
          unsigned long long h = (unsigned long long) info->_buffer[i].instance_handle;
          int known = 0;
          for (int k = 0; k < nhandles; k++) known |= handles[k] == h;
          if (!known && nhandles < 16) handles[nhandles++] = h;
          nsamples++;
          /* v tells the writers apart: this program writes 1, a peer 2. */
          if (seq->_buffer[i].v == 1) from_v1++;
          if (seq->_buffer[i].v == 2) from_v2++;
        }
        PadKeyDataReader_return_loan(r_PadKey, seq, info);
      }
      usleep(100000);
    }
    printf("PadKey samples=%d instances=%d v1=%d v2=%d\n", nsamples, nhandles, from_v1, from_v2);
  }
  DDS_DomainParticipant_delete_contained_entities(p);
  DDS_DomainParticipantFactory_delete_participant(f, p);
  return 0;
}
