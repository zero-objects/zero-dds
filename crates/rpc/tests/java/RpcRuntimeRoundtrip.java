// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Java request-reply and function-call bindings of DDS-RPC 1.0
// §7.11.2, exercised end to end over an in-process binding of the three
// entity interfaces `org.omg.dds.rpc` leaves abstract.
//
// Every type §7.11.2.2.1 and §7.11.2.3 name is used here, not merely
// declared, and the generated `ZdServices` of
// `crates/lang-rules/src/emit/interface_java.rs` is what the request and
// the reply travel in — so this is also the check that the generator and
// the runtime agree.

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutionException;

import org.omg.dds.rpc.ClientEndpoint;
import org.omg.dds.rpc.DataReader;
import org.omg.dds.rpc.DataWriter;
import org.omg.dds.rpc.DomainParticipant;
import org.omg.dds.rpc.Future;
import org.omg.dds.rpc.FutureCompletionListener;
import org.omg.dds.rpc.HeaderCodec;
import org.omg.dds.rpc.RPCRuntime;
import org.omg.dds.rpc.EnhancedReplySupport;
import org.omg.dds.rpc.RelatedIdentity;
import org.omg.dds.rpc.RemoteExceptionCode;
import org.omg.dds.rpc.RemoteInvalidArgumentError;
import org.omg.dds.rpc.RemoteOutOfResourcesError;
import org.omg.dds.rpc.RemoteUnsupportedError;
import org.zerodds.ddsrpc.ReplierImpl;
import org.zerodds.ddsrpc.RequesterImpl;
import org.zerodds.ddsrpc.RawTypeSupport;
import org.zerodds.ddsrpc.ZeroDdsParticipant;
import org.omg.dds.rpc.Replier;
import org.omg.dds.rpc.Requester;
import org.omg.dds.rpc.RequesterErrorListener;
import org.omg.dds.rpc.RpcQos;
import org.omg.dds.rpc.ServiceMapping;
import org.omg.dds.rpc.ServiceMatching;
import org.omg.dds.rpc.WireEncoding;
import org.omg.dds.rpc.RemoteUnknownOperationError;
import org.omg.dds.rpc.ReplierParams;
import org.omg.dds.rpc.ReplyHeader;
import org.omg.dds.rpc.RequestHeader;
import org.omg.dds.rpc.RequesterParams;
import org.omg.dds.rpc.PlaceholderTypes;
import org.omg.dds.rpc.Sample;
import org.omg.dds.rpc.SampleIdentity;
import org.omg.dds.rpc.Server;
import org.omg.dds.rpc.Service;
import org.omg.dds.rpc.SimpleReplierListener;
import org.omg.dds.rpc.SimpleRequesterListener;
import org.omg.dds.rpc.TakenSample;
import org.omg.dds.rpc.TypeSupport;

public final class RpcRuntimeRoundtrip {

    private static int failures = 0;

    // -- the in-process binding of the three abstract entities ---------

    static final class Topic {
        final List<TakenSample> samples = new ArrayList<>();
        final List<LoopbackReader> readers = new ArrayList<>();
        int writers = 0;
    }

    static final class Bus {
        private final Map<String, Topic> topics = new HashMap<>();
        /** Whether a write calls the readers' data-available listeners. */
        boolean notify = false;
        /**
         * Whether a reader reports the identity of a sample and its related
         * identity. Off, the binding behaves as the one over ZeroDDS does,
         * where the reader has no accessor for either.
         */
        boolean reportIdentity = false;
        /** Whether a writer can send PID_RELATED_SAMPLE_IDENTITY. */
        boolean carryRelated = true;
        /** The last body written to each topic. */
        final Map<String, byte[]> lastSample = new HashMap<>();
        /** The related identity of the last write to each topic. */
        final Map<String, SampleIdentity> lastRelated = new HashMap<>();

        Topic topic(String name) {
            return topics.computeIfAbsent(name, key -> new Topic());
        }
    }

    static final class LoopbackWriter implements DataWriter {
        private final Bus bus;
        private final String topic;
        private final byte[] guid = new byte[16];
        private long sequence = 0;
        RpcQos qos;

        LoopbackWriter(Bus bus, String topic, int tag, RpcQos qos) {
            this.bus = bus;
            this.topic = topic;
            this.qos = qos;
            for (int at = 0; at < 16; ++at) {
                guid[at] = (byte) (tag + at);
            }
            ++bus.topic(topic).writers;
        }

        @Override
        public SampleIdentity nextIdentity() {
            return new SampleIdentity(guid, sequence + 1);
        }

        private SampleIdentity deliver(byte[] sample, SampleIdentity related) {
            SampleIdentity identity = nextIdentity();
            ++sequence;
            Topic held = bus.topic(topic);
            bus.lastSample.put(topic, sample);
            bus.lastRelated.put(topic, related);
            held.samples.add(
                    bus.reportIdentity
                            ? new TakenSample(sample, identity, related)
                            : new TakenSample(sample, SampleIdentity.UNKNOWN));
            if (bus.notify) {
                for (LoopbackReader reader : new ArrayList<>(held.readers)) {
                    if (reader.listener != null) {
                        reader.listener.run();
                    }
                }
            }
            return identity;
        }

        @Override
        public SampleIdentity write(byte[] sample) {
            return deliver(sample, SampleIdentity.UNKNOWN);
        }

        @Override
        public SampleIdentity write(byte[] sample, SampleIdentity related) {
            if (!bus.carryRelated) {
                throw new RemoteUnsupportedError("no inline QoS on this binding");
            }
            return deliver(sample, related);
        }

        @Override
        public boolean carriesRelatedIdentity() {
            return bus.carryRelated;
        }

        @Override
        public int matchedCount() {
            return bus.topic(topic).readers.size();
        }

        @Override
        public int matchedCount(String serviceInstanceName) {
            int n = 0;
            for (LoopbackReader reader : bus.topic(topic).readers) {
                if (reader.instance.equals(serviceInstanceName)) {
                    ++n;
                }
            }
            return n;
        }

        @Override
        public java.util.Collection<String> matchedInstances() {
            java.util.Set<String> names = new java.util.LinkedHashSet<>();
            for (LoopbackReader reader : bus.topic(topic).readers) {
                if (!reader.instance.isEmpty()) {
                    names.add(reader.instance);
                }
            }
            return names;
        }

        @Override
        public SampleIdentity writeTo(byte[] sample, String serviceInstanceName) {
            if (matchedCount(serviceInstanceName) == 0) {
                throw new RemoteOutOfResourcesError("no reader announces " + serviceInstanceName);
            }
            return deliver(sample, SampleIdentity.UNKNOWN);
        }

        @Override
        public void setQos(RpcQos requested) {
            qos = requested;
        }
    }

    static final class LoopbackReader implements DataReader {
        private final Bus bus;
        private final String topic;
        final String instance;
        Runnable listener;
        RpcQos qos;

        LoopbackReader(Bus bus, String topic, String instance, RpcQos qos) {
            this.bus = bus;
            this.topic = topic;
            this.instance = instance;
            this.qos = qos;
            bus.topic(topic).readers.add(this);
        }

        @Override
        public List<TakenSample> take() {
            List<TakenSample> held = bus.topic(topic).samples;
            List<TakenSample> out = new ArrayList<>(held);
            held.clear();
            return out;
        }

        @Override
        public int matchedCount() {
            return bus.topic(topic).writers;
        }

        @Override
        public void setDataAvailableListener(Runnable listener) {
            this.listener = listener;
        }

        @Override
        public void setQos(RpcQos requested) {
            qos = requested;
        }
    }

    /** One createWriter or createReader the participant served. */
    static final class Created {
        final boolean writer;
        final String topic;
        final String typeName;
        final RpcQos qos;
        final String instance;
        final Object entity;

        Created(
                boolean writer,
                String topic,
                String typeName,
                RpcQos qos,
                String instance,
                Object entity) {
            this.entity = entity;
            this.writer = writer;
            this.topic = topic;
            this.typeName = typeName;
            this.qos = qos;
            this.instance = instance;
        }
    }

    static final class LoopbackParticipant implements DomainParticipant {
        private final Bus bus;
        private int tag = 0x10;
        /** §7.9.3: how many entities a close() has deleted. */
        int deletedWriters = 0;
        int deletedReaders = 0;
        final List<Created> created = new ArrayList<>();

        LoopbackParticipant(Bus bus) {
            this.bus = bus;
        }

        Created createdOn(String topic, boolean writer) {
            for (Created entry : created) {
                if (entry.topic.equals(topic) && entry.writer == writer) {
                    return entry;
                }
            }
            return null;
        }

        @Override
        public DataWriter createWriter(String topic, String typeName, RpcQos qos, String instance) {
            LoopbackWriter writer = new LoopbackWriter(bus, topic, ++tag, qos);
            created.add(new Created(true, topic, typeName, qos, instance, writer));
            return writer;
        }

        @Override
        public DataReader createReader(String topic, String typeName, RpcQos qos, String instance) {
            LoopbackReader reader = new LoopbackReader(bus, topic, instance, qos);
            created.add(new Created(false, topic, typeName, qos, instance, reader));
            return reader;
        }

        @Override
        public void deleteWriter(DataWriter writer) {
            ++deletedWriters;
        }

        @Override
        public void deleteReader(DataReader reader) {
            ++deletedReaders;
        }
    }

    // -- the codecs the two topic types need ---------------------------
    //
    // The Java counterpart of §7.11.1.4.22's `dds_type_traits<T>`.
    // Written by hand here and not generated: what is measured is the
    // runtime, and the codec walk has its own twenty-one columns.

    /**
     * §7.5.1.1.6 rule 5a gives the case label as the HASH of the
     * operation name; the generated class declares it beside the union.
     */
    static final int OP_ADD = ZdServices.robot.Calculator_add_Hash;

