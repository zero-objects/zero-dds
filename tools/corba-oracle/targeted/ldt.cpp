#include <cstdio>
#include <tao/CDR.h>
#include <ace/Message_Block.h>
#include "ldC.h"
int main(){
  S s; s.head=0x11; s.tail=0xAB; s.d=1.5L;
  TAO_OutputCDR cdr((size_t)0,(int)ACE_CDR_BYTE_ORDER);
  if(!(cdr<<s)){printf("tao refused\n");return 1;}
  printf("tao sizeof(ACE_CDR::LongDouble)=%zu bytes=", sizeof(ACE_CDR::LongDouble));
  for(const ACE_Message_Block*b=cdr.begin();b;b=b->cont()){const unsigned char*p=(const unsigned char*)b->rd_ptr();for(size_t i=0;i<b->length();i++)printf("%02x",p[i]);}
  printf("\n");
}
