#include "dds/dds.h"
#include "sample.h"
#include <stdio.h>
#include <unistd.h>
int main(void) {
  dds_entity_t pp = dds_create_participant(DDS_DOMAIN_DEFAULT, NULL, NULL);
  if (pp < 0) { printf("participant: %s\n", dds_strretcode(-pp)); return 1; }
  dds_entity_t tp = dds_create_topic(pp, &IoxOracle_Sample_desc, "IoxOracleTopic", NULL, NULL);
  if (tp < 0) { printf("topic: %s\n", dds_strretcode(-tp)); return 1; }
  dds_qos_t *q = dds_create_qos();
  dds_qset_reliability(q, DDS_RELIABILITY_RELIABLE, DDS_SECS(1));
  dds_qset_history(q, DDS_HISTORY_KEEP_LAST, 16);
  dds_entity_t wr = dds_create_writer(pp, tp, q, NULL);
  if (wr < 0) { printf("writer: %s\n", dds_strretcode(-wr)); return 1; }
  IoxOracle_Sample s; s.id = 1u; s.value = 0x12345678u;
  for (int i=0;i<8;i++) s.data[i]=(uint8_t)(i+1);
  printf("publishing IoxOracle::Sample id=1 value=0x12345678 data=01..08\n");
  for (int i=0;i<200;i++){ dds_return_t r=dds_write(wr,&s); if(r<0){printf("write: %s\n",dds_strretcode(-r));} dds_sleepfor(DDS_MSECS(50)); }
  dds_delete(pp); return 0;
}
