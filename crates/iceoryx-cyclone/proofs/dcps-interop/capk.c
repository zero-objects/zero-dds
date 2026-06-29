#include <iceoryx_binding_c/runtime.h>
#include <iceoryx_binding_c/subscriber.h>
#include <iceoryx_binding_c/chunk.h>
#include <iceoryx_binding_c/service_discovery.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <unistd.h>
typedef struct { uint32_t st,dt,iid,size; uint8_t guid[16]; int64_t ts; uint32_t si; uint16_t cid,cop; } md_t;
static iox_service_description_t f; static int have=0;
static void cb(const iox_service_description_t s){ if(!have && strstr(s.instanceString,"KMsg")){ f=s; have=1; } }
int main(void){
  iox_runtime_init("capk");
  iox_service_discovery_storage_t ds; iox_service_discovery_t d=iox_service_discovery_init(&ds);
  for(int i=0;i<50&&!have;i++){ iox_service_discovery_find_service_apply_callable(d,NULL,NULL,NULL,cb,MessagingPattern_PUB_SUB); if(!have)usleep(200000); }
  if(!have){ printf("no KMsg service\n"); return 1; }
  printf("svc {%s | %s | %s}\n", f.serviceString, f.instanceString, f.eventString);
  iox_sub_options_t o; iox_sub_options_init(&o); o.queueCapacity=16; o.historyRequest=4;
  iox_sub_storage_t ss; iox_sub_t sub=iox_sub_init(&ss,f.serviceString,f.instanceString,f.eventString,&o); iox_sub_subscribe(sub);
  const void*p=NULL;
  for(int i=0;i<100;i++){ while(iox_sub_take_chunk(sub,&p)==ChunkReceiveResult_SUCCESS){
    const iox_chunk_header_t*ch=iox_chunk_header_from_user_payload_const(p);
    const md_t*m=(const md_t*)iox_chunk_header_to_user_header_const(ch);
    const uint8_t*b=(const uint8_t*)p;
    printf("META size=%u state=%u cdr_id=0x%04x cdr_opt=0x%04x si=%u guid=%02x%02x%02x%02x\n",m->size,m->st,m->cid,m->cop,m->si,m->guid[0],m->guid[1],m->guid[2],m->guid[3]);
    printf("PAYLOAD[%u]:",m->size); for(uint32_t k=0;k<m->size&&k<48;k++)printf(" %02x",b[k]); printf("\n");
    iox_sub_release_chunk(sub,p); return 0;
  } usleep(100000); }
  printf("no sample\n"); return 1;
}
