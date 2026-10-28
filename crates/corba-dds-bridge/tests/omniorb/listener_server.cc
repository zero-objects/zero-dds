// omniORB servant of `tests/omniorb_e2e.rs`: a Listener reachable as
// corbaloc::127.0.0.1:<port>/Listener; prints every on_value and exits
// after `count` of them or 15 seconds.
#include <cstdlib>
#include <iostream>
#include <omnithread.h>
#include "Calc.hh"

static int remaining = 0;
static omni_mutex mu;
static omni_condition done(&mu);

class ListenerImpl : public POA_zerodds_test::Listener {
 public:
  void on_value(CORBA::LongLong v) override {
    omni_mutex_lock l(mu);
    std::cout << "on_value " << v << std::endl;
    if (--remaining <= 0) done.signal();
  }
};

int main(int argc, char** argv) {
  if (argc < 3) { std::cerr << "usage: listener_server <port> <count>\n"; return 2; }
  std::string endpoint = std::string("giop:tcp:127.0.0.1:") + argv[1];
  const char* options[][2] = {{"endPoint", endpoint.c_str()}, {0, 0}};
  remaining = std::atoi(argv[2]);
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv, "omniORB4", options);
  CORBA::Object_var obj = orb->resolve_initial_references("omniINSPOA");
  PortableServer::POA_var poa = PortableServer::POA::_narrow(obj);
  ListenerImpl* impl = new ListenerImpl();
  PortableServer::ObjectId_var oid = PortableServer::string_to_ObjectId("Listener");
  poa->activate_object_with_id(oid, impl);
  poa->the_POAManager()->activate();
  std::cout << "ready" << std::endl;
  {
    omni_mutex_lock l(mu);
    unsigned long s, n;
    omni_thread::get_time(&s, &n, 15, 0);
    while (remaining > 0) { if (!done.timedwait(s, n)) break; }
  }
  orb->destroy();
  return 0;
}
