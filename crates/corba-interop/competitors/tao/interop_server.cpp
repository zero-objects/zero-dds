// SPDX-License-Identifier: Apache-2.0
// TAO cross-ORB interop server: Echo + Bench, prints IORs to stdout.
#include "interopS.h"
#include <cstdio>
#include <string>

class EchoImpl : public POA_Echo {
public:
  char* ping(const char* msg) override { return CORBA::string_dup(msg); }
};

class BenchImpl : public POA_Bench {
public:
  CORBA::Long add(CORBA::Long a, CORBA::Long b) override { return a + b; }
  CORBA::Double scale(CORBA::Double x, CORBA::Double f) override { return x * f; }
  CORBA::LongLong add64(CORBA::LongLong a, CORBA::LongLong b) override { return a + b; }
  CORBA::Char next_char(CORBA::Char c) override { return (CORBA::Char)(c + 1); }
  char* concat(const char* a, const char* b) override {
    std::string s = std::string(a) + b;
    return CORBA::string_dup(s.c_str());
  }
  CORBA::WChar* wecho(const CORBA::WChar* w) override { return CORBA::wstring_dup(w); }
  CORBA::Any* aecho(const CORBA::Any& a) override { return new CORBA::Any(a); }
  LongSeq* reverse(const LongSeq& xs) override {
    LongSeq* out = new LongSeq(xs);
    for (CORBA::ULong i = 0; i < out->length() / 2; i++) {
      CORBA::Long t = (*out)[i];
      (*out)[i] = (*out)[out->length() - 1 - i];
      (*out)[out->length() - 1 - i] = t;
    }
    return out;
  }
  void divmod(CORBA::Long a, CORBA::Long b, CORBA::Long& q, CORBA::Long& r) override {
    q = a / b; r = a % b;
  }
  void increment(CORBA::Long& x) override { x = x + 1; }
  CORBA::Object_ptr echo_ref(CORBA::Object_ptr o) override {
    return CORBA::Object::_duplicate(o);
  }
  CORBA::Long checked(CORBA::Long idx, CORBA::Long limit) override {
    if (idx < limit) return idx;
    RangeError ex; ex.requested = idx; ex.limit = limit;
    throw ex;
  }
};

int main(int argc, char** argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  CORBA::Object_var pobj = orb->resolve_initial_references("RootPOA");
  PortableServer::POA_var poa = PortableServer::POA::_narrow(pobj.in());
  poa->the_POAManager()->activate();

  EchoImpl* echo = new EchoImpl();
  BenchImpl* bench = new BenchImpl();
  PortableServer::ObjectId_var eoid = poa->activate_object(echo);
  PortableServer::ObjectId_var boid = poa->activate_object(bench);
  CORBA::Object_var eref = poa->id_to_reference(eoid.in());
  CORBA::Object_var bref = poa->id_to_reference(boid.in());
  CORBA::String_var eior = orb->object_to_string(eref.in());
  CORBA::String_var bior = orb->object_to_string(bref.in());
  printf("ECHO_IOR=%s\n", eior.in());
  printf("BENCH_IOR=%s\n", bior.in());
  fflush(stdout);

  orb->run();
  return 0;
}
