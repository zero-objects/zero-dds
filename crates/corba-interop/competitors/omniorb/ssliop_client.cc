// SPDX-License-Identifier: Apache-2.0
// omniORB SSLIOP cross-ORB client: calls the Echo stub over TLS via an SSLIOP
// IOR (with TAG_SSL_SEC_TRANS) published by ZeroDDS. omniORB automatically picks
// the SSL transport because the IOR requires confidentiality.
//
// Usage: ssliop_client <SSLIOP_IOR> <ca.pem> <keycert.pem>
//   ca.pem      = the self-signed ZeroDDS server cert (trusted as CA).
//   keycert.pem = omniORB's own identity (key + cert in ONE PEM — omniORB has
//                 no separate certificate_file; key_file carries both).
#include <omniORB4/CORBA.h>
#include <omniORB4/sslContext.h>
#include "interop.hh"
#include <cstdio>
#include <cstring>

// sslContext lives in the omni namespace (sslContext.h: OMNI_NAMESPACE_BEGIN(omni)).
OMNI_USING_NAMESPACE(omni)

int main(int argc, char** argv) {
  if (argc < 4) {
    fprintf(stderr, "usage: ssliop_client <IOR> <ca.pem> <keycert.pem>\n");
    return 2;
  }
  // SSL context BEFORE ORB_init: trust the ZeroDDS server cert as CA, own
  // identity (cert+key) from keycert.pem, verify the peer. key_file_password
  // MUST be set (even if empty) — otherwise omniORB does not load the key.
  sslContext::certificate_authority_file = argv[2];
  sslContext::key_file = argv[3];
  sslContext::key_file_password = "";
  sslContext::verify_mode = SSL_VERIFY_PEER;

  // Allow the SSL transport for outgoing calls (ssl preferred, tcp fallback).
  const char* a[] = {"ssliop_client", "-ORBclientTransportRule", "* ssl,tcp"};
  int ac = 3;
  CORBA::ORB_var orb = CORBA::ORB_init(ac, const_cast<char**>(a));

  CORBA::Object_var obj = orb->string_to_object(argv[1]);
  Echo_var e = Echo::_narrow(obj);
  if (CORBA::is_nil(e)) {
    fprintf(stderr, "narrow Echo failed\n");
    return 1;
  }
  CORBA::String_var r = e->ping("omni-ssliop");
  bool ok = (strcmp(r, "omni-ssliop") == 0);
  printf("  %s omni-ssliop Echo over TLS\n", ok ? "OK" : "FAIL");
  orb->destroy();
  return ok ? 0 : 1;
}
