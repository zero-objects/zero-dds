#include <iostream>
#include <fstream>
#include "gapS.h"
#include <tao/PortableServer/PortableServer.h>

class SinkImpl : public POA_Sink {
public:
  void put(const GapProbe &g) override {
    std::cout << "put s.len=" << strlen(g.s.in()) << " b=" << g.b << std::endl;
  }
};

int main(int argc, char **argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  CORBA::Object_var obj = orb->resolve_initial_references("RootPOA");
  PortableServer::POA_var poa = PortableServer::POA::_narrow(obj.in());
  poa->the_POAManager()->activate();
  SinkImpl *impl = new SinkImpl();
  PortableServer::ObjectId_var oid = poa->activate_object(impl);
  CORBA::Object_var ref = poa->id_to_reference(oid.in());
  CORBA::String_var ior = orb->object_to_string(ref.in());
  std::ofstream f("/root/gapprobe/tao/ior.txt");
  f << ior.in() << std::endl;
  f.close();
  orb->run();
  return 0;
}
