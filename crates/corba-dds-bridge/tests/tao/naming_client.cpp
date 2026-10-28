// TAO client of `tests/tao_e2e.rs`: resolves a name through a `corbaname:`
// URL and calls `add` on what it gets back. TAO's core library parses
// `corbaname:` itself; no CosNaming stub is linked.
#include <iostream>
#include "CalcC.h"

int main(int argc, char** argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  if (argc < 2) { std::cerr << "usage: naming_client <corbaname-url>\n"; return 2; }
  try {
    CORBA::Object_var obj = orb->string_to_object(argv[1]);
    zerodds_test::Calc_var add = zerodds_test::Calc::_narrow(obj.in());
    CORBA::LongLong r = add->add(2, 40);
    std::cout << "add " << r << std::endl;
  } catch (const CORBA::SystemException& e) {
    std::cout << "add EXCEPTION " << e._name() << std::endl;
  } catch (const CORBA::UserException& e) {
    std::cout << "add USER_EXCEPTION " << e._name() << std::endl;
  }
  orb->destroy();
  return 0;
}
