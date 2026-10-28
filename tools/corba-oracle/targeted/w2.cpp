#include <cstdio>
#include <omniORB4/CORBA.h>
#include <omniORB4/codeSets.h>
#include "w.hh"
static CORBA::WChar txt[] = { 0x0061, 0x00e4, 0x0000 };
int main(int argc, char **argv){
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  W w; w.head=0x11; w.c=(CORBA::WChar)0x00e4; w.mid=0x22; w.s=CORBA::wstring_dup(txt); w.tail=0xAB;
  cdrMemoryStream ms(0,false); ms.setByteSwapFlag(1);
  try { w >>= ms; } catch (const CORBA::SystemException &e) { printf("omniorb refused: %s\n", e._name()); return 1; }
  const unsigned char*p=(const unsigned char*)ms.bufPtr();
  printf("omniorb-noTCS sizeof(WChar)=%zu bytes=", sizeof(CORBA::WChar));
  for(size_t i=0;i<ms.bufSize();i++)printf("%02x",p[i]);
  printf(" (%u)\n",(unsigned)ms.bufSize());
}
