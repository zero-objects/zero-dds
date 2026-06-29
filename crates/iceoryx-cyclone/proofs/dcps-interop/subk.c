#include "dds/dds.h"
#include "svar.h"
#include <stdio.h>
int main(void){ setvbuf(stdout,NULL,_IONBF,0);
  dds_entity_t pp=dds_create_participant(DDS_DOMAIN_DEFAULT,NULL,NULL);
  dds_entity_t tp=dds_create_topic(pp,&IoxOracle_KMsg_desc,"KMsgTopic",NULL,NULL);
  dds_qos_t*q=dds_create_qos(); dds_qset_reliability(q,DDS_RELIABILITY_RELIABLE,DDS_SECS(1)); dds_qset_history(q,DDS_HISTORY_KEEP_LAST,16);
  dds_entity_t rd=dds_create_reader(pp,tp,q,NULL);
  printf("cyclone KMsg subscriber waiting...\n");
  IoxOracle_KMsg s; void*sp[1]={&s}; dds_sample_info_t si[1];
  for(int i=0;i<200;i++){ dds_return_t n=dds_take(rd,sp,si,1,1);
    if(n>0&&si[0].valid_data){ printf("CYCLONE GOT KMsg id=%u name=%s value=0x%X\n",s.id,s.name,s.value);
      if(s.id==7&&s.value==0xABCD){ printf("CYCLONE OK: Cyclone read the ZeroDDS-DataWriter KMsg over iceoryx SHM\n"); dds_delete(pp); return 0; } }
    dds_sleepfor(DDS_MSECS(50)); }
  dds_delete(pp); printf("CYCLONE FAIL\n"); return 1;
}
