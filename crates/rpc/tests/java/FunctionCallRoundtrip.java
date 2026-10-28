// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The function-call style of DDS-RPC 1.0 §7.2.2.3, §7.9.1, §7.11.1.5 and
// §7.11.2.3, run: the generated `<I>ClientImpl` calls the generated
// `<I>ServiceImpl` over the DDS Java PSM of crates/java-omgdds, with the
// generated codec in between, under both encodings.

import java.io.ByteArrayOutputStream;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.TimeUnit;

import org.omg.dds.rpc.DataReader;
import org.omg.dds.rpc.DataWriter;
import org.omg.dds.rpc.DomainParticipant;
import org.omg.dds.rpc.Future;
import org.omg.dds.rpc.RelatedIdentity;
import org.omg.dds.rpc.RemoteUnknownExceptionError;
import org.omg.dds.rpc.RemoteUnsupportedError;
import org.omg.dds.rpc.ReplierParams;
import org.zerodds.ddsrpc.RequesterImpl;
import org.omg.dds.rpc.RequesterParams;
import org.omg.dds.rpc.Sample;
import org.omg.dds.rpc.SampleIdentity;
import org.omg.dds.rpc.Server;
import org.omg.dds.rpc.UserExceptionRaised;
import org.omg.dds.rpc.WireEncoding;
import org.omg.dds.rpc.RpcQos;
import org.zerodds.ddsrpc.ZeroDdsParticipant;

public final class FunctionCallRoundtrip {

    private static int failures = 0;

    static void check(boolean held, String what) {
        if (!held) {
            System.err.println("FAIL " + what);
            ++failures;
        }
    }

    /** The service implementation: plain Java, no DDS in sight. */
    static final class Impl implements ZdServices.robot.Calculator {
        int total = 5;
        String label = "calc";
        int resets = 0;
        boolean overflow = false;
        boolean boom = false;

        @Override
        public int ping() {
            return 5;
        }

        @Override
        public int total() {
            return total;
        }

        @Override
        public String label() {
            return label;
        }

        @Override
        public void label(String label) {
            this.label = label;
        }

        @Override
        public int add(int a, int b) {
            if (overflow) {
                ZdTypes.robot.Overflow raised = new ZdTypes.robot.Overflow();
                raised.code = 7;
                throw new UserExceptionRaised(raised);
            }
            return a + b;
        }

        @Override
        public ZdServices.robot.Calculator_divmod_Out divmod(int a, int b) {
            ZdServices.robot.Calculator_divmod_Out out = new ZdServices.robot.Calculator_divmod_Out();
            out.return_ = a / b;
            out.remainder = a % b;
            return out;
        }

        @Override
        public ZdTypes.robot.Pose locate(String room) {
            ZdTypes.robot.Pose pose = new ZdTypes.robot.Pose();
            pose.x = room.length();
            pose.y = 2;
            return pose;
        }

        @Override
        public void reset() {
            ++resets;
        }

        @Override
        public double scale(double value) {
            if (boom) {
                throw new IllegalStateException("boom");
            }
            return value * 2.0;
        }
    }

    /** A participant that remembers the last sample written to each topic. */
    static final class Recording implements DomainParticipant {
        private final DomainParticipant inner;
        final Map<String, byte[]> last = new HashMap<>();

        Recording(DomainParticipant inner) {
            this.inner = inner;
        }

        @Override
        public DataWriter createWriter(String topic, String typeName, RpcQos qos, String instance) {
            DataWriter writer = inner.createWriter(topic, typeName, qos, instance);
            return new DataWriter() {
                @Override
                public SampleIdentity write(byte[] sample) {
                    last.put(topic, sample);
                    return writer.write(sample);
                }

                @Override
                public SampleIdentity nextIdentity() {
                    return writer.nextIdentity();
                }

                @Override
                public int matchedCount() {
                    return writer.matchedCount();
                }
            };
        }

        @Override
        public DataReader createReader(String topic, String typeName, RpcQos qos, String instance) {
            return inner.createReader(topic, typeName, qos, instance);
        }

        @Override
        public void deleteWriter(DataWriter writer) {
            // The wrapper is not what the inner participant created; the
            // test does not need the entity gone.
        }

        @Override
        public void deleteReader(DataReader reader) {
            inner.deleteReader(reader);
        }
    }

