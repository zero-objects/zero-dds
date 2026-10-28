// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// A Java peer of `java_peer.rs`: the generated Enhanced client of the old
// version of `evo::Svc` (`EVOLUTION_OLD` of `build.rs`) over the DDS Java PSM
// binding, on a real RTPS participant. Arguments: `client <domain> <seconds>`
// waits that long for the service and prints `no service matched`, or calls
// `add(1, 2)` and prints `add=<n>`.
import org.omg.dds.rpc.RPCRuntime;
import org.zerodds.ddsrpc.ZeroDdsParticipant;

public final class JavaEvolutionPeer {

    public static void main(String[] args) throws Exception {
        int domain = Integer.parseInt(args[1]);
        long seconds = Long.parseLong(args[2]);
        org.omg.dds.domain.DomainParticipant psm =
                org.omg.dds.domain.DomainParticipantFactory.getInstance(
                                org.omg.dds.core.ServiceEnvironment.createInstance(
                                        JavaEvolutionPeer.class.getClassLoader()))
                        .createParticipant(domain);
        RPCRuntime runtime = RPCRuntime.over(new ZeroDdsParticipant(psm));
        ZdServices.evo.SvcClientImpl client = runtime.createClient(ZdServices.evo.SvcClientImpl::new);
        try {
            client.waitForService(org.omg.dds.core.Duration.newDuration(
                    seconds, java.util.concurrent.TimeUnit.SECONDS, runtime));
        } catch (org.omg.dds.rpc.TimeoutError none) {
            System.out.println("no service matched");
            System.exit(2);
        }
        System.out.println("add=" + client.add(1, 2));
        System.exit(0);
    }
}
