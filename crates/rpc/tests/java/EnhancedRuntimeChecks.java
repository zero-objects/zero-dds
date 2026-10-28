// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The generated Java function-call service and client of the Enhanced
// Service Mapping (DDS-RPC 1.0 §7.5.1.2) at run time, over a loopback
// binding of the three abstract entities:
//
//   - §7.7.2: a request that names no operation of the interface is
//     answered REMOTE_EX_UNKNOWN_OPERATION;
//   - §7.5.1.2.1.1: a @Choice holds exactly one member, so an encoder
//     refuses none and several, and a decoder refuses several;
//   - §7.5.2: an implementation that fails with something its operation
//     does not declare is answered REMOTE_EX_UNKNOWN_EXCEPTION, and a reply
//     whose Result holds an exception the client does not know is raised
//     as REMOTE_EX_UNKNOWN_EXCEPTION;
//   - §7.4.1: the generated Support names the interface, so a run-time
//     service name composes <interface>_<service>_Request.
//
// Compiled with the Enhanced `ZdServices` of the interface in
// crates/rpc/tests/java_enhanced_codec.rs. The member id of `add` arrives
// as the first argument.

import java.io.ByteArrayOutputStream;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutionException;

import org.omg.dds.rpc.DataReader;
import org.omg.dds.rpc.DataWriter;
import org.omg.dds.rpc.DomainParticipant;
import org.omg.dds.rpc.EnhancedReplySupport;
import org.omg.dds.rpc.Future;
import org.omg.dds.rpc.HeaderCodec;
import org.omg.dds.rpc.RPCRuntime;
import org.omg.dds.rpc.RemoteExceptionCode;
import org.omg.dds.rpc.RemoteInvalidArgumentError;
import org.omg.dds.rpc.RemoteUnknownExceptionError;
import org.omg.dds.rpc.RemoteUnknownOperationError;
import org.omg.dds.rpc.RequesterParams;
import org.omg.dds.rpc.RpcQos;
import org.omg.dds.rpc.SampleIdentity;
import org.omg.dds.rpc.Replier;
import org.omg.dds.rpc.Requester;
import org.omg.dds.rpc.Sample;
import org.omg.dds.rpc.TakenSample;
import org.omg.dds.rpc.TopicNames;
import org.omg.dds.rpc.UserExceptionRaised;
import org.omg.dds.rpc.WireEncoding;

public final class EnhancedRuntimeChecks {

    private static int failures = 0;

    static void check(boolean held, String what) {
        if (!held) {
            System.err.println("FAIL " + what);
            ++failures;
        }
    }

    // -- a loopback binding of the three entities ------------------------

    static final class Bus {
        final Map<String, List<TakenSample>> queue = new HashMap<>();
        final Map<String, Integer> readers = new HashMap<>();
        final Map<String, Integer> writers = new HashMap<>();
        final Map<String, List<Runnable>> listeners = new HashMap<>();

        /** DDS 1.4 on_data_available: the readers of the topic are told a sample is there. */
        void arrived(String topic) {
            for (Runnable listener : new ArrayList<>(listeners.getOrDefault(topic, new ArrayList<>()))) {
                listener.run();
            }
        }

        List<TakenSample> queueOf(String topic) {
            return queue.computeIfAbsent(topic, key -> new ArrayList<>());
        }
    }

    static final class LoopWriter implements DataWriter {
        private final Bus bus;
        private final String topic;
        private final byte[] guid = new byte[16];
        private long sequence = 0;

        LoopWriter(Bus bus, String topic, int tag) {
            this.bus = bus;
            this.topic = topic;
            for (int at = 0; at < 16; ++at) {
                guid[at] = (byte) (tag + at);
            }
            bus.writers.merge(topic, 1, Integer::sum);
        }

        private SampleIdentity deliver(byte[] sample, SampleIdentity related) {
            SampleIdentity identity = nextIdentity();
            ++sequence;
            bus.queueOf(topic).add(new TakenSample(sample, identity, related));
            bus.arrived(topic);
            return identity;
        }

        @Override
        public SampleIdentity write(byte[] sample) {
            return deliver(sample, SampleIdentity.UNKNOWN);
        }

        @Override
        public SampleIdentity write(byte[] sample, SampleIdentity related) {
            return deliver(sample, related);
        }

        @Override
        public boolean carriesRelatedIdentity() {
            return true;
        }

