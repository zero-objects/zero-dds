// SPDX-License-Identifier: Apache-2.0
// omniORB cross-ORB interop server: implements Echo + Bench, writes the
// stringified IORs to stdout (ECHO_IOR=… / BENCH_IOR=…), runs until killed.
//
// Counterpart: the ZeroDDS client (interop_client echo|bench <IOR>) calls this
// server — proof that ZeroDDS drives a foreign-ORB server.
#include <omniORB4/CORBA.h>
#include "interop.hh"
#include <cstdio>
#include <cstring>
#include <string>
#include <algorithm>

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
    q = a / b;
    r = a % b;
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
  PortableServer::POA_var poa = PortableServer::POA::_narrow(pobj);
  PortableServer::POAManager_var mgr = poa->the_POAManager();
  mgr->activate();

  EchoImpl* echo = new EchoImpl();
  BenchImpl* bench = new BenchImpl();
  CORBA::Object_var eref = poa->servant_to_reference(echo);
  CORBA::Object_var bref = poa->servant_to_reference(bench);
  CORBA::String_var eior = orb->object_to_string(eref);
  CORBA::String_var bior = orb->object_to_string(bref);

  printf("ECHO_IOR=%s\n", (const char*)eior);
  printf("BENCH_IOR=%s\n", (const char*)bior);
  fflush(stdout);

  orb->run();
  return 0;
}
