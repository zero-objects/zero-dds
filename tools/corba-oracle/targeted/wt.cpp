#include <cstdio>
#include <tao/CDR.h>
#include <ace/Message_Block.h>
#include "wC.h"
int main(){
  W w; w.head=0x11; w.c=(CORBA::WChar)0x00e4; w.mid=0x22;
  CORBA::WChar t[]={0x0061,0x00e4,0x0000}; w.s=CORBA::wstring_dup(t); w.tail=0xAB;
  TAO_OutputCDR cdr((size_t)0,(int)ACE_CDR_BYTE_ORDER);
  if(!(cdr<<w)){printf("tao refused to marshal\n");return 1;}
  printf("tao sizeof(WChar)=%zu bytes=", sizeof(CORBA::WChar));
  for(const ACE_Message_Block*b=cdr.begin();b;b=b->cont()){const unsigned char*p=(const unsigned char*)b->rd_ptr();for(size_t i=0;i<b->length();i++)printf("%02x",p[i]);}
  printf("\n");
}
