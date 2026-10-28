#include <cstdio>
#include <omniORB4/CORBA.h>
#include <omniORB4/codeSets.h>
#include "w.hh"
static CORBA::WChar txt[] = { 0x0061, 0x0062, 0x0000 };
static void one(int minor, CORBA::ULong id, const char *label) {
  W w; w.head=0x11; w.c=(CORBA::WChar)0x0061; w.mid=0x22; w.s=CORBA::wstring_dup(txt); w.tail=0xAB;
  cdrMemoryStream ms(0,false); ms.setByteSwapFlag(1);
  GIOP::Version v; v.major=1; v.minor=(CORBA::Octet)minor;
  _OMNI_NS(omniCodeSet)::TCS_W *tcs = _OMNI_NS(omniCodeSet)::getTCS_W(id, v);
  if (!tcs) { printf("%s: no TCS\n", label); return; }
  ms.TCS_W(tcs);
  try { w >>= ms; } catch (const CORBA::SystemException &e) { printf("%s refused: %s\n", label, e._name()); return; }
  const unsigned char*p=(const unsigned char*)ms.bufPtr();
  printf("%-28s ", label); for(size_t i=0;i<ms.bufSize();i++)printf("%02x",p[i]); printf(" (%u)\n",(unsigned)ms.bufSize());
}
int main(int argc,char**argv){
  CORBA::ORB_var orb=CORBA::ORB_init(argc,argv);
  one(2, _OMNI_NS(omniCodeSet)::ID_UTF_16, "giop1.2 utf-16");
  one(1, _OMNI_NS(omniCodeSet)::ID_UTF_16, "giop1.1 utf-16");
  one(2, 0x00010100u, "giop1.2 ucs-2");
  one(1, 0x00010100u, "giop1.1 ucs-2");
  one(2, 0x00010104u, "giop1.2 ucs-4");
  one(1, 0x00010104u, "giop1.1 ucs-4");
  return 0;
}