    static void run(WireEncoding encoding, int domain) throws Exception {
        String tag = " (" + encoding + ")";
        Recording participant =
                new Recording(new ZeroDdsParticipant(org.omg.dds.domain.DomainParticipantFactory.getInstance(
                        org.omg.dds.core.ServiceEnvironment.createInstance(
                                FunctionCallRoundtrip.class.getClassLoader())).createParticipant(domain)));
        Impl impl = new Impl();

        // The PSM writer cannot send PID_RELATED_SAMPLE_IDENTITY (§7.8.2),
        // so the Basic header correlates (a named setting).
        ReplierParams serving = ZdServices.robot.CalculatorSupport.replierParams(participant, "")
                        .relatedIdentity(RelatedIdentity.IF_SUPPORTED)
                        .wireEncoding(encoding);
        ZdServices.robot.CalculatorServiceImpl service =
                new ZdServices.robot.CalculatorServiceImpl(serving, impl);
        RequesterParams asking = ZdServices.robot.CalculatorSupport.requesterParams(participant)
                        .wireEncoding(encoding);
        ZdServices.robot.CalculatorClientImpl client = new ZdServices.robot.CalculatorClientImpl(asking);
        check(client.isServiceMatched(), "the client matched the service" + tag);

        // §7.11.2: the client's and the service's entities by the class of their topic type.
        // Over a binding that wraps the PSM's they are not the PSM's.
        check(client.getRequestDataWriter(ZdServices.robot.Calculator_Request.class) == null
                        && service.getRequestDataReader(ZdServices.robot.Calculator_Request.class) == null,
                "a binding that wraps the PSM's hands out no PSM entities" + tag);
        boolean notOurs = false;
        try {
            client.getRequestDataWriter(String.class);
        } catch (IllegalArgumentException expected) {
            notOurs = true;
        }
        check(notOurs, "a class that is no request type of the client is refused" + tag);
        ZeroDdsParticipant direct = new ZeroDdsParticipant(
                org.omg.dds.domain.DomainParticipantFactory.getInstance(
                        org.omg.dds.core.ServiceEnvironment.createInstance(
                                FunctionCallRoundtrip.class.getClassLoader()))
                        .createParticipant(domain + 100));
        ZdServices.robot.CalculatorClientImpl typedClient = new ZdServices.robot.CalculatorClientImpl(
                ZdServices.robot.CalculatorSupport.requesterParams(direct).wireEncoding(encoding));
        ZdServices.robot.CalculatorServiceImpl typedService = new ZdServices.robot.CalculatorServiceImpl(
                ZdServices.robot.CalculatorSupport.replierParams(direct, "").wireEncoding(encoding), impl);
        org.omg.dds.pub.DataWriter<ZdServices.robot.Base_Request> baseRequests =
                typedClient.getRequestDataWriter(ZdServices.robot.Base_Request.class);
        org.omg.dds.pub.DataWriter<ZdServices.robot.Calculator_Request> ownRequests =
                typedClient.getRequestDataWriter(ZdServices.robot.Calculator_Request.class);
        check(baseRequests != null && ownRequests != null
                        && baseRequests.getTopic().getTypeSupport().getType()
                                == ZdServices.robot.Base_Request.class
                        && ownRequests.getTopic().getTypeSupport().getType()
                                == ZdServices.robot.Calculator_Request.class,
                "the client hands out the request writer of its interface and of its base, each typed" + tag);
        org.omg.dds.sub.DataReader<ZdServices.robot.Base_Reply> baseReplies =
                typedClient.getReplyDataReader(ZdServices.robot.Base_Reply.class);
        org.omg.dds.sub.DataReader<ZdServices.robot.Calculator_Request> served =
                typedService.getRequestDataReader(ZdServices.robot.Calculator_Request.class);
        org.omg.dds.pub.DataWriter<ZdServices.robot.Base_Reply> baseAnswers =
                typedService.getReplyDataWriter(ZdServices.robot.Base_Reply.class);
        check(baseReplies != null && served != null && baseAnswers != null
                        && baseAnswers.getTopic().getTypeSupport().getType()
                                == ZdServices.robot.Base_Reply.class,
                "the service hands out the request reader and the reply writer by class" + tag);
        typedClient.close();
        typedService.close();

        // A call, with a return value.
        check(client.add(2, 40) == 42, "add" + tag);
        check(service.implementation() == impl, "the service knows its implementation" + tag);

        // `out`: the Out structure carries the return value and the output.
        ZdServices.robot.Calculator_divmod_Out out = client.divmod(17, 5);
        check(out.return_ == 3 && out.remainder == 2, "divmod" + tag);

        // A structure by value, a string in.
        ZdTypes.robot.Pose pose = client.locate("kitchen");
        check(pose.x == 7 && pose.y == 2, "locate" + tag);

        // void, and what the implementation saw.
        client.reset();
        check(impl.resets == 1, "reset reached the implementation" + tag);

        // Attributes: the readonly one, and the writable one both ways.
        check(client.total() == 5, "readonly attribute" + tag);
        client.label("robot");
        check("robot".equals(client.label()) && "robot".equals(impl.label), "attribute" + tag);

        // An operation of the base interface, over the base's own topics.
        check(client.ping() == 5, "ping through the base client" + tag);

        // An eight-octet member aligns by the encoding, from the start of
        // the sample: header 29 -> 32, discriminator 32..36, then the
        // double at 40 (classic CDR) or at 36 (XCDR2).
        check(client.scale(1.5) == 3.0, "double round trip" + tag);
        byte[] request = participant.last.get(ZdServices.robot.CalculatorSupport.REQUEST_TOPIC_NAME);
        int expected = encoding == WireEncoding.XCDR1 ? 48 : 44;
        check(request != null && request.length == expected,
                "the request is " + expected + " octets under " + encoding + ", not "
                        + (request == null ? -1 : request.length));

        // §7.4.1, §7.4.2.3: a service name set at run time composes
        // `<interface_name>_<service_name>_Request`, with the interface
        // name the generated factory gave.
        RequesterParams named =
                ZdServices.robot.CalculatorSupport.requesterParams(participant)
                        .serviceName("Fast");
        check("robot_Calculator_Fast_Request".equals(
                        org.omg.dds.rpc.TopicNames.of(named).request()),
                "a run-time service name keeps the interface name" + tag);
        check("robot_Calculator_Fast_Reply".equals(
                        org.omg.dds.rpc.TopicNames.of(named).reply()),
                "and names the reply topic" + tag);

        // A declared exception reaches the caller as UserExceptionRaised.
        impl.overflow = true;
        boolean raised = false;
        try {
            client.add(1, 1);
        } catch (UserExceptionRaised expectedFailure) {
            raised = expectedFailure.value() instanceof ZdTypes.robot.Overflow
                    && ((ZdTypes.robot.Overflow) expectedFailure.value()).code == 7;
        }
        check(raised, "a declared exception reaches the caller" + tag);
        impl.overflow = false;

        // Any other failure of the implementation is REMOTE_EX_UNKNOWN_EXCEPTION.
        impl.boom = true;
        boolean unknown = false;
        try {
            client.scale(1.0);
        } catch (RemoteUnknownExceptionError expectedFailure) {
            unknown = true;
        }
        check(unknown, "an undeclared failure is REMOTE_EX_UNKNOWN_EXCEPTION" + tag);
        impl.boom = false;

        // An operation the service does not have is REMOTE_EX_UNSUPPORTED (§7.7.1.1).
        RequesterImpl<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply>
                raw = new RequesterImpl<>(
                        ZdServices.robot.CalculatorSupport.requesterParams(participant)
                                .wireEncoding(encoding));
        ZdServices.robot.Calculator_Request unknownCall = new ZdServices.robot.Calculator_Request();
        unknownCall.data = new ZdServices.robot.Calculator_Call();
        unknownCall.data._d = 424242;
        unknownCall.data.unknownOp = new org.omg.dds.rpc.UnknownOperation();
        SampleIdentity id = raw.sendRequest(unknownCall);
        boolean unsupported = false;
        for (int wait = 0; wait < 500 && !unsupported; ++wait) {
            try {
                if (raw.receiveReply(id) != null) {
                    break;
                }
            } catch (RemoteUnsupportedError expectedFailure) {
                unsupported = true;
            }
            Thread.sleep(2);
        }
        check(unsupported, "an unknown operation is REMOTE_EX_UNSUPPORTED" + tag);
        raw.close();

        // Asynchronous operations, and a paused service.
        Future<Integer> sum = client.add_async(3, 4);
        check(sum.get(5, TimeUnit.SECONDS) == 7, "add_async" + tag);
        Server server = Server.create();
        server.add(service);
        service.pause();
        check(service.isPaused(), "paused" + tag);
        Future<Integer> held = client.add_async(1, 1);
        Thread.sleep(50);
        check(!held.isDone(), "a paused service answers nothing" + tag);
        service.resume();
        check(held.get(5, TimeUnit.SECONDS) == 2, "and answers what waited, on resume" + tag);

        // §7.9.3: close() deactivates both.
        client.close();
        check(client.isNull(), "a closed client is null" + tag);
        server.close();
        check(service.isNull(), "a closed service is null" + tag);
    }

    public static void main(String[] args) throws Exception {
        run(WireEncoding.XCDR1, 80);
        run(WireEncoding.XCDR2, 81);
        if (failures != 0) {
            System.err.println(failures + " checks failed");
            System.exit(1);
        }
        System.out.println("function-call round trip: every check held");
    }
}
