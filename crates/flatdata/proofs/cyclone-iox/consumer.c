// External cross-vendor iceoryx (C++/POSH) consumer: reads a Cyclone-published
// DDS sample over real iceoryx shared memory via the stable C binding. Depends
// ONLY on libiceoryx_binding_c + the reverse-engineered Cyclone PSMX layout
// (docs/interop/cyclone-iceoryx-shm-ground-truth.md). No Cyclone/DDS library.
#include <iceoryx_binding_c/runtime.h>
#include <iceoryx_binding_c/subscriber.h>
#include <iceoryx_binding_c/chunk.h>
#include <iceoryx_binding_c/service_discovery.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <unistd.h>

// Cyclone dds_psmx_metadata_t (dds/ddsc/dds_psmx.h), the iceoryx user-header.
// VERIFIED field sizes from the Cyclone headers: instance_id is uint32 (NOT
// uint64), sample_state/data_type are uint32, guid is 16 bytes.
typedef struct {
  uint32_t sample_state;   // dds_loaned_sample_state_t enum (2 == RAW_DATA)
  uint32_t data_type;      // reserved, 0
  uint32_t instance_id;    // dds_psmx_instance_id_t (uint32)
  uint32_t sample_size;    // payload byte length
  uint8_t  guid[16];       // original writer GUID
  int64_t  timestamp;      // source timestamp
  uint32_t statusinfo;     // DDSI status (write/dispose/unregister)
  uint16_t cdr_identifier; // RTPS encoding; NATIVE if raw
  uint16_t cdr_options;
} dds_psmx_metadata_t;

static iox_service_description_t found; static int have=0;
static void on_svc(const iox_service_description_t s){
  if(!have && strcmp(s.instanceString,"IoxOracle::Sample")==0){ found=s; have=1; }
}

int main(void){
  iox_runtime_init("zerodds-iox-oracle");
  iox_service_discovery_storage_t dstore; iox_service_discovery_t disc=iox_service_discovery_init(&dstore);
  for(int i=0;i<50 && !have;i++){
    iox_service_discovery_find_service_apply_callable(disc,NULL,NULL,NULL,on_svc,MessagingPattern_PUB_SUB);
    if(!have) usleep(200000);
  }
  if(!have){ printf("ORACLE FAIL: no IoxOracle::Sample iox service registered by Cyclone\n"); return 1; }
  printf("==> subscribing to Cyclone service {%s | %s | %s}\n", found.serviceString, found.instanceString, found.eventString);

  iox_sub_options_t opts; iox_sub_options_init(&opts); opts.queueCapacity=16U; opts.historyRequest=4U;
  iox_sub_storage_t sstore;
  iox_sub_t sub=iox_sub_init(&sstore, found.serviceString, found.instanceString, found.eventString, &opts);
  iox_sub_subscribe(sub);

  const void* payload=NULL; int got=0;
  for(int i=0;i<100 && !got;i++){
    while(iox_sub_take_chunk(sub,&payload)==ChunkReceiveResult_SUCCESS){
      const iox_chunk_header_t* ch=iox_chunk_header_from_user_payload_const(payload);
      const dds_psmx_metadata_t* md=(const dds_psmx_metadata_t*)iox_chunk_header_to_user_header_const(ch);
      const uint8_t* p=(const uint8_t*)payload;
      printf("METADATA: sample_size=%u state=%u cdr_id=0x%04x statusinfo=%u guid=%02x%02x%02x%02x..\n",
             md->sample_size, md->sample_state, md->cdr_identifier, md->statusinfo,
             md->guid[0],md->guid[1],md->guid[2],md->guid[3]);
      printf("PAYLOAD[%u]:", md->sample_size);
      for(uint32_t k=0;k<md->sample_size && k<32;k++) printf(" %02x", p[k]);
      printf("\n");
      uint32_t id, val; memcpy(&id,p+0,4); memcpy(&val,p+4,4);
      int data_ok=1; for(int k=0;k<8;k++) if(p[8+k]!=(uint8_t)(k+1)) data_ok=0;
      if(md->sample_size==16u && md->sample_state==2u && id==1u && val==0x12345678u && data_ok){
        printf("ORACLE OK: external iceoryx(C++) consumer decoded the Cyclone sample over SHM (id=1 value=0x12345678 data=01..08, raw self-contained, size=16)\n");
        got=1;
      }
      iox_sub_release_chunk(sub,payload);
      if(got) break;
    }
    if(!got) usleep(100000);
  }
  iox_sub_deinit(sub);
  return got?0:1;
}
