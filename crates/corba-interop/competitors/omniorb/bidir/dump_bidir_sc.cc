// SPDX-License-Identifier: Apache-2.0
// Cross-ORB byte-conformance capture of the BiDirIIOPServiceContext (§15.8, tag 5)
// against omniORB 4.3.3 (the Linux test host). omniORB marshals the same struct via
// cdrEncapsulationStream → comparison with ZeroDDS
// (corba-iiop bidir::tests::bidir_sc_byte_identical_to_omniorb).
//
// Build + run:
//   printf 'struct ListenPoint { string host; unsigned short port; };\n
//           typedef sequence<ListenPoint> ListenPointList;\n
//           struct BiDirSC { ListenPointList listen_points; };\n' > Bidir.idl
//   omniidl -bcxx Bidir.idl
//   g++ -O2 -std=c++17 -I. dump_bidir_sc.cc BidirSK.cc -o dumpbin \
//       -lomniORB4 -lomnithread -lpthread
//   ./dumpbin
// Expectation (listen_points=[{"client.local",5555}], little-endian, clearMemory=1):
//   01000000010000000d000000636c69656e742e6c6f63616c0000b315
#include <omniORB4/CORBA.h>
#include "Bidir.hh"
#include <cstdio>

int main(int argc, char** argv) {
    CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
    cdrEncapsulationStream s((CORBA::ULong)0, (CORBA::Boolean)1); // clearMemory=1 → null pads
    BiDirSC sc;
    sc.listen_points.length(1);
    sc.listen_points[0].host = CORBA::string_dup("client.local");
    sc.listen_points[0].port = 5555;
    sc >>= s;
    const _CORBA_Octet* buf = (const _CORBA_Octet*) s.bufPtr();
    CORBA::ULong len = s.bufSize();
    for (CORBA::ULong i = 0; i < len; i++) printf("%02x", buf[i]);
    printf("\n");
    return 0;
}
