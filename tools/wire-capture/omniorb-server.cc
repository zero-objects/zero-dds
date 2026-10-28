#include <iostream>
#include <fstream>
#include "gap.hh"

class SinkImpl : public POA_Sink {
public:
  void put(const GapProbe& g) override {
    std::cout << "put s.len=" << strlen(g.s) << " b=" << g.b << std::endl;
  }
};

int main(int argc, char **argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  CORBA::Object_var obj = orb->resolve_initial_references("RootPOA");
  PortableServer::POA_var poa = PortableServer::POA::_narrow(obj);
  SinkImpl *impl = new SinkImpl();
  PortableServer::ObjectId_var oid = poa->activate_object(impl);
  CORBA::Object_var ref = impl->_this();
  CORBA::String_var ior = orb->object_to_string(ref);
  std::ofstream f("/root/gapprobe/omni/ior.txt");
  f << ior << std::endl;
  f.close();
  poa->the_POAManager()->activate();
  orb->run();
  return 0;
}
