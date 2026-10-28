/* cyclone-wire.c — one publisher and one subscriber in one process pair,
 * so that a DATA submessage actually goes on the loopback and tcpdump can
 * see whether PID_KEY_HASH (0x0070) is in its inline QoS.
 *
 *   ./wire pub  Foo|NoKeyFinal|NoKeyMutable
 *   ./wire sub  Foo|NoKeyFinal|NoKeyMutable
 */
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include "dds/dds.h"
#include "keyhash.h"

int main (int argc, char **argv)
{
  if (argc != 3) { fprintf (stderr, "usage: wire pub|sub <type>\n"); return 2; }
  const char *role = argv[1], *tname = argv[2];

  const dds_topic_descriptor_t *td;
  if (strcmp (tname, "Foo") == 0) td = &Foo_desc;
  else if (strcmp (tname, "NoKeyFinal") == 0) td = &NoKeyFinal_desc;
  else if (strcmp (tname, "NoKeyMutable") == 0) td = &NoKeyMutable_desc;
  else { fprintf (stderr, "unknown type %s\n", tname); return 2; }

  dds_entity_t pp = dds_create_participant (0, NULL, NULL);
  if (pp < 0) { fprintf (stderr, "participant: %s\n", dds_strretcode (-pp)); return 1; }
  dds_entity_t tp = dds_create_topic (pp, td, tname, NULL, NULL);
  if (tp < 0) { fprintf (stderr, "topic: %s\n", dds_strretcode (-tp)); return 1; }

  dds_qos_t *q = dds_create_qos ();
  dds_qset_reliability (q, DDS_RELIABILITY_RELIABLE, DDS_SECS (1));
  dds_qset_durability (q, DDS_DURABILITY_TRANSIENT_LOCAL);
  dds_qset_history (q, DDS_HISTORY_KEEP_LAST, 10);

  if (strcmp (role, "sub") == 0)
  {
    dds_entity_t rd = dds_create_reader (pp, tp, q, NULL);
    if (rd < 0) { fprintf (stderr, "reader: %s\n", dds_strretcode (-rd)); return 1; }
    printf ("sub %s ready\n", tname); fflush (stdout);
    for (int i = 0; i < 200; i++) { dds_sleepfor (DDS_MSECS (100)); }
    return 0;
  }

  dds_entity_t wr = dds_create_writer (pp, tp, q, NULL);
  if (wr < 0) { fprintf (stderr, "writer: %s\n", dds_strretcode (-wr)); return 1; }

  /* wait for the reader to match */
  for (int i = 0; i < 100; i++)
  {
    dds_publication_matched_status_t st;
    if (dds_get_publication_matched_status (wr, &st) == 0 && st.current_count > 0) break;
    dds_sleepfor (DDS_MSECS (100));
  }

  dds_return_t rc = DDS_RETCODE_ERROR;
  for (int i = 0; i < 5; i++)
  {
    if (strcmp (tname, "Foo") == 0) {
      Foo s = { .id = 0x12345678, .x = 10, .y = 20 };
      rc = dds_write (wr, &s);
    } else if (strcmp (tname, "NoKeyFinal") == 0) {
      NoKeyFinal s = { .a = 0x12345678, .b = 20 };
      rc = dds_write (wr, &s);
    } else {
      NoKeyMutable s = { .a = 0x12345678, .b = 20 };
      rc = dds_write (wr, &s);
    }
    dds_sleepfor (DDS_MSECS (200));
  }
  printf ("pub %s wrote, rc=%d\n", tname, (int) rc); fflush (stdout);
  dds_sleepfor (DDS_MSECS (500));
  dds_delete (pp);
  return 0;
}
