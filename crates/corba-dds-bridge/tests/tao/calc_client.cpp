// TAO client of `tests/tao_e2e.rs`: reads the bridge's IOR file
// (`<repo_id> <operation> IOR:...` per line), calls `add`, `never`, the add
// object's `never` and `echo`, prints one line per outcome in the form the
// omniORB client prints.
#include <fstream>
#include <iostream>
#include <map>
#include <string>
#include "CalcC.h"

int main(int argc, char** argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  if (argc < 2) { std::cerr << "usage: calc_client <ior_file>\n"; return 2; }
  std::map<std::string, std::string> iors;
  std::ifstream in(argv[1]);
  std::string repo, op, ior;
  while (in >> repo >> op >> ior) { iors[op] = ior; }
  try {
    CORBA::Object_var obj = orb->string_to_object(iors["add"].c_str());
    zerodds_test::Calc_var add = zerodds_test::Calc::_narrow(obj.in());
    CORBA::LongLong r = add->add(2, 40);
    std::cout << "add " << r << std::endl;
  } catch (const CORBA::SystemException& e) {
    std::cout << "add EXCEPTION " << e._name() << std::endl;
  }
  try {
    CORBA::Object_var obj = orb->string_to_object(iors["never"].c_str());
    zerodds_test::Calc_var never = zerodds_test::Calc::_narrow(obj.in());
    CORBA::LongLong r = never->never(1);
    std::cout << "never " << r << std::endl;
  } catch (const CORBA::SystemException& e) {
    std::cout << "never EXCEPTION " << e._name() << std::endl;
  }
  try {
    // The add object, asked for an operation it does not map.
    CORBA::Object_var obj = orb->string_to_object(iors["add"].c_str());
    zerodds_test::Calc_var add = zerodds_test::Calc::_narrow(obj.in());
    CORBA::LongLong r = add->never(1);
    std::cout << "wrong " << r << std::endl;
  } catch (const CORBA::SystemException& e) {
    std::cout << "wrong EXCEPTION " << e._name() << std::endl;
  }
  try {
    // 200 000 bytes: above the bridge's fragment_size in both directions.
    CORBA::Object_var obj = orb->string_to_object(iors["echo"].c_str());
    zerodds_test::Calc_var echo = zerodds_test::Calc::_narrow(obj.in());
    zerodds_test::Bytes data;
    data.length(200000);
    for (CORBA::ULong i = 0; i < data.length(); ++i) data[i] = static_cast<CORBA::Octet>(i % 251);
    zerodds_test::Bytes_var back = echo->echo(data);
    bool same = back->length() == data.length();
    for (CORBA::ULong i = 0; same && i < data.length(); ++i) same = back[i] == data[i];
    std::cout << "echo " << back->length() << (same ? " same" : " differs") << std::endl;
  } catch (const CORBA::SystemException& e) {
    std::cout << "echo EXCEPTION " << e._name() << std::endl;
  }
  orb->destroy();
  return 0;
}
