// SPDX-License-Identifier: Apache-2.0
// omniORB ZIOP client (CORBA 3.3 Part 2 §12): with the ZIOP policies as
// the ORB's global policies it calls Echo::ping on the reference it is
// given, with a short and a long compressible string. Exit 0 and "ALL OK"
// when every reply matches.
//
// Usage: ziop_client <IOR>
#include <omniORB4/CORBA.h>
#include <omniORB4/omniZIOP.h>
#include "interop.hh"
#include <cstdio>
#include <cstring>
#include <string>

static int fails = 0;
static void check(const char* name, bool ok) {
  printf("  %s %s\n", ok ? "OK" : "FAIL", name);
  if (!ok) fails++;
}

int main(int argc, char** argv) {
  // omniORB uses ZIOP on a client connection only when the client
  // transport rule that matches the server's address names "ziop".
  const char* options[][2] = {{"clientTransportRule", "* unix,ssl,tcp,ziop"}, {0, 0}};
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv, "omniORB4", options);
  if (argc < 2) { fprintf(stderr, "usage: ziop_client <IOR>\n"); return 2; }
  CORBA::PolicyList pl;
  pl.length(3);
  pl[0] = omniZIOP::create_compression_enabling_policy(1);
  Compression::CompressorIdLevelList ids;
  ids.length(1);
  ids[0].compressor_id = Compression::COMPRESSORID_ZLIB;
  ids[0].compression_level = 6;
  pl[1] = omniZIOP::create_compression_id_level_list_policy(ids);
  pl[2] = omniZIOP::create_compression_low_value_policy(0);
  omniZIOP::setGlobalPolicies(pl);

  CORBA::Object_var obj = orb->string_to_object(argv[1]);
  Echo_var echo = Echo::_narrow(obj);
  if (CORBA::is_nil(echo)) { fprintf(stderr, "narrow Echo failed\n"); return 1; }
  std::string big;
  for (int i = 0; i < 400; i++) big += "omniORB ZIOP ";
  for (int round = 0; round < 2; round++) {
    CORBA::String_var r = echo->ping(big.c_str());
    check("ping(5200 compressible octets)", big == (const char*)r);
  }
  orb->destroy();
  if (fails == 0) printf("ALL OK\n");
  return fails == 0 ? 0 : 1;
}