        @Override
        public SampleIdentity nextIdentity() {
            return new SampleIdentity(guid, sequence + 1);
        }

        @Override
        public int matchedCount() {
            return bus.readers.getOrDefault(topic, 0);
        }
    }

    static final class LoopReader implements DataReader {
        private final Bus bus;
        private final String topic;
        private Runnable listener;

        LoopReader(Bus bus, String topic) {
            this.bus = bus;
            this.topic = topic;
            bus.readers.merge(topic, 1, Integer::sum);
        }

        @Override
        public List<TakenSample> take() {
            List<TakenSample> held = bus.queueOf(topic);
            List<TakenSample> out = new ArrayList<>(held);
            held.clear();
            return out;
        }

        @Override
        public int matchedCount() {
            return bus.writers.getOrDefault(topic, 0);
        }

        @Override
        public void setDataAvailableListener(Runnable listener) {
            List<Runnable> registered = bus.listeners.computeIfAbsent(topic, key -> new ArrayList<>());
            if (this.listener != null) {
                registered.remove(this.listener);
            }
            this.listener = listener;
            if (listener != null) {
                registered.add(listener);
            }
        }
    }

    static final class LoopParticipant implements DomainParticipant {
        final Bus bus = new Bus();
        private int tag = 0x20;
        /** The topic each created entity is on, in creation order. */
        final List<String> topics = new ArrayList<>();

        @Override
        public DataWriter createWriter(String topic, String typeName, RpcQos qos, String instance) {
            topics.add("writer " + topic);
            return new LoopWriter(bus, topic, ++tag);
        }

        @Override
        public DataReader createReader(String topic, String typeName, RpcQos qos, String instance) {
            topics.add("reader " + topic);
            return new LoopReader(bus, topic);
        }

        @Override
        public void deleteWriter(DataWriter writer) {}

        @Override
        public void deleteReader(DataReader reader) {}
    }

    // -- the implementation of the interface ------------------------------

    static final class Implementation implements ZdServices.robot.Calc {
        @Override
        public int ident() {
            return 9;
        }

        @Override
        public int add(int a, int b) {
            if (a < 0) {
                ZdTypes.robot.TooFast refusal = new ZdTypes.robot.TooFast();
                refusal.limit = 7;
                refusal.why = "negative";
                throw new UserExceptionRaised(refusal);
            }
            return a + b;
        }

        @Override
        public void ping() {
            throw new IllegalStateException("ping is not implemented here");
        }

        @Override
        public void note(String text) {}
    }

    // -- octets ------------------------------------------------------------

    static void put32(ByteArrayOutputStream out, long value) {
        for (int shift = 0; shift < 32; shift += 8) {
            out.write((int) ((value >>> shift) & 0xFF));
        }
    }

    /** A member id no operation, result or exception of the interface has. */
    static final long FOREIGN = 0x00ABCDEFL;

    static ZdServices.robot.Calc_Request addRequest(int a, int b) {
        ZdServices.robot.Calc_Request request = new ZdServices.robot.Calc_Request();
        request.add = new ZdServices.robot.Calc_add_In();
        request.add.a = a;
        request.add.b = b;
        return request;
    }

    static ZdServices.robot.Calc_Request pingRequest() {
        ZdServices.robot.Calc_Request request = new ZdServices.robot.Calc_Request();
        request.ping = new ZdServices.robot.Calc_ping_In();
        request.ping.dummy = new org.omg.dds.rpc.UnusedMember();
        return request;
    }

    /** A request that holds one member the interface does not have. */
    static byte[] foreignRequest() {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        put32(out, 12);
        put32(out, (4L << 28) | FOREIGN);
        put32(out, 4);
        put32(out, 0);
        return out.toByteArray();
    }

    /** A request that holds no member at all: an empty DHEADER. */
    static byte[] emptyRequest() {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        put32(out, 0);
        return out.toByteArray();
    }

    /** Two members, spliced out of two well-formed requests. */
    static byte[] twoMemberRequest() {
        byte[] one = ZdServices.robot.CalcCodec.REQUEST.serialize(addRequest(1, 2));
        byte[] two = ZdServices.robot.CalcCodec.REQUEST.serialize(pingRequest());
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        put32(out, (one.length - 4) + (two.length - 4));
        out.write(one, 4, one.length - 4);
        out.write(two, 4, two.length - 4);
        return out.toByteArray();
    }

