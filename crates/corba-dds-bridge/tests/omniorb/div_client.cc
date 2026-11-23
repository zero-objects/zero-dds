// omniORB client of `tests/omniorb_e2e.rs` for the typed mapping
// (zerodds-corba-bridge-1.0 §5.2): reads the bridge's IOR file, calls
// `divide` three times and prints one line per outcome — a result with its
// out argument, the user exception with members, the one without.
#include <fstream>
#include <iostream>
#include <string>
#include "Div.hh"

static void divide(zerodds_test::Divider_ptr d, CORBA::LongLong n, CORBA::LongLong m) {
  zerodds_test::Fraction f;
  f.numerator = n;
  f.denominator = m;
  try {
    CORBA::LongLong remainder = 0;
    CORBA::LongLong q = d->divide(f, remainder);
    std::cout << "divide " << q << " " << remainder << std::endl;
  } catch (const zerodds_test::Divider::DivideByZero& e) {
    std::cout << "DivideByZero " << e.numerator << " " << e.why.in() << std::endl;
  } catch (const zerodds_test::Divider::Unsupported&) {
    std::cout << "Unsupported" << std::endl;
  } catch (const CORBA::SystemException& e) {
    std::cout << "EXCEPTION " << e._name() << std::endl;
  }
}

int main(int argc, char** argv) {
  if (argc < 2) { std::cerr << "usage: div_client <ior_file>\n"; return 2; }
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  std::ifstream in(argv[1]);
  std::string repo, op, ior;
  in >> repo >> op >> ior;
  zerodds_test::Divider_var d = zerodds_test::Divider::_narrow(orb->string_to_object(ior.c_str()));
  divide(d, 7, 2);
  divide(d, 7, 0);
  divide(d, -7, 2);
  orb->destroy();
  return 0;
}
