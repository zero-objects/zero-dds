// SPDX-License-Identifier: Apache-2.0
// omniORB SSLIOP cross-ORB server: activates an Echo object on a pure SSL
// endpoint and prints its IOR (with TAG_SSL_SEC_TRANS) to stdout.
// The ZeroDDS ssliop_client parses the SSL component and calls Echo over TLS.
//
// Usage: ssliop_server <ca.pem> <keycert.pem>
//   keycert.pem = key + cert in ONE PEM (omniORB key_file carries both).
#include <omniORB4/CORBA.h>
#include <omniORB4/sslContext.h>
#include "interop.hh"
#include <cstdio>

// sslContext lives in the omni namespace (sslContext.h: OMNI_NAMESPACE_BEGIN(omni)).
OMNI_USING_NAMESPACE(omni)

class EchoImpl : public POA_Echo {
public:
  char* ping(const char* msg) override { return CORBA::string_dup(msg); }
};

int main(int argc, char** argv) {
  if (argc < 3) {
    fprintf(stderr, "usage: ssliop_server <ca.pem> <keycert.pem>\n");
    return 2;
  }
  // SSL context BEFORE ORB_init: own identity (cert+key in keycert.pem, cert
  // FIRST — omniORB reads the first PEM object as the certificate), no client
  // auth. key_file_password MUST be set (even if empty): otherwise omniORB's
  // default password callback returns nothing → key load fails silently →
  // no server cert → TLS HandshakeFailure.
  sslContext::certificate_authority_file = argv[1];
  sslContext::key_file = argv[2];
  sslContext::key_file_password = "";
  sslContext::verify_mode = SSL_VERIFY_NONE;

  // Pure SSL endpoint (no cleartext): the IOR carries only the SSL profile.
  const char* a[] = {"ssliop_server", "-ORBendPoint", "giop:ssl:127.0.0.1:0"};
  int ac = 3;
  CORBA::ORB_var orb = CORBA::ORB_init(ac, const_cast<char**>(a));

  CORBA::Object_var pobj = orb->resolve_initial_references("RootPOA");
  PortableServer::POA_var poa = PortableServer::POA::_narrow(pobj);
  poa->the_POAManager()->activate();

  EchoImpl* servant = new EchoImpl();
  Echo_var ref = servant->_this();
  CORBA::String_var ior = orb->object_to_string(ref);
  printf("SSLIOP_IOR=%s\n", static_cast<char*>(ior));
  fflush(stdout);

  orb->run();
  return 0;
}
