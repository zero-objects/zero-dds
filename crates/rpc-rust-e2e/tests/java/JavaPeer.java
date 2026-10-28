// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Java peer of `java_peer.rs`: the generated Enhanced binding of
// `enh::Base2` and `enh::Derived2` over the DDS Java PSM binding, on a real
// RTPS participant. Arguments: `client <domain>` calls `Derived2.more()` of a
// service of the same interface and prints `more=<n>`; `client-base <domain>`
// calls `Base2.ident()` as a client of the base interface and prints
// `ident=<n>`; `service <domain>` serves `Derived2` until its input closes.
import org.omg.dds.rpc.RPCRuntime;
import org.omg.dds.rpc.TimeoutError;
import org.zerodds.ddsrpc.ZeroDdsParticipant;

public final class JavaPeer {

    static final class Implementation implements ZdServices.enh.Derived2 {
        @Override
        public int ident() {
            return 41;
        }

        @Override
        public int more() {
            return 42;
        }
    }

    public static void main(String[] args) throws Exception {
        int domain = Integer.parseInt(args[1]);
        org.omg.dds.domain.DomainParticipant psm =
                org.omg.dds.domain.DomainParticipantFactory.getInstance(
                                org.omg.dds.core.ServiceEnvironment.createInstance(
                                        JavaPeer.class.getClassLoader()))
                        .createParticipant(domain);
        RPCRuntime runtime = RPCRuntime.over(new ZeroDdsParticipant(psm));
        if (args[0].equals("client-base")) {
            ZdServices.enh.Base2ClientImpl client =
                    runtime.createClient(ZdServices.enh.Base2ClientImpl::new);
            try {
                client.waitForService(org.omg.dds.core.Duration.newDuration(30, java.util.concurrent.TimeUnit.SECONDS, runtime));
            } catch (TimeoutError none) {
                System.out.println("no service matched");
                System.exit(2);
            }
            System.out.println("ident=" + client.ident());
            System.exit(0);
        }
        if (args[0].equals("client")) {
            ZdServices.enh.Derived2ClientImpl client =
                    runtime.createClient(ZdServices.enh.Derived2ClientImpl::new);
            try {
                client.waitForService(org.omg.dds.core.Duration.newDuration(30, java.util.concurrent.TimeUnit.SECONDS, runtime));
            } catch (TimeoutError none) {
                System.out.println("no service matched");
                System.exit(2);
            }
            System.out.println("more=" + client.more());
            System.exit(0);
        }
        ZdServices.enh.Derived2ServiceImpl service =
                runtime.createService(p -> new ZdServices.enh.Derived2ServiceImpl(p, new Implementation()));
        System.out.println("serving");
        System.out.flush();
        System.in.read();
        service.close();
        System.exit(0);
    }
}