    /** A reply to {@code add} whose Result holds a member the client does not know. */
    static byte[] foreignExceptionReply(long add) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        put32(out, 24);
        put32(out, (4L << 28) | add);
        put32(out, 16);
        put32(out, 12);
        put32(out, (4L << 28) | FOREIGN);
        put32(out, 4);
        put32(out, 0);
        return out.toByteArray();
    }

    // -- the service side --------------------------------------------------

    static ZdServices.robot.Calc_Reply answer(
            ZdServices.robot.CalcServiceImpl service,
            DataWriter forge,
            DataReader replies,
            byte[] request,
            String what) {
        SampleIdentity id = forge.write(request);
        service.dispatch();
        List<TakenSample> out = replies.take();
        check(out.size() == 1, what + ": one reply");
        if (out.size() != 1) {
            return new ZdServices.robot.Calc_Reply();
        }
        check(out.get(0).relatedIdentity().equals(id),
                what + ": the reply names the request it answers (PID_RELATED_SAMPLE_IDENTITY)");
        return ZdServices.robot.CalcCodec.REPLY.deserialize(
                out.get(0).octets(), new HeaderCodec.Cursor(0), WireEncoding.XCDR2);
    }

    static void serviceSide() {
        LoopParticipant participant = new LoopParticipant();
        RPCRuntime runtime = RPCRuntime.over(participant);
        ZdServices.robot.CalcServiceImpl service =
                runtime.createService(p -> new ZdServices.robot.CalcServiceImpl(p, new Implementation()));
        DataWriter forge = participant.createWriter(
                ZdServices.robot.CalcSupport.REQUEST_TOPIC_NAME,
                ZdServices.robot.CalcSupport.REQUEST_TYPE_NAME, RpcQos.defaults(), "");
        DataReader replies = participant.createReader(
                ZdServices.robot.CalcSupport.REPLY_TOPIC_NAME,
                ZdServices.robot.CalcSupport.REPLY_TYPE_NAME, RpcQos.defaults(), "");

        ZdServices.robot.Calc_Reply good = answer(service, forge, replies,
                ZdServices.robot.CalcCodec.REQUEST.serialize(addRequest(2, 40)), "add(2, 40)");
        check(good.add != null && good.add.result != null && good.add.result.return_ == 42,
                "add(2, 40) is answered 42");
        check(good.remoteEx == null, "and carries no remoteEx");

        // §7.7.2: a member that is no operation of the interface.
        ZdServices.robot.Calc_Reply unknown =
                answer(service, forge, replies, foreignRequest(), "an unknown operation");
        check(unknown.remoteEx == RemoteExceptionCode.REMOTE_EX_UNKNOWN_OPERATION,
                "§7.7.2: an unknown operation is answered REMOTE_EX_UNKNOWN_OPERATION");
        check(unknown.add == null && unknown.ping == null && unknown.ident == null,
                "and the reply holds no other member");

        // A request with no member at all names no operation either.
        ZdServices.robot.Calc_Reply nothing =
                answer(service, forge, replies, emptyRequest(), "an empty request");
        check(nothing.remoteEx == RemoteExceptionCode.REMOTE_EX_UNKNOWN_OPERATION,
                "a request without a member is answered REMOTE_EX_UNKNOWN_OPERATION");

        // §7.5.2: a failure the operation does not declare.
        ZdServices.robot.Calc_Reply failed = answer(service, forge, replies,
                ZdServices.robot.CalcCodec.REQUEST.serialize(pingRequest()), "ping");
        check(failed.remoteEx == RemoteExceptionCode.REMOTE_EX_UNKNOWN_EXCEPTION,
                "§7.5.2: an undeclared failure is answered REMOTE_EX_UNKNOWN_EXCEPTION");

        // A declared exception travels in the Result of its operation.
        ZdServices.robot.Calc_Reply declared = answer(service, forge, replies,
                ZdServices.robot.CalcCodec.REQUEST.serialize(addRequest(-1, 0)), "add(-1, 0)");
        check(declared.add != null && declared.add.result == null
                        && declared.add.toofast_ex != null
                        && declared.add.toofast_ex.limit == 7
                        && "negative".equals(declared.add.toofast_ex.why),
                "a declared exception travels in the Result of the operation");
        check(declared.remoteEx == null, "and is no remote exception");

        // §7.5.1.2.1.1: a request of two members is malformed; the service
        // does not pick one of them.
        boolean refused = false;
        try {
            ZdServices.robot.CalcCodec.REQUEST.deserialize(twoMemberRequest(), new HeaderCodec.Cursor(0));
        } catch (RemoteInvalidArgumentError expected) {
            refused = true;
        }
        check(refused, "§7.5.1.2.1.1: the decoder refuses a request of two members");
        // §7.5.2: the service answers it REMOTE_EX_INVALID_ARGUMENT.
        SampleIdentity malformedId = forge.write(twoMemberRequest());
        service.dispatch();
        List<TakenSample> invalid = replies.take();
        check(invalid.size() == 1, "§7.5.2: the malformed request is answered");
        if (invalid.size() == 1) {
            ZdServices.robot.Calc_Reply refusal = ZdServices.robot.CalcCodec.REPLY.deserialize(
                    invalid.get(0).octets(), new HeaderCodec.Cursor(0), WireEncoding.XCDR2);
            check(refusal.remoteEx == RemoteExceptionCode.REMOTE_EX_INVALID_ARGUMENT,
                    "and the answer is REMOTE_EX_INVALID_ARGUMENT");
            check(invalid.get(0).relatedIdentity().equals(malformedId),
                    "which names the request it answers");
        }
        ZdServices.robot.Calc_Reply after = answer(service, forge, replies,
                ZdServices.robot.CalcCodec.REQUEST.serialize(addRequest(1, 1)), "add(1, 1) afterwards");
        check(after.add != null && after.add.result != null && after.add.result.return_ == 2,
                "the service answers the next request");
        service.close();
    }

    // -- the choice structures ---------------------------------------------

    static void choices() {
        // §7.5.1.2.1.1: "exactly one member shall be present at any given time".
        boolean none = false;
        try {
            ZdServices.robot.CalcCodec.REQUEST.serialize(new ZdServices.robot.Calc_Request());
        } catch (IllegalStateException expected) {
            none = true;
        }
        check(none, "§7.5.1.2.1.1: a request of no member is refused on encode");

        ZdServices.robot.Calc_Request two = addRequest(1, 2);
        two.ping = pingRequest().ping;
        boolean several = false;
        try {
            ZdServices.robot.CalcCodec.REQUEST.serialize(two);
        } catch (IllegalStateException expected) {
            several = true;
        }
        check(several, "§7.5.1.2.1.1: a request of two members is refused on encode");

        boolean noneReply = false;
        try {
            ZdServices.robot.CalcCodec.REPLY.serialize(new ZdServices.robot.Calc_Reply());
        } catch (IllegalStateException expected) {
            noneReply = true;
        }
        check(noneReply, "§7.5.1.2.1.1: a reply of no member is refused on encode");

        ZdServices.robot.Calc_Reply twoReplies = new ZdServices.robot.Calc_Reply();
        twoReplies.ping = new ZdServices.robot.Calc_ping_Result();
        twoReplies.remoteEx = RemoteExceptionCode.REMOTE_EX_UNSUPPORTED;
        boolean severalReply = false;
        try {
            ZdServices.robot.CalcCodec.REPLY.serialize(twoReplies);
        } catch (IllegalStateException expected) {
            severalReply = true;
        }
        check(severalReply, "§7.5.1.2.1.1: a reply of two members is refused on encode");

        // The Result is a choice as well: a result and an exception together.
        ZdServices.robot.Calc_Reply both = new ZdServices.robot.Calc_Reply();
        both.add = new ZdServices.robot.Calc_add_Result();
        both.add.result = new ZdServices.robot.Calc_add_Out();
        both.add.toofast_ex = new ZdTypes.robot.TooFast();
        boolean bothRefused = false;
        try {
            ZdServices.robot.CalcCodec.REPLY.serialize(both);
        } catch (IllegalStateException expected) {
            bothRefused = true;
        }
        check(bothRefused, "a Result of the value and an exception is refused on encode");
    }

    // -- the client side -----------------------------------------------------

    /**
     * Answers the one request the client has written with {@code reply} and
     * returns how the future of the call ended: {@code null} for a value.
     */
    static Throwable answered(
            ZdServices.robot.CalcClientImpl client,
            DataReader seen,
            DataWriter answers,
            Future<?> call,
            byte[] reply) throws InterruptedException {
        List<TakenSample> asked = seen.take();
        check(asked.size() == 1, "the client wrote its request");
        if (asked.size() != 1) {
            return new IllegalStateException("no request");
        }
        answers.write(reply, asked.get(0).identity());
        client.dispatchReplies();
        check(call.isDone(), "the reply completed the call");
        try {
            call.get();
            return null;
        } catch (ExecutionException failure) {
            return failure.getCause();
        }
    }

    static void clientSide(long add) throws Exception {
        LoopParticipant participant = new LoopParticipant();
        RPCRuntime runtime = RPCRuntime.over(participant);
        DataReader seen = participant.createReader(
                ZdServices.robot.CalcSupport.REQUEST_TOPIC_NAME,
                ZdServices.robot.CalcSupport.REQUEST_TYPE_NAME, RpcQos.defaults(), "");
        DataWriter answers = participant.createWriter(
                ZdServices.robot.CalcSupport.REPLY_TOPIC_NAME,
                ZdServices.robot.CalcSupport.REPLY_TYPE_NAME, RpcQos.defaults(), "");
        ZdServices.robot.CalcClientImpl client =
                runtime.createClient(ZdServices.robot.CalcClientImpl::new);

        @SuppressWarnings("unchecked")
        EnhancedReplySupport<ZdServices.robot.Calc_Reply> support =
                (EnhancedReplySupport<ZdServices.robot.Calc_Reply>) ZdServices.robot.CalcCodec.REPLY;

        // A well-formed reply is the value.
        ZdServices.robot.Calc_Reply ok = new ZdServices.robot.Calc_Reply();
        ok.add = new ZdServices.robot.Calc_add_Result();
        ok.add.result = new ZdServices.robot.Calc_add_Out();
        ok.add.result.return_ = 3;
        Future<Integer> sum = client.add_async(1, 2);
        check(answered(client, seen, answers, sum, support.serialize(ok)) == null && sum.get() == 3,
                "a reply with a result is the value");

        // remoteEx = REMOTE_EX_UNKNOWN_OPERATION raises at the call site.
        Throwable unknownOperation = answered(client, seen, answers, client.add_async(1, 2),
                support.serialize(
                        support.remoteExceptionSample(RemoteExceptionCode.REMOTE_EX_UNKNOWN_OPERATION)));
        check(unknownOperation instanceof RemoteUnknownOperationError,
                "remoteEx = REMOTE_EX_UNKNOWN_OPERATION raises RemoteUnknownOperationError");

        // The reply answers another operation than the request asked.
        ZdServices.robot.Calc_Reply other = new ZdServices.robot.Calc_Reply();
        other.ping = new ZdServices.robot.Calc_ping_Result();
        other.ping.result = new ZdServices.robot.Calc_ping_Out();
        other.ping.result.dummy = new org.omg.dds.rpc.UnusedMember();
        Throwable wrongOperation = answered(client, seen, answers, client.add_async(1, 2),
                support.serialize(other));
        check(wrongOperation instanceof RemoteUnknownOperationError,
                "a reply that answers another operation raises RemoteUnknownOperationError");

        // §7.5.2: a Result whose exception member the client does not know.
        Throwable unknownException = answered(client, seen, answers, client.add_async(1, 2),
                foreignExceptionReply(add));
        check(unknownException instanceof RemoteUnknownExceptionError,
                "§7.5.2: an exception member the client does not know raises RemoteUnknownExceptionError");

        // remoteEx = REMOTE_EX_UNKNOWN_EXCEPTION as the service sends it.
        Throwable remote = answered(client, seen, answers, client.add_async(1, 2),
                support.serialize(
                        support.remoteExceptionSample(RemoteExceptionCode.REMOTE_EX_UNKNOWN_EXCEPTION)));
        check(remote instanceof RemoteUnknownExceptionError,
                "remoteEx = REMOTE_EX_UNKNOWN_EXCEPTION raises RemoteUnknownExceptionError");

        // A declared exception is the user exception of the operation.
        ZdServices.robot.Calc_Reply raised = new ZdServices.robot.Calc_Reply();
        raised.add = new ZdServices.robot.Calc_add_Result();
        raised.add.toofast_ex = new ZdTypes.robot.TooFast();
        raised.add.toofast_ex.limit = 5;
        raised.add.toofast_ex.why = "slow down";
        Throwable declared = answered(client, seen, answers, client.add_async(1, 2),
                support.serialize(raised));
        check(declared instanceof UserExceptionRaised
                        && ((UserExceptionRaised) declared).value() instanceof ZdTypes.robot.TooFast
                        && ((ZdTypes.robot.TooFast) ((UserExceptionRaised) declared).value()).limit == 5,
                "a declared exception is raised as the exception the operation declares");
        client.close();
    }

    // -- the Support names the interface -------------------------------------

    static void support() {
        LoopParticipant participant = new LoopParticipant();
        RequesterParams params =
                ZdServices.robot.CalcSupport.requesterParams(participant);
        check("robot_Calc".equals(params.interfaceName()),
                "§7.4.1: the generated Support sets interfaceName to the module-qualified interface");
        check(ZdServices.robot.CalcSupport.INTERFACE_NAME.equals(params.interfaceName()),
                "and it is the INTERFACE_NAME constant");
        check(TopicNames.of(params).request().equals(ZdServices.robot.CalcSupport.REQUEST_TOPIC_NAME)
                        && TopicNames.of(params).reply().equals(ZdServices.robot.CalcSupport.REPLY_TOPIC_NAME),
                "with no run-time name the topics are the generated ones");
        // §7.4.2.3: a run-time service name composes <interface>_<service>_Request.
        check(TopicNames.of(params.serviceName("Left")).request().equals("robot_Calc_Left_Request")
                        && TopicNames.of(params).reply().equals("robot_Calc_Left_Reply"),
                "§7.4.2.3: a run-time service name composes <interface>_<service>_Request");
        // §7.4.3, §7.5.1.2.6: the derived interface announces its base as a topic alias.
        check(java.util.Arrays.asList("robot_Base2").equals(params.parents()),
                "§7.4.3: the Support of a derived Enhanced interface names its base as a parent");
        check(ZdServices.robot.Base2Support.requesterParams(participant).parents().isEmpty(),
                "and the Support of the base has no parent");
        check("robot_Base2_Left_Request".equals(TopicNames.of(params).requestAliases().get(0)),
                "§7.4.3: the alias is the base's request topic");
        check("robot_Base2".equals(ZdServices.robot.Base2Support.requesterParams(participant).interfaceName()),
                "the Support of a base interface names the base");
        check("robot_Calc".equals(
                        ZdServices.robot.CalcSupport.replierParams(participant, "").interfaceName()),
                "and so does the replier side");

        ZdServices.robot.CalcServiceImpl service =
                new ZdServices.robot.CalcServiceImpl(participant, new Implementation());
        check(participant.topics.contains("reader " + ZdServices.robot.CalcSupport.REQUEST_TOPIC_NAME)
                        && participant.topics.contains("writer " + ZdServices.robot.CalcSupport.REPLY_TOPIC_NAME),
                "a service made from the Support listens on the generated topics");
        service.close();

        // §7.11.2.2.3: the runtime makes a requester and a replier from the two topic classes.
        RPCRuntime runtime = RPCRuntime.over(participant);
        check(RPCRuntime.getInstance(EnhancedRuntimeChecks.class.getClassLoader()) != null,
                "§7.11.2.2.3: getInstance(ClassLoader) returns the singleton");
        Replier<ZdServices.robot.Calc_Request, ZdServices.robot.Calc_Reply> replier =
                runtime.createReplier(ZdServices.robot.Calc_Request.class, ZdServices.robot.Calc_Reply.class);
        Requester<ZdServices.robot.Calc_Request, ZdServices.robot.Calc_Reply> requester =
                runtime.createRequester(ZdServices.robot.Calc_Request.class, ZdServices.robot.Calc_Reply.class);
        SampleIdentity sent = SampleIdentity.of(requester.sendRequest(addRequest(20, 22)));
        check(!sent.isUnknown(), "§7.11.2.2.3: a requester made from the classes sends");
        List<Sample<ZdServices.robot.Calc_Request>> got = replier.receiveRequests();
        check(got.size() == 1 && got.get(0).getData().add != null && got.get(0).getData().add.a == 20,
                "and the replier made from the classes receives it");
        boolean refused = false;
        try {
            runtime.createRequester(String.class, Integer.class);
        } catch (IllegalArgumentException expected) {
            refused = true;
        }
        check(refused, "classes that are no topic types of a generated interface are refused");
        runtime.run();
    }

    public static void main(String[] args) throws Exception {
        long add = Long.parseLong(args[0]);
        serviceSide();
        choices();
        clientSide(add);
        support();
        if (failures != 0) {
            System.err.println(failures + " checks failed");
            System.exit(1);
        }
        System.out.println("every check held");
    }
}
