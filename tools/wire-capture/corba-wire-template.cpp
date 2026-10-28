/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * omniORB and TAO over GIOP/TCP: a real remote call, so a capture holds
 * the request the ORB marshals rather than what a probe's own
 * `cdrMemoryStream` holds afterwards.
 *
 * One source for both ORBs, because both implement the OMG IDL-to-C++
 * mapping and `wireprobe-fill --vendor omniorb` writes statements
 * against it — the same statements the in-memory probes use, which is
 * what makes the two levels comparable.
 *
 * ## The alternation, and why it decides something here
 *
 * `omniorb-template.cpp` and `tao-template.cpp` fill their buffer with
 * `0xAA` before serialising, because the 2026-08-13 capture found that
 * neither ORB writes the alignment fill: what goes out is whatever the
 * connection's send buffer held from the previous call. This runner
 * makes that visible by alternating a long **poison** call and the
 * corpus call over **one** connection: the short call's fill spans then
 * land on octets the long call wrote.
 *
 * @TYPE@ is the C++ name of the corpus type; `wire-golden.sh`
 * substitutes it.
 */
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iostream>
#include <string>
#include <unistd.h>

#include "@HEADER@"

class ZdwSinkImpl : public POA_ZdwSink {
public:
    int calls = 0;
    void put(const @TYPE@ &value) override { (void)value; calls++; }
    void done() override { finished = true; }
    bool finished = false;
};

static int zd_fill_sample(@TYPE@ &sample) {
#include "fill-sample.inc"
    return 0;
}

static int zd_fill_poison(@TYPE@ &sample) {
#include "fill-poison.inc"
    return 0;
}

int main(int argc, char **argv) {
    if (argc < 3) {
        std::fprintf(stderr, "usage: probe <server|client> <count> [ior] [orb args]\n");
        return 2;
    }
    const std::string role = argv[1];
    const int count = std::atoi(argv[2]);

    /* The ORB takes its own options off the tail of the command line:
       `-ORBendPoint` for omniORB, `-ORBListenEndpoints` for TAO. */
    int orb_argc = argc - 3;
    char **orb_argv = argv + 3;
    static char program[] = "probe";
    char *shifted[64];
    shifted[0] = program;
    for (int i = 0; i < orb_argc && i < 62; i++) shifted[i + 1] = orb_argv[i];
    orb_argc += 1;

    CORBA::ORB_var orb = CORBA::ORB_init(orb_argc, shifted);

    if (role == "server") {
        CORBA::Object_var poa_object = orb->resolve_initial_references("RootPOA");
        PortableServer::POA_var poa = PortableServer::POA::_narrow(poa_object);
        PortableServer::POAManager_var manager = poa->the_POAManager();
        manager->activate();

        ZdwSinkImpl *sink = new ZdwSinkImpl();
        PortableServer::ObjectId_var id = poa->activate_object(sink);
        CORBA::Object_var reference = poa->id_to_reference(id.in());
        CORBA::String_var ior = orb->object_to_string(reference.in());
        std::ofstream("ior.txt") << ior.in() << std::endl;

        /* Neither ORB has a portable "run until n calls" and a fixed
           sleep would either cut a run short or pad every one of them.
           `perform_work` in a loop stops on the client's `done`. */
        for (int i = 0; i < 1200 && !sink->finished; i++) {
            if (orb->work_pending()) orb->perform_work();
            else usleep(20000);
        }
        std::printf("read %d\n", sink->calls);
        return sink->calls == count ? 0 : 1;
    }

    if (argc < 4) { std::fprintf(stderr, "the client needs an IOR\n"); return 2; }
    CORBA::Object_var object = orb->string_to_object(argv[3]);
    ZdwSink_var sink = ZdwSink::_narrow(object);
    if (CORBA::is_nil(sink)) { std::fprintf(stderr, "the IOR would not narrow\n"); return 1; }

    for (int i = 0; i < count; i++) {
        @TYPE@ value;
        const int filled = (i % 2 == 0) ? zd_fill_poison(value) : zd_fill_sample(value);
        if (filled != 0) return filled;
        sink->put(value);
        std::printf("wrote %d %s\n", i, i % 2 == 0 ? "poison" : "sample");
        std::fflush(stdout);
        usleep(120000);
    }
    sink->done();
    std::printf("matched 1\n");
    orb->destroy();
    return 0;
}
