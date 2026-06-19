// SPDX-License-Identifier: Apache-2.0
// TAO CosNaming cross-ORB client: narrows a NamingContext IOR (published by
// ZeroDDS) and drives the real OMG CosNaming wire. The CosNaming stubs are
// generated via tao_idl from competitors/cosnaming.idl (cosnamingC.{h,cpp}) —
// NO orbsvcs/TAO_CosNaming needed, links only against TAO core.
// Usage: naming_client <NAMING_IOR>
#include "cosnamingC.h"
#include <cstdio>

static int fails = 0;
static void check(const char* name, bool ok) {
  printf("  %s %s\n", ok ? "OK" : "FAIL", name);
  if (!ok) fails++;
}

int main(int argc, char** argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  if (argc < 2) { fprintf(stderr, "usage: naming_client <IOR>\n"); return 2; }
  CORBA::Object_var obj = orb->string_to_object(argv[1]);
  CosNaming::NamingContext_var nc = CosNaming::NamingContext::_narrow(obj.in());
  if (CORBA::is_nil(nc.in())) { fprintf(stderr, "narrow NamingContext failed\n"); return 1; }

  CosNaming::Name name;
  name.length(1);
  name[0].id = CORBA::string_dup("tao-test");
  name[0].kind = CORBA::string_dup("obj");

  nc->bind(name, nc.in());
  CORBA::Object_var got = nc->resolve(name);
  check("resolve returns non-nil ref", !CORBA::is_nil(got.in()));
  nc->rebind(name, nc.in());
  nc->unbind(name);
  check("bind/resolve/rebind/unbind roundtrip", true);

  // resolve after unbind → typed NotFound exception (cross-ORB).
  bool caught = false;
  try { nc->resolve(name); }
  catch (const CosNaming::NamingContext::NotFound&) { caught = true; }
  check("resolve(unbound) → NotFound", caught);

  // Federation: bind_new_context + leaf via sub-ref + compound resolve from root.
  CosNaming::Name ctxName; ctxName.length(1);
  ctxName[0].id = CORBA::string_dup("sub"); ctxName[0].kind = CORBA::string_dup("ctx");
  CosNaming::NamingContext_var sub = nc->bind_new_context(ctxName);
  CosNaming::Name leaf; leaf.length(1);
  leaf[0].id = CORBA::string_dup("leaf"); leaf[0].kind = CORBA::string_dup("obj");
  sub->bind(leaf, nc.in());
  CosNaming::Name compound; compound.length(2);
  compound[0].id = CORBA::string_dup("sub"); compound[0].kind = CORBA::string_dup("ctx");
  compound[1].id = CORBA::string_dup("leaf"); compound[1].kind = CORBA::string_dup("obj");
  CORBA::Object_var fedGot = nc->resolve(compound);
  check("federation: bind_new_context + compound resolve", !CORBA::is_nil(fedGot.in()));

  printf("%s\n", fails == 0 ? "ALL OK" : "FAILURES");
  return fails == 0 ? 0 : 1;
}
