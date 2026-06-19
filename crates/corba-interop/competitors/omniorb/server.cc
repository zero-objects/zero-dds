#include <omniORB4/CORBA.h>
#include "Echo.hh"
#include <fstream>
class Echo_i : public POA_Echo {
public: char* ping(const char* msg){ return CORBA::string_dup(msg); }
};
int main(int argc, char** argv){
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  CORBA::Object_var pobj = orb->resolve_initial_references("RootPOA");
  PortableServer::POA_var poa = PortableServer::POA::_narrow(pobj);
  Echo_i* svc = new Echo_i();
  poa->activate_object(svc);
  CORBA::Object_var ref = poa->servant_to_reference(svc);
  CORBA::String_var ior = orb->object_to_string(ref);
  std::ofstream f("/tmp/echo_omni.ior"); f << ior << std::endl; f.close();
  poa->the_POAManager()->activate();
  orb->run();
  return 0;
}
