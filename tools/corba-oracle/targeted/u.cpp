#include <cstdio>
#include <omniORB4/CORBA.h>
#include "u.hh"
int main(int argc,char**argv){
  CORBA::ORB_var orb=CORBA::ORB_init(argc,argv);
  S s; s.head=0x11;
  s.a.one(7);                 /* case 1 */
  s.b.two(1.5); s.b._d(3);    /* second label of a two-label case */
  s.c.other(9); s.c._d(99);   /* default reached by a value of its own */
  s.d._d(42);                 /* a discriminator that selects no member */
  s.e.one(5);
  s.tail=0xAB;
  cdrMemoryStream ms(0,false); ms.setByteSwapFlag(1); s >>= ms;
  const unsigned char*p=(const unsigned char*)ms.bufPtr();
  printf("omniorb=");for(size_t i=0;i<ms.bufSize();i++)printf("%02x",p[i]);printf(" (%u)\n",(unsigned)ms.bufSize());
}
