#include <stdio.h>
#include <string.h>
#include "ndds/ndds_c.h"
#include "gen.h"
#include "genSupport.h"
int main(int argc, char **argv) {
    const int is_pub = (argc > 1 && argv[1][0] == (char)0x70);
    struct DDS_DomainParticipantQos pq = DDS_DomainParticipantQos_INITIALIZER;
    DDS_DomainParticipantFactory_get_default_participant_qos(DDS_TheParticipantFactory, &pq);
    pq.transport_builtin.mask = DDS_TRANSPORTBUILTIN_UDPv4;
    DDS_DomainParticipant *p = DDS_DomainParticipantFactory_create_participant(
        DDS_TheParticipantFactory, 9, &pq, NULL, DDS_STATUS_MASK_NONE);
    if (!p) { printf("no participant\n"); return 1; }
    const char *tn = OneTypeSupport_get_type_name();
    OneTypeSupport_register_type(p, tn);
    DDS_Topic *t = DDS_DomainParticipant_create_topic(p, "PadOne", tn,
        &DDS_TOPIC_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
    if (is_pub) {
        DDS_DataWriter *w = DDS_DomainParticipant_create_datawriter(p, t,
            &DDS_DATAWRITER_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
        OneDataWriter *ow = OneDataWriter_narrow(w);
        struct DDS_PublicationMatchedStatus st;
        for (int i = 0; i < 200; i++) {
            DDS_DataWriter_get_publication_matched_status(w, &st);
            if (st.current_count > 0) break;
            NDDS_Utility_sleep(&(struct DDS_Duration_t){0, 100000000});
        }
        printf("matched=%d\n", (int)st.current_count);
        for (int i = 0; i < 20; i++) {
            One s; One_initialize(&s); s.a = 0xab;
            OneDataWriter_write(ow, &s, &DDS_HANDLE_NIL);
            NDDS_Utility_sleep(&(struct DDS_Duration_t){0, 200000000});
        }
    } else {
        DDS_DomainParticipant_create_datareader(p, DDS_Topic_as_topicdescription(t),
            &DDS_DATAREADER_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
        NDDS_Utility_sleep(&(struct DDS_Duration_t){30, 0});
    }
    return 0;
}
