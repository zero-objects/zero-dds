/* SPDX-License-Identifier: Apache-2.0
 * Proves the CODEGEN-emitted @appendable/nested/seq<struct> codec (nested_gen.c
 * from nested.idl) is byte-identical to the Rust core (ADR 0013). */
#include <stdio.h>
#include <string.h>
#include "nested_gen.h"
static long rd(const char*p,unsigned char*b,long c){FILE*f=fopen(p,"rb");long n;if(!f){fprintf(stderr,"open %s\n",p);return -1;}n=(long)fread(b,1,(size_t)c,f);fclose(f);return n;}
static int check(int en,const char*gp,const char*tag){
  Outer o,d; unsigned char out[256],g[256]; long gn; size_t i; zdw_writer w; zdw_reader r;
  o.id=0xCAFEBABEuL; o.one.a=0x1111u; o.one.b=0x22223333uL;
  o.many[0].a=0xAAAAu; o.many[0].b=0xBBBBCCCCuL; o.many[1].a=0xDDDDu; o.many[1].b=0xEEEEFFFFuL; o.many_len=2;
  strcpy(o.label,"nested");
  zdw_writer_init(&w,out,sizeof(out),en);
  if(outer_encode(&w,&o)!=ZDW_OK){fprintf(stderr,"%s: enc err\n",tag);return 1;}
  gn=rd(gp,g,(long)sizeof(g)); if(gn<0)return 1;
  if((long)w.len!=gn||memcmp(out,g,(size_t)gn)!=0){fprintf(stderr,"%s: mismatch gen=%lu golden=%ld\n",tag,(unsigned long)w.len,gn);return 1;}
  printf("%s: %lu bytes byte-identical to Rust golden (GENERATED @appendable/nested)\n",tag,(unsigned long)w.len);
  memset(&d,0,sizeof(d)); zdw_reader_init(&r,out,w.len,en);
  if(outer_decode(&r,&d)!=ZDW_OK){fprintf(stderr,"%s: dec err\n",tag);return 1;}
  if(d.id!=o.id||d.one.a!=o.one.a||d.one.b!=o.one.b||d.many_len!=o.many_len||d.many[1].b!=o.many[1].b||strcmp(d.label,o.label)!=0){fprintf(stderr,"%s: rt mismatch\n",tag);return 1;}
  printf("%s: round-trip decode ok\n",tag); (void)i; return 0;
}
int main(int ac,char**av){int rc=0;if(ac<3){fprintf(stderr,"usage: %s le be\n",av[0]);return 2;}rc|=check(ZDW_LE,av[1],"LE");rc|=check(ZDW_BE,av[2],"BE");if(rc==0)printf("ALL OK\n");return rc;}
