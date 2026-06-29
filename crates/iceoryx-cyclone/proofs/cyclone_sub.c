#include "dds/dds.h"
#include "sample.h"
#include <stdio.h>
int main(void){
  dds_entity_t pp=dds_create_participant(DDS_DOMAIN_DEFAULT,NULL,NULL);
  dds_entity_t tp=dds_create_topic(pp,&IoxOracle_Sample_desc,"IoxOracleTopic",NULL,NULL);
  dds_qos_t*q=dds_create_qos(); dds_qset_reliability(q,DDS_RELIABILITY_RELIABLE,DDS_SECS(1)); dds_qset_history(q,DDS_HISTORY_KEEP_LAST,16);
  dds_entity_t rd=dds_create_reader(pp,tp,q,NULL);
  printf("cyclone subscriber waiting for IoxOracle::Sample...\n");
  IoxOracle_Sample s; void*samples[1]={&s}; dds_sample_info_t si[1];
  for(int i=0;i<200;i++){
    dds_return_t n=dds_take(rd,samples,si,1,1);
    if(n>0 && si[0].valid_data){
      printf("CYCLONE GOT: id=%u value=0x%08x data=%u,%u,..,%u\n", s.id, s.value, s.data[0], s.data[1], s.data[7]);
      if(s.id==2u && s.value==0xCAFEF00Du && s.data[0]==8 && s.data[7]==1){
        printf("CYCLONE OK: Cyclone DDS reader consumed the ZeroDDS-published sample over iceoryx SHM\n");
        dds_delete(pp); return 0;
      }
    }
    dds_sleepfor(DDS_MSECS(50));
  }
  dds_delete(pp); printf("CYCLONE FAIL: no ZeroDDS sample received\n"); return 1;
}
