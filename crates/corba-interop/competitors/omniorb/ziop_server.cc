// SPDX-License-Identifier: Apache-2.0
// omniORB ZIOP server (CORBA 3.3 Part 2 §12): Echo with the ZIOP policies
// as the ORB's global policies and in its reference (omniZIOP's
// setServerPolicies puts them into the TAG_POLICIES component). Prints
// ECHO_IOR=… and runs until killed.
//
// Counterpart: a ZeroDDS client with ZIOP policies calls ping with a long
// string (tests/omniorb_cross_orb.rs); a relay records whether the
// request and the reply are ZIOP messages.
#include <omniORB4/CORBA.h>
#include <omniORB4/omniZIOP.h>
#include "interop.hh"
#include <cstdio>

class EchoImpl : public POA_Echo {
public:
  char* ping(const char* msg) override { return CORBA::string_dup(msg); }
};

static CORBA::PolicyList ziop_policies() {
  CORBA::PolicyList pl;
  pl.length(3);
  pl[0] = omniZIOP::create_compression_enabling_policy(1);
  Compression::CompressorIdLevelList ids;
  ids.length(1);
  ids[0].compressor_id = Compression::COMPRESSORID_ZLIB;
  ids[0].compression_level = 6;
  pl[1] = omniZIOP::create_compression_id_level_list_policy(ids);
  pl[2] = omniZIOP::create_compression_low_value_policy(0);
  return pl;
}

int main(int argc, char** argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  CORBA::PolicyList pl = ziop_policies();
  omniZIOP::setGlobalPolicies(pl);
  CORBA::Object_var pobj = orb->resolve_initial_references("RootPOA");
  PortableServer::POA_var poa = PortableServer::POA::_narrow(pobj);
  PortableServer::POAManager_var mgr = poa->the_POAManager();
  mgr->activate();

  EchoImpl* echo = new EchoImpl();
  CORBA::Object_var ref = poa->servant_to_reference(echo);
  CORBA::Object_var zref = omniZIOP::setServerPolicies(ref, pl);
  CORBA::String_var ior = orb->object_to_string(zref);
  printf("ECHO_IOR=%s\n", (const char*)ior);
  fflush(stdout);

  orb->run();
  return 0;
}