    static final TypeSupport<ZdServices.robot.Calculator_Request> REQUEST_SUPPORT =
            new TypeSupport<ZdServices.robot.Calculator_Request>() {
                @Override
                public byte[] serialize(ZdServices.robot.Calculator_Request value) {
                    ByteArrayOutputStream out = new ByteArrayOutputStream();
                    HeaderCodec.putU32(out, value.data._d);
                    HeaderCodec.putU32(out, value.data.add.a);
                    HeaderCodec.putU32(out, value.data.add.b);
                    return out.toByteArray();
                }

                @Override
                public ZdServices.robot.Calculator_Request deserialize(
                        byte[] octets, HeaderCodec.Cursor cursor) {
                    ZdServices.robot.Calculator_Request value =
                            new ZdServices.robot.Calculator_Request();
                    value.data = new ZdServices.robot.Calculator_Call();
                    value.data.add = new ZdServices.robot.Calculator_add_In();
                    value.data._d = HeaderCodec.getU32(octets, cursor);
                    value.data.add.a = HeaderCodec.getU32(octets, cursor);
                    value.data.add.b = HeaderCodec.getU32(octets, cursor);
                    return value;
                }
            };

    static final TypeSupport<ZdServices.robot.Calculator_Reply> REPLY_SUPPORT =
            new TypeSupport<ZdServices.robot.Calculator_Reply>() {
                @Override
                public byte[] serialize(ZdServices.robot.Calculator_Reply value) {
                    ByteArrayOutputStream out = new ByteArrayOutputStream();
                    HeaderCodec.putU32(out, value.data._d);
                    HeaderCodec.putU32(out, value.data.add.result.return_);
                    return out.toByteArray();
                }

                @Override
                public ZdServices.robot.Calculator_Reply deserialize(
                        byte[] octets, HeaderCodec.Cursor cursor) {
                    ZdServices.robot.Calculator_Reply value =
                            new ZdServices.robot.Calculator_Reply();
                    value.data = new ZdServices.robot.Calculator_Return();
                    value.data.add = new ZdServices.robot.Calculator_add_Result();
                    value.data.add.result = new ZdServices.robot.Calculator_add_Out();
                    value.data._d = HeaderCodec.getU32(octets, cursor);
                    value.data.add.result.return_ = HeaderCodec.getU32(octets, cursor);
                    return value;
                }
            };

    static ZdServices.robot.Calculator_Request addRequest(int a, int b) {
        ZdServices.robot.Calculator_Request request = new ZdServices.robot.Calculator_Request();
        request.data = new ZdServices.robot.Calculator_Call();
        request.data.add = new ZdServices.robot.Calculator_add_In();
        request.data._d = OP_ADD;
        request.data.add.a = a;
        request.data.add.b = b;
        return request;
    }

    /** §7.11.1.4.7's synchronous replier listener. */
    static final class AddReplier
            implements SimpleReplierListener<
                    ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> {
        int calls = 0;

        @Override
        public ZdServices.robot.Calculator_Reply processRequest(
                Sample<ZdServices.robot.Calculator_Request> request) {
            ++calls;
            ZdServices.robot.Calculator_Reply reply = new ZdServices.robot.Calculator_Reply();
            reply.data = new ZdServices.robot.Calculator_Return();
            reply.data.add = new ZdServices.robot.Calculator_add_Result();
            reply.data.add.result = new ZdServices.robot.Calculator_add_Out();
            reply.data._d = request.getData().data._d;
            reply.data.add._d = PlaceholderTypes.RETCODE_OK;
            reply.data.add.result.return_ = request.getData().data.add.a + request.getData().data.add.b;
            return reply;
        }
    }

    /** §7.11.1.4.9's requester listener. */
    static final class CollectReplies
            implements SimpleRequesterListener<ZdServices.robot.Calculator_Reply> {
        final List<Integer> sums = new ArrayList<>();

        @Override
        public void processReply(Sample<ZdServices.robot.Calculator_Reply> reply) {
            sums.add(reply.getData().data.add.result.return_);
        }
    }

    /** §7.11.1.5.1's `Service`, over the generated interface. */
    static final class CalculatorService implements Service {
        private final String name;
        private final AddReplier listener = new AddReplier();
        private final ZdServices.robot.CalculatorReplier replier;
        private boolean paused = false;
        private boolean closed = false;

        CalculatorService(DomainParticipant participant, String name) {
            this.name = name;
            ReplierParams params = replierParams(participant, name);
            params.listener(listener);
            this.replier = new ZdServices.robot.CalculatorReplier(params);
        }

        @Override
        public int dispatch() {
            return replier.dispatchRequests();
        }

        @Override
        public String serviceName() {
            return name;
        }

        @Override
        public void pause() {
            paused = true;
        }

        @Override
        public void resume() {
            paused = false;
        }

        @Override
        public boolean isPaused() {
            return paused;
        }

        @Override
        public void close() {
            closed = true;
            replier.close();
        }

        @Override
        public boolean isNull() {
            return closed;
        }

        @Override
        public <T> org.omg.dds.sub.DataReader<T> getRequestDataReader(Class<?> requestType) {
            return requestType == ZdServices.robot.Calculator_Request.class
                    ? (org.omg.dds.sub.DataReader<T>) (Object) replier.getRequestDataReader() : null;
        }

        @Override
        public <T> org.omg.dds.pub.DataWriter<T> getReplyDataWriter(Class<?> replyType) {
            return replyType == ZdServices.robot.Calculator_Reply.class
                    ? (org.omg.dds.pub.DataWriter<T>) (Object) replier.getReplyDataWriter() : null;
        }

        int answered() {
            return listener.calls;
        }
    }


    /** The params every replier of this test starts from. */
    static ReplierParams replierParams(DomainParticipant participant, String instance) {
        ReplierParams params = ReplierParams.create();
        params.domainParticipant(participant)
                .requestTopicName(ZdServices.robot.CalculatorSupport.REQUEST_TOPIC_NAME)
                .replyTopicName(ZdServices.robot.CalculatorSupport.REPLY_TOPIC_NAME)
                .requestTypeName(ZdServices.robot.CalculatorSupport.REQUEST_TYPE_NAME)
                .replyTypeName(ZdServices.robot.CalculatorSupport.REPLY_TYPE_NAME)
                // §7.11.1.4.3: the instance this replier answers; it must
                // equal the instance the requester bound to.
                .instanceName(instance)
                .requestTypeSupport(REQUEST_SUPPORT)
                .replyTypeSupport(REPLY_SUPPORT);
        return params;
    }

    /** The params every requester of this test starts from. */
    static RequesterParams requesterParams(DomainParticipant participant) {
        RequesterParams params = RequesterParams.create();
        params.domainParticipant(participant)
                .requestTopicName(ZdServices.robot.CalculatorSupport.REQUEST_TOPIC_NAME)
                .replyTopicName(ZdServices.robot.CalculatorSupport.REPLY_TOPIC_NAME)
                .requestTypeName(ZdServices.robot.CalculatorSupport.REQUEST_TYPE_NAME)
                .replyTypeName(ZdServices.robot.CalculatorSupport.REPLY_TYPE_NAME)
                .requestTypeSupport(REQUEST_SUPPORT)
                .replyTypeSupport(REPLY_SUPPORT);
        return params;
    }


    // -- helpers for the new sections --------------------------------------

    static final String REQ = ZdServices.robot.CalculatorSupport.REQUEST_TOPIC_NAME;
    static final String REP = ZdServices.robot.CalculatorSupport.REPLY_TOPIC_NAME;

