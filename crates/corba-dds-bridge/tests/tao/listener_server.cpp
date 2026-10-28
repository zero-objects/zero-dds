// TAO servant of `tests/tao_e2e.rs`: a Listener reachable as
// corbaloc::127.0.0.1:<port>/Listener (bound in TAO's IORTable); prints every
// on_value and exits after `count` of them or 15 seconds.
#include <cstdlib>
#include <iostream>
#include <string>
#include "CalcS.h"
#include "tao/IORTable/IORTable.h"

static int remaining = 0;

class ListenerImpl : public virtual POA_zerodds_test::Listener {
 public:
  void on_value(CORBA::LongLong v) override {
    std::cout << "on_value " << v << std::endl;
    --remaining;
  }
};

int main(int argc, char** argv) {
  if (argc < 3) { std::cerr << "usage: listener_server <port> <count>\n"; return 2; }
  std::string endpoint = std::string("iiop://127.0.0.1:") + argv[1];
  remaining = std::atoi(argv[2]);
  int orb_argc = 3;
  const char* orb_argv[] = {argv[0], "-ORBListenEndpoints", endpoint.c_str(), nullptr};
  CORBA::ORB_var orb = CORBA::ORB_init(orb_argc, const_cast<char**>(orb_argv));
  CORBA::Object_var obj = orb->resolve_initial_references("RootPOA");
  PortableServer::POA_var poa = PortableServer::POA::_narrow(obj.in());
  PortableServer::Servant_var<ListenerImpl> impl = new ListenerImpl();
  PortableServer::ObjectId_var oid = poa->activate_object(impl.in());
  CORBA::Object_var ref = poa->id_to_reference(oid.in());
  CORBA::String_var ior = orb->object_to_string(ref.in());
  CORBA::Object_var table_obj = orb->resolve_initial_references("IORTable");
  IORTable::Table_var table = IORTable::Table::_narrow(table_obj.in());
  table->bind("Listener", ior.in());
  poa->the_POAManager()->activate();
  std::cout << "ready" << std::endl;
  ACE_Time_Value until = ACE_OS::gettimeofday() + ACE_Time_Value(15);
  while (remaining > 0 && ACE_OS::gettimeofday() < until) {
    ACE_Time_Value slice(0, 100000);
    orb->perform_work(slice);
  }
  poa->destroy(true, true);
  orb->destroy();
  return 0;
}
