// SPDX-License-Identifier: Apache-2.0
// omniORB servants of Features and Shapes (features.idl): writes the
// stringified IORs to stdout (FEATURES_IOR=… / SHAPES_IOR=…) and runs until
// killed; a `fire` prints `FIRED <signal>`. The ZeroDDS-generated stubs call
// it in tests/omniorb_cross_orb.rs.
#include <omniORB4/CORBA.h>
#include "features.hh"
#include <cstdio>
#include <cstring>
#include <string>

class FeaturesImpl : public POA_Features {
  std::string label_ = "init";
public:
  CORBA::Long add(CORBA::Long a, CORBA::Long b) override { return a + b; }
  CORBA::Double scale(CORBA::Double x, CORBA::Double f) override { return x * f; }
  CORBA::Boolean toggle(CORBA::Boolean b) override { return !b; }
  CORBA::Octet xor_byte(CORBA::Octet a, CORBA::Octet b) override { return a ^ b; }
  CORBA::Short neg_short(CORBA::Short s) override { return -s; }
  CORBA::ULong uadd(CORBA::ULong a, CORBA::ULong b) override { return a + b; }
  CORBA::LongLong add64(CORBA::LongLong a, CORBA::LongLong b) override { return a + b; }
  char* concat(const char* a, const char* b) override {
    return CORBA::string_dup((std::string(a) + b).c_str());
  }
  CORBA::WChar* wecho(const CORBA::WChar* w) override { return CORBA::wstring_dup(w); }
  CORBA::Any* aecho(const CORBA::Any& a) override { return new CORBA::Any(a); }
  LongSeq* reverse(const LongSeq& xs) override {
    LongSeq* out = new LongSeq(xs);
    CORBA::ULong n = out->length();
    for (CORBA::ULong i = 0; i < n; i++) (*out)[i] = xs[n - 1 - i];
    return out;
  }
  StringSeq* echo_seq(const StringSeq& xs) override { return new StringSeq(xs); }
  void divmod(CORBA::Long a, CORBA::Long b, CORBA::Long& q, CORBA::Long& r) override {
    q = a / b;
    r = a % b;
  }
  void increment(CORBA::Long& x) override { x = x + 1; }
  void swap(CORBA::Long& a, CORBA::Long& b) override {
    CORBA::Long t = a;
    a = b;
    b = t;
  }
  void fire(CORBA::Long signal) override {
    printf("FIRED %d\n", (int)signal);
    fflush(stdout);
  }
  CORBA::Long answer() override { return 42; }
  char* label() override { return CORBA::string_dup(label_.c_str()); }
  void label(const char* value) override { label_ = value; }
};

class ShapesImpl : public POA_Shapes {
public:
  Point translate(const Point& p, CORBA::Long dx, CORBA::Long dy) override {
    Point out; out.x = p.x + dx; out.y = p.y + dy;
    return out;
  }
  CORBA::Long sum_point(const Point& p) override { return p.x + p.y; }
  void split(const Point& p, CORBA::Long& x, CORBA::Long& y) override { x = p.x; y = p.y; }
  void bump(Point& p) override { p.x += 1; p.y += 1; }
  Color next_color(Color c) override {
    return c == RED ? GREEN : c == GREEN ? BLUE : RED;
  }
  Variant* wrap_long(CORBA::Long v) override {
    Variant* out = new Variant();
    out->as_long(v);
    return out;
  }
  CORBA::Long unwrap_long(const Variant& v) override {
    switch (v._d()) {
      case 0: return v.as_long();
      case 1: return (CORBA::Long)strlen(v.as_string());
      default: return v.as_flag() ? 1 : 0;
    }
  }
  CORBA::Long checked(CORBA::Long idx, CORBA::Long limit) override {
    if (idx < limit) return idx;
    OutOfRange ex; ex.requested = idx; ex.limit = limit;
    throw ex;
  }
};

int main(int argc, char** argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  CORBA::Object_var pobj = orb->resolve_initial_references("RootPOA");
  PortableServer::POA_var poa = PortableServer::POA::_narrow(pobj);
  poa->the_POAManager()->activate();
  CORBA::Object_var fref = poa->servant_to_reference(new FeaturesImpl());
  CORBA::Object_var sref = poa->servant_to_reference(new ShapesImpl());
  CORBA::String_var fior = orb->object_to_string(fref);
  CORBA::String_var sior = orb->object_to_string(sref);
  printf("FEATURES_IOR=%s\n", (const char*)fior);
  printf("SHAPES_IOR=%s\n", (const char*)sior);
  fflush(stdout);
  orb->run();
  return 0;
}
