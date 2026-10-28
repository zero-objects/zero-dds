#include <stdio.h>
#include <string.h>
#include "dds/dds.h"
#include "gen.h"
int main(void) {
    dds_entity_t p = dds_create_participant(7, NULL, NULL);
    dds_entity_t t = dds_create_topic(p, &TypeB_desc, "PadB", NULL, NULL);
    dds_qos_t *q = dds_create_qos();
    dds_qset_reliability(q, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));
    dds_entity_t w = dds_create_writer(p, t, q, NULL);
    dds_delete_qos(q);
    for (int i = 0; i < 40; i++) {
        TypeB s; memset(&s, 0, sizeof s); s.a = 0x23; s.b = (char)0x62;
        dds_write(w, &s);
        dds_sleepfor(DDS_MSECS(200));
    }
    dds_delete(p);
    return 0;
}
