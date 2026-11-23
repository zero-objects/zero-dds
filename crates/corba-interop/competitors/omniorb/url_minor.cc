// SPDX-License-Identifier: Apache-2.0
// omniORB side of the Object URL cell (CORBA 3.3 Part 2 §7.6.10): converts
// each argument with ORB::string_to_object and prints one line per URL,
//   <url> TAB OK
//   <url> TAB <exception name> TAB <minor code in hex>
// then "ALL OK". The test compares the lines with what ZeroDDS raises for
// the same URLs.
//
// Usage: url_minor <url>...
#include <omniORB4/CORBA.h>
#include <cstdio>

int main(int argc, char** argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  for (int i = 1; i < argc; i++) {
    try {
      CORBA::Object_var obj = orb->string_to_object(argv[i]);
      printf("%s\tOK\n", argv[i]);
    } catch (CORBA::SystemException& e) {
      printf("%s\t%s\t%lx\n", argv[i], e._name(), (unsigned long)e.minor());
    }
  }
  orb->destroy();
  printf("ALL OK\n");
  return 0;
}
