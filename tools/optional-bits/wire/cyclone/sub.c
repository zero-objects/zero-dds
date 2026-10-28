#include <stdio.h>
#include "dds/dds.h"
#include "gen.h"
int main(void) {
    dds_entity_t p = dds_create_participant(7, NULL, NULL);
    dds_entity_t t = dds_create_topic(p, &One_desc, "PadOne", NULL, NULL);
    dds_qos_t *q = dds_create_qos();
    dds_qset_reliability(q, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));
    dds_entity_t r = dds_create_reader(p, t, q, NULL);
    dds_delete_qos(q);
    void *sm[1]; One one; sm[0] = &one; dds_sample_info_t si[1];
    for (int i = 0; i < 40; i++) { dds_take(r, sm, si, 1, 1); dds_sleepfor(DDS_MSECS(200)); }
    dds_delete(p);
    return 0;
}