    /** A request sample as the Basic mapping frames it, built by hand. */
    static byte[] basicRequestBytes(
            SampleIdentity id, String instance, ZdServices.robot.Calculator_Request request) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] header = HeaderCodec.encodeRequestHeader(new RequestHeader(id, instance));
        out.write(header, 0, header.length);
        HeaderCodec.padTo4(out);
        byte[] payload = REQUEST_SUPPORT.serialize(request);
        out.write(payload, 0, payload.length);
        return out.toByteArray();
    }

    /** The two QoS-bearing sections of the Enhanced test: a type of one 32-bit value. */
    static final class IntSupport implements EnhancedReplySupport<Integer> {
        @Override
        public byte[] serialize(Integer value) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            HeaderCodec.putU32(out, value);
            return out.toByteArray();
        }

        @Override
        public Integer deserialize(byte[] octets, HeaderCodec.Cursor cursor) {
            return HeaderCodec.getU32(octets, cursor);
        }

        /** A value of -1000 or less stands for {@code remoteEx = -1000 - value}. */
        @Override
        public RemoteExceptionCode remoteExceptionOf(Integer reply) {
            return reply <= -1000
                    ? RemoteExceptionCode.fromWire(-1000 - reply)
                    : RemoteExceptionCode.REMOTE_EX_OK;
        }

        @Override
        public Integer remoteExceptionSample(RemoteExceptionCode code) {
            return -1000 - code.wireValue();
        }
    }

    static final IntSupport INTS = new IntSupport();
    static final String EREQ = "Doubler_Service_Request";
    static final String EREP = "Doubler_Service_Reply";

    static ReplierParams enhancedReplier(DomainParticipant participant, String instance) {
        ReplierParams params = ReplierParams.create();
        params.domainParticipant(participant)
                .requestTopicName(EREQ)
                .replyTopicName(EREP)
                .requestTypeName("Doubler_Request")
                .replyTypeName("Doubler_Reply")
                .requestTypeSupport(INTS)
                .replyTypeSupport(INTS)
                .instanceName(instance)
                .mapping(ServiceMapping.ENHANCED);
        return params;
    }

    static RequesterParams enhancedRequester(DomainParticipant participant) {
        RequesterParams params = RequesterParams.create();
        params.domainParticipant(participant)
                .requestTopicName(EREQ)
                .replyTopicName(EREP)
                .requestTypeName("Doubler_Request")
                .replyTypeName("Doubler_Reply")
                .requestTypeSupport(INTS)
                .replyTypeSupport(INTS)
                .mapping(ServiceMapping.ENHANCED);
        return params;
    }

    /** Both a reply listener and an error listener, for the arrival section. */
    static final class CollectAll
            implements SimpleRequesterListener<ZdServices.robot.Calculator_Reply>,
                    RequesterErrorListener {
        final List<Integer> sums = new ArrayList<>();
        final List<RuntimeException> failed = new ArrayList<>();

        @Override
        public void processReply(Sample<ZdServices.robot.Calculator_Reply> reply) {
            sums.add(reply.getData().data.add.result.return_);
        }

        @Override
        public void onReplyFailure(SampleIdentity request, RuntimeException failure) {
            failed.add(failure);
        }
    }

    static ZdServices.robot.Calculator_Reply refusalReply() {
        ZdServices.robot.Calculator_Reply refusal = new ZdServices.robot.Calculator_Reply();
        refusal.data = new ZdServices.robot.Calculator_Return();
        refusal.data.add = new ZdServices.robot.Calculator_add_Result();
        refusal.data.add.result = new ZdServices.robot.Calculator_add_Out();
        return refusal;
    }

    static void check(boolean held, String what) {
        if (!held) {
            System.err.println("FAIL " + what);
            ++failures;
        }
    }

    /** The octets the two headers take, for the Rust side to compare. */
    static void printHeaderOctets() {
        byte[] guid = new byte[16];
        for (int at = 0; at < 16; ++at) {
            guid[at] = 0x42;
        }
        SampleIdentity identity = new SampleIdentity(guid, 7);
        for (byte[] octets : new byte[][] {
            HeaderCodec.encodeRequestHeader(new RequestHeader(identity, "calc-A")),
            HeaderCodec.encodeReplyHeader(
                    new ReplyHeader(identity, RemoteExceptionCode.REMOTE_EX_INVALID_ARGUMENT)),
        }) {
            StringBuilder line = new StringBuilder();
            for (byte octet : octets) {
                line.append(String.format("%02x", octet));
            }
            System.out.println(line);
        }
    }


    // =====================================================================
    // §7.11.2.2 and §7.11.2.3 over the polling binding: the Basic mapping
    // =====================================================================

    static void basicPolling() {
        Bus bus = new Bus();
        LoopbackParticipant participant = new LoopbackParticipant(bus);

        // §7.11.2.2.3: the runtime is the factory for the other entities,
        // and the singleton of its participant.
        RPCRuntime runtime = RPCRuntime.over(participant);
        check(runtime.domainParticipant() == participant, "the runtime holds the participant");
        check(RPCRuntime.over(participant) == runtime, "over() returns the one runtime of a participant");

        CollectReplies replies = new CollectReplies();
        RequesterParams requesterParams = requesterParams(participant);
        requesterParams.listener(replies);

        CalculatorService service = new CalculatorService(participant, "Calculator");

        // §7.11.2.2.1: the typed per-service Requester the generator wrote.
        ZdServices.robot.CalculatorRequester requester =
                new ZdServices.robot.CalculatorRequester(requesterParams);

        // §7.5.1.1.6/7: the topics are announced under the interface's
        // type names, the two writers and the two readers.
        Created requestWriter = participant.createdOn(REQ, true);
        check(requestWriter != null
                && requestWriter.typeName.equals("robot::Calculator_Request"),
                "the request topic is announced as robot::Calculator_Request");
        Created replyReader = participant.createdOn(REP, false);
        check(replyReader != null && replyReader.typeName.equals("robot::Calculator_Reply"),
                "the reply topic is announced as robot::Calculator_Reply");
        // §7.6.2.1.1/2: "the client-side DataWriter shall not include
        // service_instance_name", and nor does the client-side DataReader;
        // the service side names its instance.
        check(requestWriter != null && requestWriter.instance.isEmpty(),
                "the requester's writer announces no service_instance_name");
        check(replyReader != null && replyReader.instance.isEmpty(),
                "the requester's reader announces no service_instance_name");
        Created serviceReader = participant.createdOn(REQ, false);
        check(serviceReader != null && serviceReader.instance.equals("Calculator"),
                "the service's reader announces its instance");
        // §7.10.2: no QoS given, the defaults: RELIABLE, KEEP_ALL, VOLATILE.
        check(requestWriter != null && requestWriter.qos.equals(RpcQos.defaults()),
                "the default QoS is in effect");
        check(RpcQos.defaults().reliability() == RpcQos.Reliability.RELIABLE
                && RpcQos.defaults().historyKind() == RpcQos.HistoryKind.KEEP_ALL
                && RpcQos.defaults().durability() == RpcQos.Durability.VOLATILE,
                "§7.10.2: RELIABLE, KEEP_ALL, VOLATILE");

        // The runtime makes the same entity, and returns itself as the
        // environment of what it made.
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> made =
                runtime.createRequester(requesterParams(participant));
        check(made.getEnvironment() == runtime, "an entity returns the runtime it was made by");
        made.close();

        check(!requester.isNull(), "a constructed requester is not null");
        check(!requester.isBound(), "a requester starts unbound");
        requester.bind("Calculator");
        check(requester.isBound(), "bind() binds");
        check("Calculator".equals(requester.boundInstanceName()), "the bound name is kept");
        check(requester.isServiceMatched(), "the replier's reader matched the requester's writer");

        SampleIdentity first = requester.sendRequest(addRequest(2, 40));
        check(!first.isUnknown(), "sendRequest returns the identity the middleware gave");

        // §7.5.1.1.6: the request is the header, padding to a four-octet
        // boundary, and then the `long` discriminator of the Call union.
        // "Calculator" makes the header 24 + 4 + 11 = 39 octets.
        byte[] request = bus.lastSample.get(REQ);
        check(request != null && request.length == 40 + 12, "request length: header, pad, union");
        check(request != null && request[39] == 0, "one padding octet follows the header");
        check(request != null
                && (request[40] & 0xFF | (request[41] & 0xFF) << 8 | (request[42] & 0xFF) << 16
                        | (request[43] & 0xFF) << 24) == OP_ADD,
                "the discriminator starts on the four-octet boundary");

        // §7.11.1.5.3: the Server dispatches into the Service.
        Server server = Server.create();
        server.add(service);
        check(server.size() == 1, "the server holds one service");
        boolean twice = false;
        try {
            Server.create().add(service);
        } catch (IllegalStateException expected) {
            twice = true;
        }
        check(twice, "§7.11.1.5.1: a Service belongs to only one Server");
        check(server.runOnce() == 1, "one request was answered");
        check(service.answered() == 1, "the replier listener saw it");
        byte[] replyBytes = bus.lastSample.get(REP);
        check(replyBytes != null && replyBytes.length == 28 + 8, "reply length: header, union");
        // §7.8.2: the reply carries the identity of the request in the
        // inline QoS, besides the header of the Basic mapping.
        check(first.equals(bus.lastRelated.get(REP)), "PID_RELATED_SAMPLE_IDENTITY is the request's identity");

        // §7.8.1: the reply is correlated by the request's identity.
        Sample<ZdServices.robot.Calculator_Reply> reply = requester.receiveReply(first);
        check(reply != null, "the reply arrived");
        check(reply != null && reply.getData().data.add.result.return_ == 42, "2 + 40");
        check(reply != null && reply.relatedIdentity().equals(first), "related_request_id");

        // §7.2.1: the content-based filter drops a reply that answers
        // another client's request (a foreign request-writer GUID). A
        // requester without the filter would hand it out.
        {
            DataWriter foreignWriter =
                    participant.createWriter(REP, "robot::Calculator_Reply", RpcQos.defaults(), "");
            byte[] foreignGuid = new byte[16];
            foreignGuid[0] = (byte) 0xEE;
            ReplyHeader foreignHeader =
                    new ReplyHeader(new SampleIdentity(foreignGuid, 1), RemoteExceptionCode.REMOTE_EX_OK);
            foreignWriter.write(HeaderCodec.encodeReplyHeader(foreignHeader));
            check(requester.receiveReplies().isEmpty(),
                    "a reply to another client is filtered out");
        }

        // §7.11.1.4.3: an unnamed replier answers only the default
        // instance, so a request bound to "Calculator" is ignored.
        {
            ZdServices.robot.CalculatorReplier unnamed =
                    new ZdServices.robot.CalculatorReplier(replierParams(participant, ""));
            requester.sendRequest(addRequest(0, 0));
            check(unnamed.receiveRequests().isEmpty(),
                    "an unnamed replier ignores a request bound to an instance");
        }

        // §7.2.1.3: two outstanding requests, taken in the other order.
        requester.sendRequest(addRequest(1, 1));
        SampleIdentity second = requester.sendRequest(addRequest(10, 10));
        check(server.runOnce() == 2, "both were answered");
        Sample<ZdServices.robot.Calculator_Reply> later = requester.receiveReply(second);
        check(later != null && later.getData().data.add.result.return_ == 20, "10 + 10");
        check(requester.receiveReply(second) == null, "a reply is taken once");

        // §7.11.2.2.7: the iterator over the replies that have arrived is a
        // ListIterator.
        requester.sendRequest(addRequest(3, 4));
        check(server.runOnce() == 1, "the fourth request was answered");
        Sample.Iterator<ZdServices.robot.Calculator_Reply> walk = requester.replyIterator();
        check(walk instanceof java.util.ListIterator, "Sample.Iterator is a java.util.ListIterator");
        int seen = 0;
        while (walk.hasNext()) {
            seen += walk.next().getData().data.add.result.return_;
        }
        check(seen == 9, "(1 + 1) + (3 + 4) through the iterator");
        check(walk.hasPrevious() && walk.previousIndex() == 1, "the iterator walks back as well");
        boolean refused = false;
        try {
            walk.remove();
        } catch (UnsupportedOperationException expected) {
            refused = true;
        }
        check(refused, "the iterator over a taken list does not remove");

        // §7.11.1.4.9: the requester listener.
        requester.sendRequest(addRequest(5, 6));
        check(server.runOnce() == 1, "the fifth request was answered");
        requester.dispatchReplies();
        check(replies.sums.size() == 1 && replies.sums.get(0) == 11, "the listener saw 5 + 6");

        // §7.11.2.2.4 and §7.11.2.2.5: the future and its completion
        // listener. The future is pending until the reply arrives.
        Future<Sample<ZdServices.robot.Calculator_Reply>> pending =
                requester.sendRequestAsync(addRequest(7, 8));
        check(!pending.isDone(), "the future is pending before the service has answered");
        check(server.runOnce() == 1, "the sixth request was answered");
        requester.receiveReplies();
        check(pending.isDone(), "the future is ready once the reply has been read");
        final boolean[] notified = {false};
        pending.withCompletionListener(
                new FutureCompletionListener<Sample<ZdServices.robot.Calculator_Reply>>() {
                    @Override
                    public void onComplete(
                            java.util.concurrent.Future<Sample<ZdServices.robot.Calculator_Reply>> future) {
                        notified[0] = true;
                    }
                });
        check(notified[0], "the completion listener fired on a ready future");
        try {
            check(pending.get().getData().data.add.result.return_ == 15, "7 + 8 through a future");
        } catch (Exception failure) {
            check(false, "the future resolved: " + failure);
        }

        // §7.5.2 and §7.11.2.1: a reply carrying a remote exception code
        // raises the class the table names, at the call site that asks.
        ZdServices.robot.CalculatorReplier raw =
                new ZdServices.robot.CalculatorReplier(replierParams(participant, "Calculator"));
        SampleIdentity refusedId = requester.sendRequest(addRequest(0, 0));
        check(raw.receiveRequests().size() == 1, "the raw replier polled one request");
        raw.sendReply(refusalReply(), refusedId, RemoteExceptionCode.REMOTE_EX_UNKNOWN_OPERATION);
        boolean raised = false;
        try {
            requester.receiveReply(refusedId);
        } catch (RemoteUnknownOperationError expected) {
            raised = true;
        }
        check(raised, "REMOTE_EX_UNKNOWN_OPERATION raises RemoteUnknownOperationError");

        // §7.10.2: the mutable policies change at run time, the immutable
        // ones do not.
        requester.setQos(
                RpcQos.defaults().withDeadlineNanos(5_000_000_000L), RpcQos.defaults());
        check(((LoopbackWriter) requestWriter.entity).qos.deadlineNanos() == 5_000_000_000L,
                "setQos reached the request writer");
        boolean immutable = false;
        try {
            requester.setQos(
                    RpcQos.defaults().withReliability(RpcQos.Reliability.BEST_EFFORT),
                    RpcQos.defaults());
        } catch (IllegalArgumentException expected) {
            immutable = true;
        }
        check(immutable, "RELIABILITY is not mutable");
        service.replier.setQos(RpcQos.defaults(), RpcQos.defaults().withDeadlineNanos(1_000_000_000L));
        check(((LoopbackWriter) participant.createdOn(REP, true).entity).qos.deadlineNanos()
                == 1_000_000_000L,
                "setQos reached the reply writer");

        // §7.11.1.5.2: ServiceEndpoint's pause and resume.
        service.pause();
        requester.sendRequest(addRequest(1, 2));
        check(server.runOnce() == 0, "a paused service dispatches nothing");
        service.resume();
        check(server.runOnce() == 1, "a resumed one does");

        // §7.11.2.3: the generated Client is a ClientEndpoint.
        check(
                ClientEndpoint.class.isAssignableFrom(ZdServices.robot.CalculatorClient.class),
                "the generated Client extends ClientEndpoint");

        int deletedBefore = participant.deletedWriters + participant.deletedReaders;
        requester.close();
        check(requester.isNull(), "a closed requester is null");
        // §7.9.3: deactivation deletes the underlying DataWriter and
        // DataReader — and only those two entities.
        check(participant.deletedWriters + participant.deletedReaders == deletedBefore + 2,
                "close() deleted the requester's DataWriter and DataReader");
        int deletedAfterRequester = participant.deletedWriters + participant.deletedReaders;
        server.close();
        check(participant.deletedWriters + participant.deletedReaders == deletedAfterRequester + 2,
                "closing the service deleted its replier's DataWriter and DataReader");
        check(service.isNull(), "a closed service is null");
    }

    // =====================================================================
    // §7.5.1.1: classic CDR by default, XCDR2 by a named setting
    // =====================================================================

    /** Remembers the offset and the encoding the runtime serialises with. */
    static final class OffsetProbe implements TypeSupport<Integer> {
        int offset = -1;
        WireEncoding encoding = null;
        int readFrom = -1;
        WireEncoding readAs = null;

        @Override
        public byte[] serialize(Integer value) {
            return INTS.serialize(value);
        }

        @Override
        public byte[] serialize(Integer value, int at, WireEncoding as) {
            offset = at;
            encoding = as;
            return INTS.serialize(value);
        }

        @Override
        public Integer deserialize(byte[] octets, HeaderCodec.Cursor cursor) {
            return INTS.deserialize(octets, cursor);
        }

        @Override
        public Integer deserialize(byte[] octets, HeaderCodec.Cursor cursor, WireEncoding as) {
            readFrom = cursor.at;
            readAs = as;
            return INTS.deserialize(octets, cursor);
        }
    }

    static void wireEncoding() {
        for (WireEncoding encoding : WireEncoding.values()) {
            Bus bus = new Bus();
            LoopbackParticipant participant = new LoopbackParticipant(bus);
            OffsetProbe request = new OffsetProbe();
            OffsetProbe reply = new OffsetProbe();
            ReplierParams serving = enhancedReplier(participant, "Calculator");
            serving.mapping(ServiceMapping.BASIC)
                    .requestTypeSupport(request)
                    .replyTypeSupport(reply);
            RequesterParams asking = enhancedRequester(participant);
            asking.mapping(ServiceMapping.BASIC)
                    .requestTypeSupport(request)
                    .replyTypeSupport(reply);
            if (encoding == WireEncoding.XCDR2) {
                serving.wireEncoding(encoding);
                asking.wireEncoding(encoding);
            }
            Replier<Integer, Integer> replier = new ReplierImpl<>(serving);
            Requester<Integer, Integer> requester = new RequesterImpl<>(asking);
            requester.bind("Calculator");
            SampleIdentity id = SampleIdentity.of(requester.sendRequest(7));
            String tag = " (" + encoding + ")";
            // The header of "Calculator" is 39 octets; the payload starts
            // at the next multiple of four, and the codec is told where.
            check(request.offset == 40 && request.encoding == encoding,
                    "the request codec gets offset 40 and " + encoding);
            List<Sample<Integer>> got = replier.receiveRequests();
            check(got.size() == 1 && request.readFrom == 40 && request.readAs == encoding,
                    "the request is read from offset 40 as " + encoding);
            replier.sendReply(14, got.get(0).identity());
            check(reply.offset == 28 && reply.encoding == encoding,
                    "the reply codec gets offset 28 and " + encoding);
            check(requester.receiveReply(id) != null && reply.readFrom == 28
                    && reply.readAs == encoding, "the reply is read from offset 28" + tag);
            // DATA_REPRESENTATION follows the encoding.
            check(participant.createdOn(EREQ, true).qos.dataRepresentation() == encoding,
                    "the writers offer " + encoding);
        }
        check(RequesterParams.create().wireEncoding() == WireEncoding.XCDR1,
                "classic CDR is the default");
        check(RpcQos.defaults().dataRepresentation() == WireEncoding.XCDR1,
                "and the default data representation");
    }

    // =====================================================================
    // §7.8.2: a reply carries PID_RELATED_SAMPLE_IDENTITY
    // =====================================================================

    static void relatedIdentityPolicy() {
        Bus bus = new Bus();
        bus.carryRelated = false;
        LoopbackParticipant participant = new LoopbackParticipant(bus);
        ZdServices.robot.CalculatorRequester requester =
                new ZdServices.robot.CalculatorRequester(requesterParams(participant));
        ZdServices.robot.CalculatorReplier strict =
                new ZdServices.robot.CalculatorReplier(replierParams(participant, ""));

        SampleIdentity id = requester.sendRequest(addRequest(1, 2));
        check(strict.receiveRequests().size() == 1, "the strict replier polled the request");
        boolean unsupported = false;
        try {
            strict.sendReply(refusalReply(), id);
        } catch (RemoteUnsupportedError expected) {
            unsupported = true;
        }
        check(unsupported, "a binding without inline QoS fails a reply that must carry the PID");

        // The other reading, behind a named setting: the header of the
        // Basic mapping still correlates.
        ZdServices.robot.CalculatorReplier lenient =
                new ZdServices.robot.CalculatorReplier(
                        replierParams(participant, "").relatedIdentity(RelatedIdentity.IF_SUPPORTED));
        SampleIdentity id2 = requester.sendRequest(addRequest(3, 4));
        List<Sample<ZdServices.robot.Calculator_Request>> got = lenient.receiveRequests();
        check(got.size() == 1, "the lenient replier polled the request");
        ZdServices.robot.Calculator_Reply answer = refusalReply();
        answer.data._d = OP_ADD;
        answer.data.add.result.return_ = 7;
        lenient.sendReply(answer, got.get(0).identity());
        Sample<ZdServices.robot.Calculator_Reply> back = requester.receiveReply(id2);
        check(back != null && back.getData().data.add.result.return_ == 7,
                "the header correlates the reply without the PID");
    }

    // =====================================================================
    // §7.6.2.2: requests wait for the service to be matched
    // =====================================================================

    static void matching() {
        Bus bus = new Bus();
        LoopbackParticipant participant = new LoopbackParticipant(bus);
        ZdServices.robot.CalculatorRequester requester =
                new ZdServices.robot.CalculatorRequester(
                        requesterParams(participant).matching(ServiceMatching.UNTIL_MATCHED));
        SampleIdentity id = requester.sendRequest(addRequest(1, 1));
        check(bus.topic(REQ).samples.isEmpty(), "a request waits while no service is matched");
        check(!requester.isServiceMatched(), "no service is matched");
        check(bus.topic(REQ).samples.isEmpty(), "and is still not written");

        CalculatorService service = new CalculatorService(participant, "");
        check(requester.isServiceMatched(), "the service is matched");
        check(bus.topic(REQ).samples.size() == 1, "the queued request was written once matched");
        Server server = Server.create();
        server.add(service);
        check(server.runOnce() == 1, "the service answered it");
        Sample<ZdServices.robot.Calculator_Reply> reply = requester.receiveReply(id);
        check(reply != null && reply.getData().data.add.result.return_ == 2,
                "the identity sendRequest returned is the one the reply carries");

        // A request still waiting when the requester closes is reported.
        Bus lonely = new Bus();
        LoopbackParticipant alone = new LoopbackParticipant(lonely);
        ZdServices.robot.CalculatorRequester waiting =
                new ZdServices.robot.CalculatorRequester(
                        requesterParams(alone).matching(ServiceMatching.UNTIL_MATCHED));
        Future<Sample<ZdServices.robot.Calculator_Reply>> future =
                waiting.sendRequestAsync(addRequest(1, 2));
        waiting.close();
        boolean failed = false;
        try {
            future.get();
        } catch (ExecutionException expected) {
            failed = expected.getCause() instanceof RemoteOutOfResourcesError;
        } catch (InterruptedException interrupted) {
            check(false, "interrupted");
        }
        check(failed, "a request dropped at close() fails its future");

        // The other reading, behind a named setting: no waiting in Basic.
        Bus immediate = new Bus();
        LoopbackParticipant fast = new LoopbackParticipant(immediate);
        ZdServices.robot.CalculatorRequester sends =
                new ZdServices.robot.CalculatorRequester(requesterParams(fast));
        sends.sendRequest(addRequest(1, 1));
        check(immediate.topic(REQ).samples.size() == 1,
                "the Basic mapping writes at once by default (§7.6.1)");

        // §7.6.2.2, the service side: requests wait while the service's
        // writer has not matched the client's reader.
        Bus gate = new Bus();
        LoopbackParticipant gated = new LoopbackParticipant(gate);
        ZdServices.robot.CalculatorReplier replier =
                new ZdServices.robot.CalculatorReplier(
                        replierParams(gated, "").matching(ServiceMatching.UNTIL_MATCHED));
        DataWriter client = gated.createWriter(REQ, "robot::Calculator_Request", RpcQos.defaults(), "");
        client.write(basicRequestBytes(client.nextIdentity(), "", addRequest(2, 3)));
        check(replier.receiveRequests().isEmpty(), "the replier holds the request: no client reader yet");
        // The replier keeps the request it has taken off the reader and has not yet
        // answered (Figure 7.3): the reader has nothing left, and the request is not lost.
        check(gate.topic(REQ).samples.isEmpty(), "the reader gave the request to the replier");
        check(replier.receiveRequests().isEmpty(), "which holds it as long as the client is unmatched");
        gated.createReader(REP, "robot::Calculator_Reply", RpcQos.defaults(), "");
        check(replier.receiveRequests().size() == 1, "once the client's reader is matched it is taken");
    }

    // =====================================================================
    // §7.10: QoS
    // =====================================================================

    static void qos() throws Exception {
        File file = File.createTempFile("rpc-qos", ".xml");
        try {
            String xml =
                    "<dds><qos_library name=\"RpcLib\"><qos_profile name=\"Calc\">"
                            + "<datawriter_qos>"
                            + "<history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>32</depth></history>"
                            + "<reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>"
                            + "</datawriter_qos>"
                            + "<datareader_qos>"
                            + "<durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>"
                            + "</datareader_qos>"
                            + "</qos_profile></qos_library>"
                            + "<qos_library name=\"Other\"><qos_profile name=\"P\"/></qos_library></dds>";
            Files.write(file.toPath(), xml.getBytes("UTF-8"));
            String path = file.getAbsolutePath();

            // §7.10.1: "file://path#library", the protocol optional.
            for (String url : new String[] {path + "#RpcLib", "file://" + path + "#RpcLib"}) {
                RpcQos qos = RpcQos.fromUrl(url);
                check(qos.historyKind() == RpcQos.HistoryKind.KEEP_LAST && qos.historyDepth() == 32,
                        "the history of the profile: " + url);
                check(qos.reliability() == RpcQos.Reliability.BEST_EFFORT,
                        "the reliability of the profile: " + url);
                check(qos.durability() == RpcQos.Durability.TRANSIENT_LOCAL,
                        "the durability of the reader profile: " + url);
            }
            check(RpcQos.fromUrl(path + "#Other").equals(RpcQos.defaults()),
                    "a profile without policies is the defaults");
            boolean refused = false;
            try {
                RpcQos.fromUrl("http://host/qos.xml");
            } catch (IllegalArgumentException expected) {
                refused = true;
            }
            check(refused, "only file:// is supported");
            refused = false;
            try {
                RpcQos.fromUrl(path + "#Missing");
            } catch (IllegalArgumentException expected) {
                refused = true;
            }
            check(refused, "a missing library is an error, not the defaults");

            // The profiles reach the endpoints of the right topic.
            Bus bus = new Bus();
            LoopbackParticipant participant = new LoopbackParticipant(bus);
            ZdServices.robot.CalculatorRequester requester =
                    new ZdServices.robot.CalculatorRequester(
                            requesterParams(participant).qosProfiles(path + "#RpcLib", null));
            check(participant.createdOn(REQ, true).qos.historyDepth() == 32,
                    "RequestProfile is the QoS of the request topic");
            check(participant.createdOn(REP, false).qos.equals(RpcQos.defaults()),
                    "an absent ReplyProfile leaves the reply topic at the defaults");
            requester.close();
        } finally {
            file.delete();
        }
    }

    // =====================================================================
    // §7.9.2.2, §7.11.1.4.7 to .10: listeners are called when a sample arrives
    // =====================================================================

    static void arrival() {
        Bus bus = new Bus();
        bus.notify = true;
        LoopbackParticipant participant = new LoopbackParticipant(bus);
        CollectAll listener = new CollectAll();
        CalculatorService service = new CalculatorService(participant, "");
        ZdServices.robot.CalculatorRequester requester =
                new ZdServices.robot.CalculatorRequester(requesterParams(participant).listener(listener));

        // No pump: the write itself reaches the replier's listener, whose
        // reply reaches the requester's.
        requester.sendRequest(addRequest(5, 6));
        check(service.answered() == 1, "the replier listener ran on arrival, without a pump");
        check(listener.sums.size() == 1 && listener.sums.get(0) == 11,
                "the requester listener ran on arrival, without a pump");

        Future<Sample<ZdServices.robot.Calculator_Reply>> future =
                requester.sendRequestAsync(addRequest(7, 8));
        check(future.isDone(), "a future whose reply arrives from inside the write is ready");
        try {
            check(future.get().getData().data.add.result.return_ == 15, "7 + 8");
        } catch (Exception failure) {
            check(false, "the future resolved: " + failure);
        }
        service.close();

        // A failure reaches the error listener, not the thread that wrote.
        ZdServices.robot.CalculatorReplier raw =
                new ZdServices.robot.CalculatorReplier(replierParams(participant, ""));
        SampleIdentity id = requester.sendRequest(addRequest(0, 0));
        List<Sample<ZdServices.robot.Calculator_Request>> got = raw.receiveRequests();
        check(got.size() == 1, "the raw replier took the request");
        raw.sendReply(refusalReply(), id, RemoteExceptionCode.REMOTE_EX_UNKNOWN_OPERATION);
        check(listener.failed.size() == 1
                && listener.failed.get(0) instanceof RemoteUnknownOperationError,
                "a remote exception reaches the error listener");

        Created reader = participant.createdOn(REP, false);
        requester.close();
        check(((LoopbackReader) reader.entity).listener == null,
                "close() removes the arrival listener");
    }

    // =====================================================================
    // §7.5.1.2 and §7.8.2: the Enhanced mapping
    // =====================================================================

    static void enhanced() {
        Bus bus = new Bus();
        bus.reportIdentity = true;
        LoopbackParticipant participant = new LoopbackParticipant(bus);
        Replier<Integer, Integer> replier = new ReplierImpl<>(enhancedReplier(participant, "inst-1"));
        Requester<Integer, Integer> requester = new RequesterImpl<>(enhancedRequester(participant));

        SampleIdentity id = SampleIdentity.of(requester.sendRequest(21));
        check(bus.lastSample.get(EREQ).length == 4, "an Enhanced request has no header");
        List<Sample<Integer>> requests = replier.receiveRequests();
        check(requests.size() == 1 && requests.get(0).getData() == 21, "the replier took the request");
        check(requests.get(0).identity().equals(id),
                "§7.8.2.1: the identity of the request is the identity of its sample");
        replier.sendReply(requests.get(0).getData() * 2, requests.get(0).identity());
        check(bus.lastSample.get(EREP).length == 4, "an Enhanced reply has no header");
        check(id.equals(bus.lastRelated.get(EREP)), "the reply carries the request's identity");
        Sample<Integer> reply = requester.receiveReply(id);
        check(reply != null && reply.getData() == 42, "the reply is correlated by PID_RELATED_SAMPLE_IDENTITY");
        check(reply != null && reply.relatedIdentity().equals(id), "§7.8.2.2: the related identity");

        // §7.5.1.2.5 rule 5: a remote exception travels in `remoteEx`.
        SampleIdentity failing = SampleIdentity.of(requester.sendRequest(1));
        replier.receiveRequests();
        replier.sendReply(0, failing, RemoteExceptionCode.REMOTE_EX_UNSUPPORTED);
        boolean raised = false;
        try {
            requester.receiveReply(failing);
        } catch (RemoteUnsupportedError expected) {
            raised = true;
        }
        check(raised, "remoteEx raises at the call site");

        // §7.2.1: a reply to another client is filtered, and a reply with
        // no related identity cannot be correlated and is an error.
        DataWriter stranger = participant.createWriter(EREP, "Doubler_Reply", RpcQos.defaults(), "");
        byte[] foreignGuid = new byte[16];
        foreignGuid[0] = (byte) 0xEE;
        stranger.write(INTS.serialize(5), new SampleIdentity(foreignGuid, 9));
        check(requester.receiveReplies().isEmpty(), "a reply to another client is filtered out");
        stranger.write(INTS.serialize(5));
        boolean uncorrelated = false;
        try {
            requester.receiveReplies();
        } catch (RemoteInvalidArgumentError expected) {
            uncorrelated = true;
        }
        check(uncorrelated, "a reply without PID_RELATED_SAMPLE_IDENTITY is refused");

        // The Enhanced mapping has no header to fall back on.
        boolean illegal = false;
        try {
            new ReplierImpl<>(
                    enhancedReplier(participant, "").relatedIdentity(RelatedIdentity.IF_SUPPORTED));
        } catch (IllegalArgumentException expected) {
            illegal = true;
        }
        check(illegal, "IF_SUPPORTED is refused where the reply has no header");

        // §7.4.1.2.2/3 service instance: a bound requester writes to the
        // instance it is bound to only, and waits for it to be matched.
        requester.bind("inst-1");
        SampleIdentity bound = SampleIdentity.of(requester.sendRequest(4));
        check(bus.topic(EREQ).samples.size() == 1, "the request reached the bound instance");
        replier.receiveRequests();
        requester.bind("other-instance");
        int before = bus.topic(EREQ).samples.size();
        requester.sendRequest(5);
        check(bus.topic(EREQ).samples.size() == before, "no request is written to an unmatched instance");
        check(!requester.isServiceMatched(), "the bound instance is not matched");
        requester.unbind();
        check(bound != null, "the bound send returned an identity");

        // §7.6.2.2: queued until matched.
        Bus late = new Bus();
        late.reportIdentity = true;
        LoopbackParticipant lateParticipant = new LoopbackParticipant(late);
        Requester<Integer, Integer> early = new RequesterImpl<>(enhancedRequester(lateParticipant));
        SampleIdentity queued = SampleIdentity.of(early.sendRequest(7));
        check(late.topic(EREQ).samples.isEmpty(), "the Enhanced mapping queues until the service is matched");
        Replier<Integer, Integer> appears = new ReplierImpl<>(enhancedReplier(lateParticipant, ""));
        check(early.isServiceMatched(), "matched");
        List<Sample<Integer>> arrived = appears.receiveRequests();
        check(arrived.size() == 1 && arrived.get(0).identity().equals(queued),
                "the queued request went out under the identity sendRequest returned");
        early.close();
        appears.close();
        requester.close();
        replier.close();
    }

    // =====================================================================
    // §7.11.2 over the DDS Java PSM of crates/java-omgdds
    // =====================================================================

    static void overTheJavaPsm() throws Exception {
        check("robot::Calculator_Request".equals(new RawTypeSupport("robot::Calculator_Request").getTypeName()),
                "the PSM type support carries the interface's type name");
        org.omg.dds.domain.DomainParticipant psm = org.omg.dds.domain.DomainParticipantFactory.getInstance(
                org.omg.dds.core.ServiceEnvironment.createInstance(
                        RpcRuntimeRoundtrip.class.getClassLoader())).createParticipant(77);
        ZeroDdsParticipant bound = new ZeroDdsParticipant(psm);

        CollectReplies listener = new CollectReplies();
        AddReplier adder = new AddReplier();
        ReplierParams serving = replierParams(bound, "").listener(adder);
        Replier<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> replier =
                new ReplierImpl<>(serving);
        ZdServices.robot.CalculatorRequester requester =
                new ZdServices.robot.CalculatorRequester(requesterParams(bound).listener(listener));
        check(requester.isServiceMatched(), "the PSM entities matched");

        // The write reaches the replier's listener, and its reply the
        // requester's, on arrival.
        requester.sendRequest(addRequest(20, 22));
        for (int wait = 0; wait < 200 && listener.sums.isEmpty(); ++wait) {
            Thread.sleep(10);
        }
        check(adder.calls == 1, "the replier listener ran on arrival over the PSM");
        check(listener.sums.size() == 1 && listener.sums.get(0) == 42, "20 + 22 over the PSM");

        // §7.8.2: the PSM binding sends PID_RELATED_SAMPLE_IDENTITY in the inline QoS and
        // reads it back, so the default policy (REQUIRED) holds and the reply is correlated
        // by it.
        ReplierParams strict = replierParams(bound, "").requestTopicName("PsmStrict_Request")
                        .replyTopicName("PsmStrict_Reply");
        Replier<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> polled =
                new ReplierImpl<>(strict);
        RequesterParams strictRequester = requesterParams(bound).requestTopicName("PsmStrict_Request")
                        .replyTopicName("PsmStrict_Reply");
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> asks =
                new RequesterImpl<>(strictRequester);
        SampleIdentity asked = SampleIdentity.of(asks.sendRequest(addRequest(1, 1)));
        List<Sample<ZdServices.robot.Calculator_Request>> got = polled.receiveRequests();
        check(got.size() == 1, "the polled replier took the request over the PSM");
        check(got.get(0).identity().equals(asked),
                "the request arrived under the identity sendRequest returned over the PSM");
        polled.sendReply(refusalReply(), got.get(0).identity());
        Sample<ZdServices.robot.Calculator_Reply> related = asks.receiveReply(asked);
        check(related != null && related.relatedIdentity().equals(asked),
                "the reply carries PID_RELATED_SAMPLE_IDENTITY over the PSM");

        // §7.10.2: DEADLINE and LIFESPAN change at run time, and the PSM entity
        // applies the change (DDS 1.4 §2.2.3); an immutable policy does not.
        requester.setQos(RpcQos.defaults().withDeadlineNanos(1_000_000_000L), RpcQos.defaults());
        boolean refused = false;
        try {
            requester.setQos(
                    RpcQos.defaults().withReliability(RpcQos.Reliability.BEST_EFFORT),
                    RpcQos.defaults());
        } catch (IllegalArgumentException expected) {
            refused = true;
        }
        check(refused, "RELIABILITY does not change after enable");

        // §7.6.2.1.2: the service-side endpoints announce the instance they serve
        // (PID 0x0080); the client lists the instances its request writer matched.
        Replier<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> named =
                new ReplierImpl<>(replierParams(bound, "psm-A").requestTopicName("PsmNamed_Request")
                        .replyTopicName("PsmNamed_Reply"));
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> finds =
                new RequesterImpl<>(requesterParams(bound).requestTopicName("PsmNamed_Request")
                        .replyTopicName("PsmNamed_Reply"));
        check(finds.getDiscoveredServiceInstances().contains("psm-A"),
                "the instance the replier announced is discovered over the PSM");
        finds.waitForService("psm-A");
        check(!finds.getDiscoveredServiceInstances().contains(""),
                "and the unnamed one is no instance");
        named.close();
        finds.close();

        // §7.6.2.2.1 and §7.6.2.2.2, Figures 7.3 and 7.4: the requests wait for the matching
        // that the announced related endpoints describe. The requester is made first: its
        // request is queued until a service is discovered, the service holds back requests
        // of a client whose reply reader it has not matched, and then the request arrives
        // under the identity sendRequest returned.
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> gated =
                new RequesterImpl<>(requesterParams(bound).requestTopicName("PsmGated_Request")
                        .replyTopicName("PsmGated_Reply").matching(ServiceMatching.UNTIL_MATCHED));
        SampleIdentity early = SampleIdentity.of(gated.sendRequest(addRequest(3, 4)));
        check(!gated.isServiceMatched(), "no service yet: the request is queued");
        Replier<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> gatedReplier =
                new ReplierImpl<>(replierParams(bound, "").requestTopicName("PsmGated_Request")
                        .replyTopicName("PsmGated_Reply").matching(ServiceMatching.UNTIL_MATCHED));
        List<Sample<ZdServices.robot.Calculator_Request>> arrived = new java.util.ArrayList<>();
        for (int wait = 0; wait < 500 && arrived.isEmpty(); ++wait) {
            gated.isServiceMatched();
            arrived.addAll(gatedReplier.receiveRequests());
            Thread.sleep(10);
        }
        check(arrived.size() == 1 && arrived.get(0).identity().equals(early),
                "the queued request reached the service after the matching, under its identity");
        check(gated.getDiscoveredServiceInstances().isEmpty()
                        && gated.isServiceMatched(),
                "and the unnamed service is matched");
        gated.close();
        gatedReplier.close();

        requester.close();
        replier.close();
        polled.close();
        asks.close();
    }


    // =====================================================================
    // §7.11.2: the params in the PSM's types and the PSM's entities
    // =====================================================================

    /** A reply of the generated codec to {@code add} that returns {@code sum}. */
    static ZdServices.robot.Calculator_Reply sumReply(int sum) {
        ZdServices.robot.Calculator_Reply reply = refusalReply();
        reply.data._d = OP_ADD;
        reply.data.add._d = 0;
        reply.data.add.result.return_ = sum;
        return reply;
    }

    /** Takes from {@code reader} until it gave a sample or two seconds passed. */
    static <T> List<org.omg.dds.sub.Sample<T>> takeWithin(org.omg.dds.sub.DataReader<T> reader)
            throws InterruptedException {
        List<org.omg.dds.sub.Sample<T>> taken = new ArrayList<>();
        for (int wait = 0; wait < 200 && taken.isEmpty(); ++wait) {
            reader.take(taken);
            if (taken.isEmpty()) {
                Thread.sleep(10);
            }
        }
        return taken;
    }

    /** Receives from {@code replier} until it gave a request or two seconds passed. */
    static <TReq> List<Sample<TReq>> requestsWithin(Replier<TReq, ?> replier)
            throws InterruptedException {
        List<Sample<TReq>> got = new ArrayList<>();
        for (int wait = 0; wait < 200 && got.isEmpty(); ++wait) {
            got.addAll(replier.receiveRequests());
            if (got.isEmpty()) {
                Thread.sleep(10);
            }
        }
        return got;
    }

    static void psmEntities() throws Exception {
        org.omg.dds.domain.DomainParticipantFactory factory =
                org.omg.dds.domain.DomainParticipantFactory.getInstance(
                        org.omg.dds.core.ServiceEnvironment.createInstance(
                                RpcRuntimeRoundtrip.class.getClassLoader()));
        org.omg.dds.domain.DomainParticipant psm = factory.createParticipant(78);
        org.omg.dds.core.policy.PolicyFactory pf = psm.getEnvironment().getSPI().getPolicyFactory();
        org.omg.dds.pub.Publisher publisher = psm.createPublisher();
        org.omg.dds.sub.Subscriber subscriber = psm.createSubscriber();
        org.omg.dds.pub.DataWriterQos writerQos = publisher.getDefaultDataWriterQos()
                .withPolicy(pf.Reliability().withReliable())
                .withPolicy(pf.History().withKeepAll())
                .withPolicy(pf.Lifespan().withDuration(7, java.util.concurrent.TimeUnit.SECONDS));
        org.omg.dds.sub.DataReaderQos readerQos = subscriber.getDefaultDataReaderQos()
                .withPolicy(pf.Reliability().withReliable())
                .withPolicy(pf.History().withKeepLast(11));

        // The way of the machine-readable files: the runtime singleton, params in the PSM's
        // types, and the requester and replier made from the classes of the topic types.
        RPCRuntime runtime = RPCRuntime.getInstance(RpcRuntimeRoundtrip.class.getClassLoader());
        RequesterParams asking = runtime.createRequesterParams()
                .withDomainParticipant(psm)
                .withPublisher(publisher)
                .withSubscriber(subscriber)
                .withDataWriterQos(writerQos)
                .withDataReaderQos(readerQos);
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> requester =
                runtime.createRequester(ZdServices.robot.Calculator_Request.class,
                        ZdServices.robot.Calculator_Reply.class, asking);
        Replier<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> replier =
                runtime.createReplier(ZdServices.robot.Calculator_Request.class,
                        ZdServices.robot.Calculator_Reply.class,
                        runtime.createReplierParams().withDomainParticipant(psm));
        check(requester.getRequesterParams().getDomainParticipant() == psm
                        && requester.getRequesterParams().getDataWriterQos() == writerQos
                        && requester.getRequesterParams().getPublisher() == publisher,
                "the params keep the PSM participant, Publisher and QoS they were given");
        requester.waitForService(runtime.getSPI().newDuration(5, java.util.concurrent.TimeUnit.SECONDS));

        // The four entities are the PSM's, typed by the topic types, in the Publisher and the
        // Subscriber and with the QoS the params named.
        org.omg.dds.pub.DataWriter<ZdServices.robot.Calculator_Request> requests =
                requester.getRequestDataWriter();
        org.omg.dds.sub.DataReader<ZdServices.robot.Calculator_Reply> replies =
                requester.getReplyDataReader();
        check(requests != null && replies != null, "the requester hands out its PSM entities");
        check(requests.getParent() == publisher && replies.getParent() == subscriber,
                "the request writer is in the Publisher, the reply reader in the Subscriber the params named");
        check(requests.getQos().getLifespan().getDuration()
                                .getDuration(java.util.concurrent.TimeUnit.SECONDS) == 7
                        && replies.getQos().getHistory().getKind()
                                == org.omg.dds.core.policy.History.Kind.KEEP_LAST
                        && replies.getQos().getHistory().getDepth() == 11,
                "the request writer has the DataWriterQos, the reply reader the DataReaderQos the params named");
        check(requests.getTopic().getTypeSupport().getType() == ZdServices.robot.Calculator_Request.class
                        && requests.getTopic().getTypeName()
                                .equals(ZdServices.robot.CalculatorSupport.REQUEST_TYPE_NAME),
                "the request topic is typed by the request class, under the type name of the interface");
        check(replier.getRequestDataReader().getTopicDescription().getTypeSupport().getType()
                        == ZdServices.robot.Calculator_Request.class
                        && replier.getReplyDataWriter().getTopic().getTypeSupport().getType()
                                == ZdServices.robot.Calculator_Reply.class,
                "the replier hands out its request reader and reply writer, typed");

        // The runtime keeps sending and reading the octets it frames through the same entities.
        SampleIdentity sent = SampleIdentity.of(requester.sendRequest(addRequest(2, 3)));
        List<Sample<ZdServices.robot.Calculator_Request>> got = requestsWithin(replier);
        check(got.size() == 1 && got.get(0).identity().equals(sent) && got.get(0).getData().data.add.b == 3,
                "a request of the requester reaches the replier through the typed entities");
        replier.sendReply(sumReply(5), sent);
        Sample<ZdServices.robot.Calculator_Reply> answer = null;
        for (int wait = 0; wait < 200 && answer == null; ++wait) {
            answer = requester.receiveReply(sent);
            if (answer == null) {
                Thread.sleep(10);
            }
        }
        check(answer != null && answer.getData().data.add.result.return_ == 5,
                "and its reply reaches the requester");

        // A request written through getRequestDataWriter() is the topic's sample as it stands,
        // header and all: the replier answers it under the identity its header names.
        SampleIdentity named = new SampleIdentity(new byte[] {
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0, 0, 1, 3}, 77);
        ZdServices.robot.Calculator_Request byHand = addRequest(4, 5);
        byHand.header = new RequestHeader(named, "");
        requests.write(byHand);
        got = requestsWithin(replier);
        check(got.size() == 1 && got.get(0).identity().equals(named) && got.get(0).getData().data.add.a == 4,
                "a request written through getRequestDataWriter() reaches the replier with its header");

        // A reply written through getReplyDataWriter() and read through getReplyDataReader():
        // the value with the header it was written with.
        ZdServices.robot.Calculator_Reply byHandReply = sumReply(9);
        byHandReply.header = new ReplyHeader(named, RemoteExceptionCode.REMOTE_EX_OK);
        replier.getReplyDataWriter().write(byHandReply);
        List<org.omg.dds.sub.Sample<ZdServices.robot.Calculator_Reply>> read = takeWithin(replies);
        check(read.size() == 1 && read.get(0).getData().data.add.result.return_ == 9
                        && read.get(0).getData().header.relatedRequestId().equals(named),
                "a reply read through getReplyDataReader() has its value and its header");

        // A request taken through getRequestDataReader() has the header the requester stamped.
        SampleIdentity third = SampleIdentity.of(requester.sendRequest(addRequest(7, 8)));
        List<org.omg.dds.sub.Sample<ZdServices.robot.Calculator_Request>> taken =
                takeWithin(replier.getRequestDataReader());
        check(taken.size() == 1 && taken.get(0).getData().header.requestId().equals(third)
                        && taken.get(0).getData().data.add.a == 7,
                "a request taken through getRequestDataReader() carries the identity sendRequest returned");
        check(replier.receiveRequests().isEmpty(), "and it was taken from the replier's reader");

        // DDS-Java-PSM 1.0 §7.5.1: the type support the PSM makes from the class of a topic
        // type frames the samples as the requester does.
        org.omg.dds.type.TypeSupport<ZdServices.robot.Calculator_Request> fromClass =
                org.omg.dds.type.TypeSupport.newTypeSupport(ZdServices.robot.Calculator_Request.class,
                        ZdServices.robot.CalculatorSupport.REQUEST_TYPE_NAME, psm.getEnvironment());
        java.nio.ByteBuffer encoded = java.nio.ByteBuffer.allocate(256)
                .order(java.nio.ByteOrder.LITTLE_ENDIAN);
        ((org.zerodds.cdr.TypeSupportImpl<ZdServices.robot.Calculator_Request>) fromClass)
                .encode(byHand, encoded);
        byte[] octets = java.util.Arrays.copyOf(encoded.array(), encoded.position());
        @SuppressWarnings("unchecked")
        TypeSupport<ZdServices.robot.Calculator_Request> generatedCodec =
                (TypeSupport<ZdServices.robot.Calculator_Request>)
                        requester.getRequesterParams().requestTypeSupport();
        org.zerodds.ddsrpc.TopicSample<ZdServices.robot.Calculator_Request> framing =
                org.zerodds.ddsrpc.TopicSample.request(generatedCodec,
                        ServiceMapping.BASIC, requester.getRequesterParams().wireEncoding());
        check(java.util.Arrays.equals(octets, framing.encode(byHand)),
                "the type support made from the class writes the octets the requester writes");
        org.omg.dds.topic.Topic<ZdServices.robot.Calculator_Request> spyTopic =
                psm.createTopic("SpyRequests", fromClass);
        org.omg.dds.sub.DataReader<ZdServices.robot.Calculator_Request> spy =
                psm.createSubscriber().createDataReader(spyTopic);
        psm.createPublisher().createDataWriter(spyTopic).write(byHand);
        List<org.omg.dds.sub.Sample<ZdServices.robot.Calculator_Request>> spied = takeWithin(spy);
        check(spied.size() == 1 && spied.get(0).getData().header.requestId().equals(named)
                        && spied.get(0).getData().data.add.b == 5,
                "and a topic made from it carries a request, header and all");

        // A binding that is not the PSM has no PSM entities, and refuses PSM ones it cannot use.
        LoopbackParticipant loopback = new LoopbackParticipant(new Bus());
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> plain =
                new RequesterImpl<>(requesterParams(loopback));
        check(plain.getRequestDataWriter() == null && plain.getReplyDataReader() == null,
                "a requester over a binding that is not the PSM hands out no PSM entities");
        boolean refusedPublisher = false;
        try {
            new RequesterImpl<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply>(
                    requesterParams(loopback).withPublisher(publisher));
        } catch (RemoteUnsupportedError expected) {
            refusedPublisher = true;
        }
        check(refusedPublisher, "a binding that is not the PSM refuses a Publisher");

        plain.close();
        requester.close();
        replier.close();
    }

    // =====================================================================
    // §7.11.2 receiveNondataSamples: the samples that carry no data
    // =====================================================================

    static void nondataSamples() throws Exception {
        org.omg.dds.domain.DomainParticipant psm = org.omg.dds.domain.DomainParticipantFactory.getInstance(
                org.omg.dds.core.ServiceEnvironment.createInstance(
                        RpcRuntimeRoundtrip.class.getClassLoader())).createParticipant(79);
        ZeroDdsParticipant bound = new ZeroDdsParticipant(psm);
        org.omg.dds.core.Duration fiveSeconds =
                psm.getEnvironment().getSPI().newDuration(5, java.util.concurrent.TimeUnit.SECONDS);

        // The requester: the reply writer of the replier goes away, which ends the instance of
        // the reply topic at each reply reader with a sample that carries no data (DDS 1.4
        // §2.2.2.5.1). The writer has the default WRITER_DATA_LIFECYCLE,
        // autodispose_unregistered_instances TRUE (§2.2.3.21): deleting it disposes the
        // instance before it unregisters it, so the instance is NOT_ALIVE_DISPOSED.
        Replier<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> replier =
                new ReplierImpl<>(replierParams(bound, "").requestTopicName("Nondata_Request")
                        .replyTopicName("Nondata_Reply"));
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> asking =
                new RequesterImpl<>(requesterParams(bound).requestTopicName("Nondata_Request")
                        .replyTopicName("Nondata_Reply"));
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> quiet =
                new RequesterImpl<>(requesterParams(bound).requestTopicName("Nondata_Request")
                        .replyTopicName("Nondata_Reply"));
        asking.waitForService(fiveSeconds);
        quiet.waitForService(fiveSeconds);
        check(!asking.receiveNondataSamples(true),
                "receiveNondataSamples is off by default and returns the setting it replaces");
        org.omg.dds.core.SampleIdentity first = asking.sendRequest(addRequest(1, 2));
        org.omg.dds.core.SampleIdentity second = quiet.sendRequest(addRequest(3, 4));
        List<Sample<ZdServices.robot.Calculator_Request>> requests = new ArrayList<>();
        for (int wait = 0; wait < 200 && requests.size() < 2; ++wait) {
            requests.addAll(replier.receiveRequests());
            Thread.sleep(10);
        }
        for (Sample<ZdServices.robot.Calculator_Request> request : requests) {
            replier.sendReply(refusalReply(), request.identity());
        }
        Sample<ZdServices.robot.Calculator_Reply> answer = null;
        Sample<ZdServices.robot.Calculator_Reply> quietAnswer = null;
        for (int wait = 0; wait < 200 && (answer == null || quietAnswer == null); ++wait) {
            answer = answer != null ? answer : asking.receiveReply(first);
            quietAnswer = quietAnswer != null ? quietAnswer : quiet.receiveReply(second);
            Thread.sleep(10);
        }
        check(requests.size() == 2 && answer != null && quietAnswer != null,
                "both requests were answered before the replier goes away");
        replier.close();
        Sample<ZdServices.robot.Calculator_Reply> notice = null;
        for (int wait = 0; wait < 200 && notice == null; ++wait) {
            notice = asking.takeReply();
            if (notice == null) {
                Thread.sleep(10);
            }
        }
        check(notice != null && !notice.validData() && notice.getData() == null
                        && notice.getInstanceState() == org.omg.dds.sub.InstanceState.NOT_ALIVE_DISPOSED,
                "with receiveNondataSamples(true) the requester hands out the sample without data the"
                        + " going away of the reply writer made");
        Thread.sleep(100);
        check(quiet.takeReply() == null && quiet.receiveReplies().isEmpty(),
                "without it the requester hands out no such sample");
        check(asking.receiveNondataSamples(false), "switching it off returns the setting it replaces");
        asking.close();
        quiet.close();

        // The replier: the request writer of a requester goes away, with the same lifecycle.
        Replier<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> listening =
                new ReplierImpl<>(replierParams(bound, "").requestTopicName("Nondata2_Request")
                        .replyTopicName("Nondata2_Reply"));
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> leaving =
                new RequesterImpl<>(requesterParams(bound).requestTopicName("Nondata2_Request")
                        .replyTopicName("Nondata2_Reply"));
        leaving.waitForService(fiveSeconds);
        check(!listening.receiveNondataSamples(true), "the replier's setting is off by default");
        leaving.sendRequest(addRequest(5, 6));
        check(requestsWithin(listening).size() == 1, "the request arrived");
        leaving.close();
        Sample<ZdServices.robot.Calculator_Request> gone = null;
        for (int wait = 0; wait < 200 && gone == null; ++wait) {
            org.omg.dds.sub.Sample.Iterator<ZdServices.robot.Calculator_Request> taken =
                    listening.takeRequests(10);
            if (taken.hasNext()) {
                gone = (Sample<ZdServices.robot.Calculator_Request>) taken.next();
            } else {
                Thread.sleep(10);
            }
        }
        check(gone != null && !gone.validData() && gone.getData() == null
                        && gone.getInstanceState() == org.omg.dds.sub.InstanceState.NOT_ALIVE_DISPOSED,
                "with receiveNondataSamples(true) the replier hands out the sample without data the"
                        + " going away of the request writer made");
        listening.close();
    }

    // =====================================================================
    // §7.5.1.1.1 and §7.8.2: the bounds and the constant of the headers
    // =====================================================================

    /** The request header of {@code instance} as the octets the codec writes. */
    static byte[] requestHeaderOf(String instance) {
        return HeaderCodec.encodeRequestHeader(new RequestHeader(SampleIdentity.UNKNOWN, instance));
    }

    static void headerCodec() {
        // §7.5.1.1.1: `typedef string<255> InstanceName`. 255 octets pass
        // and round-trip; the length field then is 256, the terminator
        // included.
        String longest = "a".repeat(255);
        byte[] ok = requestHeaderOf(longest);
        check(ok.length == 16 + 8 + 4 + 256, "a 255-octet instance name takes 16 + 8 + 4 + 256 octets");
        check(HeaderCodec.decodeRequestHeader(ok, new HeaderCodec.Cursor(0))
                        .instanceName().equals(longest),
                "a 255-octet instance name reads back");

        boolean encodeRefused = false;
        try {
            requestHeaderOf("a".repeat(256));
        } catch (IllegalArgumentException expected) {
            encodeRefused = true;
        }
        check(encodeRefused, "§7.5.1.1.1: a 256-octet instance name is refused on encode");

        // The bound counts octets, not characters: 128 two-octet characters
        // are 256 octets.
        boolean octetsCounted = false;
        try {
            requestHeaderOf("\u00e4".repeat(128));
        } catch (IllegalArgumentException expected) {
            octetsCounted = true;
        }
        check(octetsCounted, "the string<255> bound is counted in octets");

        // Decode: a length field of 257 says 256 octets before the terminator.
        byte[] over = Arrays.copyOf(ok, 16 + 8 + 4 + 257);
        over[24] = (byte) 0x01;
        over[25] = (byte) 0x01; // 0x0101 = 257
        boolean decodeRefused = false;
        try {
            HeaderCodec.decodeRequestHeader(over, new HeaderCodec.Cursor(0));
        } catch (RemoteInvalidArgumentError expected) {
            decodeRefused = true;
        }
        check(decodeRefused, "§7.5.1.1.1: a string<255> with a length field of 257 is refused on decode");

        // A length field above 2^31 is unsigned, not negative.
        byte[] huge = Arrays.copyOf(ok, ok.length);
        huge[24] = (byte) 0xFF;
        huge[25] = (byte) 0xFF;
        huge[26] = (byte) 0xFF;
        huge[27] = (byte) 0xFF;
        boolean hugeRefused = false;
        try {
            HeaderCodec.decodeRequestHeader(huge, new HeaderCodec.Cursor(0));
        } catch (RemoteInvalidArgumentError expected) {
            hugeRefused = true;
        }
        check(hugeRefused, "an unsigned length field above the bound is refused on decode");

        // A string without its terminator is not a CDR string.
        byte[] unterminated = requestHeaderOf("abc");
        unterminated[unterminated.length - 1] = (byte) 'x';
        boolean unterminatedRefused = false;
        try {
            HeaderCodec.decodeRequestHeader(unterminated, new HeaderCodec.Cursor(0));
        } catch (RemoteInvalidArgumentError expected) {
            unterminatedRefused = true;
        }
        check(unterminatedRefused, "a string without a terminator is refused on decode");

        // §7.8.2: RELATED_SAMPLE_IDENTITY_INVALID is { GUID_UNKNOWN,
        // SEQUENCENUMBER_UNKNOWN } and [RTPS] defines SEQUENCENUMBER_UNKNOWN
        // as { high = -1, low = 0 }.
        SampleIdentity invalid = SampleIdentity.RELATED_SAMPLE_IDENTITY_INVALID;
        check(Arrays.equals(invalid.writerGuid(), new byte[16]), "the invalid identity has GUID_UNKNOWN");
        check((int) (invalid.sequenceNumber() >>> 32) == -1, "SEQUENCENUMBER_UNKNOWN.high is -1");
        check((int) invalid.sequenceNumber() == 0, "SEQUENCENUMBER_UNKNOWN.low is 0");
        check(!invalid.equals(SampleIdentity.UNKNOWN) && !invalid.isUnknown(),
                "it is not the all-zero unset identity");
        byte[] octets = HeaderCodec.encodeReplyHeader(
                new ReplyHeader(invalid, RemoteExceptionCode.REMOTE_EX_OK));
        byte[] expected = new byte[16 + 8 + 4];
        for (int at = 16; at < 20; ++at) {
            expected[at] = (byte) 0xFF; // high = -1, little-endian
        }
        check(Arrays.equals(octets, expected), "the invalid identity takes ff ff ff ff 00 00 00 00 after the GUID");
        check(HeaderCodec.decodeReplyHeader(octets, new HeaderCodec.Cursor(0))
                        .relatedRequestId().equals(invalid),
                "and reads back as itself");
    }

    public static void main(String[] args) throws Exception {
        if (args.length > 0 && args[0].equals("--header-octets")) {
            printHeaderOctets();
            return;
        }

        basicPolling();
        wireEncoding();
        relatedIdentityPolicy();
        matching();
        qos();
        arrival();
        enhanced();
        headerCodec();
        overTheJavaPsm();
        psmEntities();
        nondataSamples();

        if (failures != 0) {
            System.err.println(failures + " checks failed");
            System.exit(1);
        }
        System.out.println("org.omg.dds.rpc round trip: every check held");
    }
}
