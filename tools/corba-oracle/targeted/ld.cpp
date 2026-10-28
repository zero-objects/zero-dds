#include <cstdio>
#include <omniORB4/CORBA.h>
#include "ld.hh"
int main(){
  S s; s.head=0x11; s.tail=0xAB;
  s.d = 1.5L;
  cdrMemoryStream ms(0,false); ms.setByteSwapFlag(1); s >>= ms;
  const unsigned char*p=(const unsigned char*)ms.bufPtr();
  printf("omniorb sizeof(long double)=%zu sizeof(CORBA::LongDouble)=%zu bytes=", sizeof(long double), sizeof(CORBA::LongDouble));
  for(size_t i=0;i<ms.bufSize();i++)printf("%02x",p[i]);
  printf(" (%u)\n",(unsigned)ms.bufSize());
}
