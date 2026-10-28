// omniORB SSLIOP client of `tests/omniorb_e2e.rs`: reads the bridge's IOR
// file, calls `never` over SSLIOP with the given CA and client key file (PEM,
// certificate and key), prints the outcome. With no key file it presents no
// certificate. The IOR's TAG_SSL_SEC_TRANS gives the SSL port.
#include <fstream>
#include <iostream>
#include <map>
#include <string>
#include "Calc.hh"
// After CORBA.h (through Calc.hh): sslContext.h builds on it.
#include <omniORB4/sslContext.h>

int main(int argc, char** argv) {
  if (argc < 3) { std::cerr << "usage: ssl_client <ior_file> <ca.pem> [<client.pem>]\n"; return 2; }
  omni::sslContext::certificate_authority_file = argv[2];
  if (argc > 3) { omni::sslContext::key_file = argv[3]; omni::sslContext::key_file_password = ""; }
  const char* options[][2] = {{"clientTransportRule", "* ssl"}, {0, 0}};
  int orb_argc = 1;
  CORBA::ORB_var orb = CORBA::ORB_init(orb_argc, argv, "omniORB4", options);
  std::map<std::string, std::string> iors;
  std::ifstream in(argv[1]);
  std::string repo, op, ior;
  while (in >> repo >> op >> ior) { iors[op] = ior; }
  try {
    zerodds_test::Calc_var never = zerodds_test::Calc::_narrow(orb->string_to_object(iors["never"].c_str()));
    CORBA::LongLong r = never->never(1);
    std::cout << "never " << r << std::endl;
  } catch (const CORBA::SystemException& e) {
    std::cout << "never EXCEPTION " << e._name() << std::endl;
  }
  orb->destroy();
  return 0;
}
