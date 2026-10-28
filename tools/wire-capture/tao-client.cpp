#include <iostream>
#include <string>
#include <unistd.h>
#include "gapC.h"

int main(int argc, char **argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  CORBA::Object_var obj = orb->string_to_object(argv[1]);
  Sink_var sink = Sink::_narrow(obj.in());

  GapProbe a;
  a.s = CORBA::string_dup(std::string(20, 0x41).c_str());
  a.b = 0x11223344; a.c = 0x55; a.d = 1.5; a.e = 0x66; a.f = 0x7788;
  GapProbe b;
  b.s = CORBA::string_dup(std::string(16, 0x42).c_str());
  b.b = 0x01020304; b.c = 0x05; b.d = 2.5; b.e = 0x06; b.f = 0x0708;

  for (int i = 0; i < 5; i++) {
    sink->put(a); usleep(150000);
    sink->put(b); usleep(150000);
  }
  std::cout << "sent 10 calls" << std::endl;
  return 0;
}
