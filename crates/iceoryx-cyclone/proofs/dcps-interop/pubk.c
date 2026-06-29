#include "dds/dds.h"
#include "svar.h"
#include <stdio.h>
int main(void){
  dds_entity_t pp=dds_create_participant(DDS_DOMAIN_DEFAULT,NULL,NULL);
  dds_entity_t tp=dds_create_topic(pp,&IoxOracle_KMsg_desc,"KMsgTopic",NULL,NULL);
  dds_qos_t*q=dds_create_qos(); dds_qset_reliability(q,DDS_RELIABILITY_RELIABLE,DDS_SECS(1)); dds_qset_history(q,DDS_HISTORY_KEEP_LAST,16);
  dds_entity_t wr=dds_create_writer(pp,tp,q,NULL);
  IoxOracle_KMsg s; s.id=7; s.name=(char*)"hello-cyclone"; s.value=0xABCD;
  printf("pub KMsg id=7 name=hello-cyclone value=0xABCD\n");
  for(int i=0;i<100;i++){ dds_write(wr,&s); dds_sleepfor(DDS_MSECS(50)); }
  dds_delete(pp); return 0;
}
