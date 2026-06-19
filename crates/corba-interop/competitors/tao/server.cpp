#include "EchoS.h"
#include <fstream>
class Echo_i : public POA_Echo {
public: char* ping(const char* msg){ return CORBA::string_dup(msg); }
};
int main(int argc, char** argv){
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  CORBA::Object_var pobj = orb->resolve_initial_references("RootPOA");
  PortableServer::POA_var poa = PortableServer::POA::_narrow(pobj.in());
  poa->the_POAManager()->activate();
  Echo_i* svc = new Echo_i();
  PortableServer::ObjectId_var oid = poa->activate_object(svc);
  CORBA::Object_var ref = poa->id_to_reference(oid.in());
  CORBA::String_var ior = orb->object_to_string(ref.in());
  std::ofstream f("/tmp/echo_tao.ior"); f << ior.in() << std::endl; f.close();
  orb->run();
  return 0;
}
