#include <cstdio>
#include <tao/CDR.h>
#include <ace/Message_Block.h>
#include "wC.h"
static void one(int minor, const char *label) {
  W w; w.head=0x11; w.c=(CORBA::WChar)0x0061; w.mid=0x22;
  CORBA::WChar t[]={0x0061,0x0062,0x0000}; w.s=CORBA::wstring_dup(t); w.tail=0xAB;
  TAO_OutputCDR cdr((size_t)0,(int)ACE_CDR_BYTE_ORDER,(ACE_Allocator*)0,(ACE_Allocator*)0,(ACE_Allocator*)0,(size_t)0,(ACE_CDR::Octet)1,(ACE_CDR::Octet)minor);
  if(!(cdr<<w)){printf("%s: tao refused\n",label);return;}
  printf("%-14s ",label);
  for(const ACE_Message_Block*b=cdr.begin();b;b=b->cont()){const unsigned char*p=(const unsigned char*)b->rd_ptr();for(size_t i=0;i<b->length();i++)printf("%02x",p[i]);}
  printf("\n");
}
int main(){ one(0,"giop 1.0"); one(1,"giop 1.1"); one(2,"giop 1.2"); }
