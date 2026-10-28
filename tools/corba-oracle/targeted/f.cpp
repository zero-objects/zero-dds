#include <cstdio>
#include <omniORB4/CORBA.h>
#include "f.hh"
int main(int argc,char**argv){
  CORBA::ORB_var orb = CORBA::ORB_init(argc,argv);
  F s; s.head=0x11; s.m=CORBA::Fixed("123.456"); s.p=CORBA::Fixed("-1.25");
  s.b=CORBA::Fixed("1234567890123456789012345678901"); s.c=BLUE; s.t=1; s.off=0; s.tail=0xAB;
  cdrMemoryStream ms(0,false); ms.setByteSwapFlag(1); s >>= ms;
  const unsigned char*p=(const unsigned char*)ms.bufPtr();
  printf("omniorb=");for(size_t i=0;i<ms.bufSize();i++)printf("%02x",p[i]);printf(" (%u)\n",(unsigned)ms.bufSize());
}
