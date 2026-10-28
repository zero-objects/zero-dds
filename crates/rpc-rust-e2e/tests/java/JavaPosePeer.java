// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// A Java peer of `java_peer.rs`: the generated Enhanced client of
// `shapes::Svc` with the `Pose` of one member (`EVOLUTION_POSE_OLD` of
// `build.rs`) over the DDS Java PSM binding, on a real RTPS participant.
// Arguments: `client <domain>` calls `origin()` and `length(Pose{x=5})` and
// prints `origin.x=<n> length=<n>`.
import org.omg.dds.rpc.RPCRuntime;
import org.zerodds.ddsrpc.ZeroDdsParticipant;

public final class JavaPosePeer {

    public static void main(String[] args) throws Exception {
        int domain = Integer.parseInt(args[1]);
        org.omg.dds.domain.DomainParticipant psm =
                org.omg.dds.domain.DomainParticipantFactory.getInstance(
                                org.omg.dds.core.ServiceEnvironment.createInstance(
                                        JavaPosePeer.class.getClassLoader()))
                        .createParticipant(domain);
        RPCRuntime runtime = RPCRuntime.over(new ZeroDdsParticipant(psm));
        ZdServices.shapes.SvcClientImpl client = runtime.createClient(ZdServices.shapes.SvcClientImpl::new);
        try {
            client.waitForService(org.omg.dds.core.Duration.newDuration(
                    20, java.util.concurrent.TimeUnit.SECONDS, runtime));
        } catch (org.omg.dds.rpc.TimeoutError none) {
            System.out.println("no service matched");
            System.exit(2);
        }
        ZdTypes.shapes.Pose pose = new ZdTypes.shapes.Pose();
        pose.setX(5);
        System.out.println("origin.x=" + client.origin().getX() + " length=" + client.length(pose));
        System.exit(0);
    }
}
