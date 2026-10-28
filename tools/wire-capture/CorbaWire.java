// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// JacORB over GIOP/TCP: a real remote call in both roles, so a capture
// holds the request JacORB marshals rather than what a
// `CDROutputStream` holds afterwards.
//
// `wire-golden.sh` substitutes @TYPE@ and reads the two fill blocks in
// over the two marker lines below — Java has no include directive, so
// the statements are pasted rather than included, which is what
// `wireprobe-jacorb` does with its own marker. The markers are spelled
// only where they are meant to be replaced: a first version named them
// in this paragraph too, and `sed` pasted the sample into the comment.
//
// ## What this measures that the in-memory probe cannot
//
// JacORB's GIOP is **big endian** — `org.jacorb.orb.CDROutputStream`
// has no `setLittleEndian`, and the flag byte of the GIOP message
// header is what carries the order (CORBA 3.3 §9.4). Nothing in the CDR
// stream reports it, so an in-memory probe's octets are big endian
// without saying so and only a capture can show the header that says it.

import java.io.FileOutputStream;
import java.io.PrintStream;
import java.util.Properties;

import org.omg.CORBA.ORB;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

public final class CorbaWire {

    static final class SinkImpl extends ZdwSinkPOA {
        int calls = 0;
        volatile boolean finished = false;

        @Override
        public void put(@TYPE@ v) {
            calls++;
        }

        @Override
        public void done() {
            finished = true;
        }
    }

    static @TYPE@ sampleValue() {
        @TYPE@ sample = null;
ZDW__FILL__SAMPLE
        return sample;
    }

    static @TYPE@ poisonValue() {
        @TYPE@ sample = null;
ZDW__FILL__POISON
        return sample;
    }

    public static void main(String[] argv) throws Exception {
        final String role = argv[0];
        final int count = Integer.parseInt(argv[1]);

        Properties properties = new Properties();
        properties.put("org.omg.CORBA.ORBClass", "org.jacorb.orb.ORB");
        properties.put("org.omg.CORBA.ORBSingletonClass", "org.jacorb.orb.ORBSingleton");
        final ORB orb = ORB.init(new String[0], properties);

        if ("server".equals(role)) {
            POA poa = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
            poa.the_POAManager().activate();
            SinkImpl sink = new SinkImpl();
            String ior = orb.object_to_string(poa.servant_to_reference(sink));
            try (PrintStream out = new PrintStream(new FileOutputStream("ior.txt"))) {
                out.println(ior);
            }
            Thread worker = new Thread(orb::run);
            worker.setDaemon(true);
            worker.start();
            // Until the client says it is done, not for a fixed time: a
            // sleep would either cut a run short or pad every one.
            for (int i = 0; i < 1200 && !sink.finished; i++) {
                Thread.sleep(20);
            }
            System.out.println("read " + sink.calls);
            orb.shutdown(false);
            System.exit(sink.calls == count ? 0 : 1);
        }

        ZdwSink sink = ZdwSinkHelper.narrow(orb.string_to_object(argv[2]));
        for (int i = 0; i < count; i++) {
            // The poison call first and then alternating, over one
            // connection: a short call's alignment fill then lands on
            // octets the long call wrote.
            sink.put(i % 2 == 0 ? poisonValue() : sampleValue());
            System.out.println("wrote " + i + (i % 2 == 0 ? " poison" : " sample"));
            System.out.flush();
            Thread.sleep(120);
        }
        sink.done();
        System.out.println("matched 1");
        orb.shutdown(false);
        System.exit(0);
    }
}
