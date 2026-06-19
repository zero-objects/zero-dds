// SPDX-License-Identifier: Apache-2.0
// omniORB CosNaming cross-ORB client: narrows a NamingContext IOR (published by
// ZeroDDS) and drives it over the real OMG CosNaming wire
// (bind/resolve/rebind/unbind of a Name=sequence<NameComponent>). Proves that a
// foreign-ORB client drives our NamingContext server.
//
// Usage: naming_client <NAMING_IOR>
#include <omniORB4/CORBA.h>
// omniORB's built-in CosNaming stubs (Naming.hh); the stub implementation lives
// in libomniORB4 itself (no separate libCOS4 needed — just do NOT link -lCOS4).
#include <omniORB4/Naming.hh>
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
  CosNaming::NamingContext_var nc = CosNaming::NamingContext::_narrow(obj);
  if (CORBA::is_nil(nc)) { fprintf(stderr, "narrow NamingContext failed\n"); return 1; }

  // Single-level name → root-level binding (no intermediate context needed).
  CosNaming::Name name;
  name.length(1);
  name[0].id = CORBA::string_dup("omni-test");
  name[0].kind = CORBA::string_dup("obj");

  // bind the NamingContext ref itself, then resolve.
  nc->bind(name, nc);
  CORBA::Object_var got = nc->resolve(name);
  check("resolve returns non-nil ref", !CORBA::is_nil(got));
  // rebind overwrites without AlreadyBound.
  nc->rebind(name, nc);
  // unbind removes the binding.
  nc->unbind(name);
  check("bind/resolve/rebind/unbind roundtrip", true);

  // resolve after unbind → typed NotFound exception (cross-ORB):
  // the ZeroDDS server throws it with RepoId IDL:omg.org/CosNaming/NamingContext/NotFound:1.0.
  bool caught = false;
  try { nc->resolve(name); }
  catch (const CosNaming::NamingContext::NotFound&) { caught = true; }
  check("resolve(unbound) → NotFound", caught);

  // Federation: bind_new_context creates a sub-context + binds it; through the
  // returned NamingContext ref we bind a leaf; a compound name from the root
  // traverses into the sub-context (ZeroDDS server federation).
  CosNaming::Name ctxName; ctxName.length(1);
  ctxName[0].id = CORBA::string_dup("sub"); ctxName[0].kind = CORBA::string_dup("ctx");
  CosNaming::NamingContext_var sub = nc->bind_new_context(ctxName);
  CosNaming::Name leaf; leaf.length(1);
  leaf[0].id = CORBA::string_dup("leaf"); leaf[0].kind = CORBA::string_dup("obj");
  sub->bind(leaf, nc);
  CosNaming::Name compound; compound.length(2);
  compound[0].id = CORBA::string_dup("sub"); compound[0].kind = CORBA::string_dup("ctx");
  compound[1].id = CORBA::string_dup("leaf"); compound[1].kind = CORBA::string_dup("obj");
  CORBA::Object_var fedGot = nc->resolve(compound);
  check("federation: bind_new_context + compound resolve", !CORBA::is_nil(fedGot));

  printf("%s\n", fails == 0 ? "ALL OK" : "FAILURES");
  return fails == 0 ? 0 : 1;
}
