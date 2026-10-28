/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The probe behind tests/serialized_key_measured.rs: the serializedKey a
 * Cyclone DDS 11.0.1 writer puts behind a dispose, read out of the same
 * call Cyclone's writer makes, ddsi_serdata_from_sample(sertype, SDK_KEY,
 * &sample), with ddsi_serdata_to_ser. SDK_DATA is printed next to it for
 * comparison. One row per type and data representation (the writer's
 * QoS selects XCDR1 or XCDR2); little endian on an x86 host. The first
 * four bytes of every row are the encapsulation header.
 *
 *   idlc serialized_key_measured.idl
 *   cc -o probe serialized_key_measured_probe.c serialized_key_measured.c \
 *      -I. -I$CYCLONE/include -L$CYCLONE/lib -lddsc -Wl,-rpath,$CYCLONE/lib
 *   CYCLONEDDS_URI='<General><Interfaces><NetworkInterface address="127.0.0.1"/></Interfaces></General>' ./probe
 *
 * N under XCDR1 prints "topic failed -3": Cyclone refuses the topic.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dds/dds.h"
#include "dds/ddsi/ddsi_serdata.h"
#include "dds/ddsi/ddsi_sertype.h"
#include "serialized_key_measured.h"
static dds_entity_t pp;
static void dump(const char*name,const dds_topic_descriptor_t*td,const void*s){
  for(int v=0;v<2;v++){
    dds_qos_t*q=dds_create_qos(); dds_data_representation_id_t rep=v?DDS_DATA_REPRESENTATION_XCDR2:DDS_DATA_REPRESENTATION_XCDR1;
    dds_qset_data_representation(q,1,&rep);
    char tn[64]; snprintf(tn,sizeof tn,"%s_%d",name,v);
    dds_entity_t t=dds_create_topic(pp,td,tn,q,NULL);
    if(t<0){printf("%s xcdr%d topic failed %d\n",name,v+1,t);dds_delete_qos(q);continue;}
    dds_entity_t w=dds_create_writer(pp,t,q,NULL);
    const struct ddsi_sertype*st; dds_get_entity_sertype(w,&st);
    for(int kind=0;kind<2;kind++){
      struct ddsi_serdata*sd=ddsi_serdata_from_sample(st,kind?SDK_KEY:SDK_DATA,s);
      if(!sd){printf("%s xcdr%d %s NULL\n",name,v+1,kind?"key":"data");continue;}
      uint32_t n=ddsi_serdata_size(sd); unsigned char*b=malloc(n); ddsi_serdata_to_ser(sd,0,n,b);
      printf("%s xcdr%d %s ",name,v+1,kind?"key ":"data"); for(uint32_t i=0;i<n;i++)printf("%02x",b[i]); printf("\n");
      free(b); ddsi_serdata_unref(sd);
    }
    dds_delete_qos(q);
  }
}
int main(void){
  pp=dds_create_participant(DDS_DOMAIN_DEFAULT,NULL,NULL);
  K_A a={.x=1,.id=7,.y=2}; dump("A",&K_A_desc,&a);
  K_F fz={.x=1,.id=7}; dump("F",&K_F_desc,&fz);
  K_M mm={.x=1,.id=7}; dump("M",&K_M_desc,&mm);
  K_N n={.x=1,.k={.a=3,.b=9}}; dump("N",&K_N_desc,&n);
  K_S s={.name="ab",.x=5,.id=6}; dump("S",&K_S_desc,&s);
  O_R r={.a=1,.x=5,.b=2}; dump("R",&O_R_desc,&r);
  O_Q qq={.a=1,.x=5,.b=2}; dump("Q",&O_Q_desc,&qq);
  dds_delete(pp); return 0;}
