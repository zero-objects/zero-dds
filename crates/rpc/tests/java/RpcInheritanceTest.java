// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The shape of the Java binding of DDS-RPC 1.0 §7.11.2, measured with
// reflection and then used: the entities §7.11.2.2.1 lists are interfaces,
// the function-call types of §7.11.2.3 inherit what the machine-readable
// Server.java and ClientEndpoint.java give them, `Sample.Iterator` is the
// closeable iterator of [DDS-Java-PSM] §7.7.2, and the runtime makes
// services and clients from the constructors the generator wrote.
//
// Compiled together with RpcRuntimeRoundtrip.java, whose in-process
// binding of the three abstract entities it uses, and with the generated
// `ZdServices` of the interface `Calculator { long add(in long a, in long b); }`.

import java.io.Closeable;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.TimeUnit;

import org.omg.dds.core.Duration;
import org.omg.dds.rpc.Client;
import org.omg.dds.rpc.ClientEndpoint;
import org.omg.dds.rpc.ClientParams;
import org.omg.dds.rpc.Future;
import org.omg.dds.rpc.FutureCompletionListener;
import org.omg.dds.rpc.RPCEntity;
import org.omg.dds.rpc.RPCRuntime;
import org.omg.dds.rpc.Replier;
import org.omg.dds.rpc.ReplierParams;
import org.omg.dds.rpc.Requester;
import org.zerodds.ddsrpc.RequesterImpl;
import org.omg.dds.rpc.RequesterParams;
import org.omg.dds.rpc.Sample;
import org.omg.dds.rpc.SampleIdentity;
import org.omg.dds.rpc.Server;
import org.omg.dds.rpc.Service;
import org.omg.dds.rpc.ServiceEndpoint;
import org.omg.dds.rpc.ServiceParams;
import org.omg.dds.rpc.ServiceProxy;

public final class RpcInheritanceTest {

    private static int failures = 0;

    static void check(boolean held, String what) {
        if (!held) {
            System.err.println("FAIL " + what);
            ++failures;
        }
    }

    /** The implementation of the generated {@code Calculator}. */
    static final class Adder implements ZdServices.robot.Calculator {
        @Override
        public int add(int a, int b) {
            return a + b;
        }
    }

    // -- §7.11.2.2.1: the entities are interfaces ------------------------

    static void interfaces() {
        Class<?>[] declared = {
            Requester.class,
            Replier.class,
            RequesterParams.class,
            ReplierParams.class,
            ServiceParams.class,
            ClientParams.class,
            Server.class,
            Service.class,
            ServiceEndpoint.class,
            Client.class,
            ClientEndpoint.class,
            ServiceProxy.class,
            RPCEntity.class,
            Sample.class,
            Sample.Iterator.class,
            Future.class,
            FutureCompletionListener.class,
        };
        for (Class<?> type : declared) {
            check(type.isInterface(), "§7.11.2.2.1: " + type.getName() + " is an interface");
        }
        // §7.11.2.2.3: the one abstract class with a placeholder implementation.
        check(java.lang.reflect.Modifier.isAbstract(RPCRuntime.class.getModifiers())
                        && !RPCRuntime.class.isInterface(),
                "§7.11.2.2.3: RPCRuntime is an abstract class");

        // The machine-readable Server.java: Server extends RPCEntity, Runnable.
        check(Arrays.asList(Server.class.getInterfaces()).contains(RPCEntity.class),
                "Server extends RPCEntity");
        check(Arrays.asList(Server.class.getInterfaces()).contains(Runnable.class),
                "Server extends Runnable");
        for (String name : new String[] {"run", "getDefaultServiceParams", "isNull", "close"}) {
            boolean found = false;
            for (Method method : Server.class.getMethods()) {
                found |= method.getName().equals(name);
            }
            check(found, "Server has " + name);
        }
        // ClientEndpoint extends ServiceProxy and gives the two entities.
        check(Arrays.asList(ClientEndpoint.class.getInterfaces()).contains(ServiceProxy.class),
                "ClientEndpoint extends ServiceProxy");
        boolean writer = false;
        boolean reader = false;
        for (Method method : ClientEndpoint.class.getDeclaredMethods()) {
            writer |= method.getReturnType() == org.omg.dds.rpc.DataWriter.class;
            reader |= method.getReturnType() == org.omg.dds.rpc.DataReader.class;
        }
        check(writer && reader, "ClientEndpoint gives the request writer and the reply reader");
    }

    // -- Figure 7.5: the inheritance hierarchy of the RPC entities ----------

    static void figure75() {
        // Direct super-interfaces, exactly as the figure draws its arrows.
        Object[][] edges = {
            {Server.class, new Class<?>[] {RPCEntity.class, Runnable.class}},
            {ServiceEndpoint.class, new Class<?>[] {RPCEntity.class}},
            {ServiceProxy.class, new Class<?>[] {RPCEntity.class}},
            {Replier.class, new Class<?>[] {RPCEntity.class}},
            {Service.class, new Class<?>[] {ServiceEndpoint.class}},
            {Requester.class, new Class<?>[] {ServiceProxy.class}},
            {ClientEndpoint.class, new Class<?>[] {ServiceProxy.class}},
            {Client.class, new Class<?>[] {ClientEndpoint.class}},
        };
        for (Object[] edge : edges) {
            Class<?> type = (Class<?>) edge[0];
            List<Class<?>> supers = Arrays.asList(type.getInterfaces());
            List<Class<?>> expected = new ArrayList<>(Arrays.asList((Class<?>[]) edge[1]));
            check(supers.size() == expected.size() && supers.containsAll(expected),
                    "Figure 7.5: " + type.getSimpleName() + " extends exactly " + expected
                            + ", found " + supers);
        }
        // Transitive: every entity of the figure is an RPCEntity.
        Class<?>[] entities = {
            Server.class, ServiceEndpoint.class, Service.class, Replier.class,
            ServiceProxy.class, ClientEndpoint.class, Client.class, Requester.class,
        };
        for (Class<?> type : entities) {
            check(RPCEntity.class.isAssignableFrom(type),
                    "Figure 7.5: " + type.getSimpleName() + " is an RPCEntity");
        }
        // Branches the figure keeps apart.
        check(!ServiceEndpoint.class.isAssignableFrom(Replier.class),
                "Figure 7.5: Replier is not a ServiceEndpoint");
        check(!ServiceEndpoint.class.isAssignableFrom(Server.class),
                "Figure 7.5: Server is not a ServiceEndpoint");
        check(!ServiceProxy.class.isAssignableFrom(Service.class)
                        && !ServiceProxy.class.isAssignableFrom(Replier.class),
                "Figure 7.5: the service side is not a ServiceProxy");
        check(!ClientEndpoint.class.isAssignableFrom(Requester.class),
                "Figure 7.5: Requester is not a ClientEndpoint");
        check(ServiceProxy.class.isAssignableFrom(Client.class)
                        && ServiceProxy.class.isAssignableFrom(Requester.class),
                "Figure 7.5: Client and Requester are ServiceProxy");
        // TReq and TRep are the type parameters of Requester and Replier only.
        check(Requester.class.getTypeParameters().length == 2
                        && Requester.class.getTypeParameters()[0].getName().equals("TReq")
                        && Requester.class.getTypeParameters()[1].getName().equals("TRep"),
                "Figure 7.5: Requester<TReq, TRep>");
        check(Replier.class.getTypeParameters().length == 2
                        && Replier.class.getTypeParameters()[0].getName().equals("TReq")
                        && Replier.class.getTypeParameters()[1].getName().equals("TRep"),
                "Figure 7.5: Replier<TReq, TRep>");
        for (Class<?> type : new Class<?>[] {
            Server.class, ServiceEndpoint.class, Service.class, ServiceProxy.class,
            ClientEndpoint.class, Client.class, RPCEntity.class,
        }) {
            check(type.getTypeParameters().length == 0,
                    "Figure 7.5: " + type.getSimpleName() + " has no type parameter");
        }
    }

    // -- the machine-readable SampleIdentity and Sample -------------------

    static void zipShapes() {
        byte[] guid = new byte[16];
        for (int at = 0; at < 16; ++at) {
            guid[at] = (byte) (at + 1);
        }
        SampleIdentity identity = new SampleIdentity(guid, (7L << 32) | 9L);
        org.omg.dds.core.SampleIdentity core = identity;
        check(core.sequenceNumber() == ((7L << 32) | 9L), "SampleIdentity.sequenceNumber() is the long");
        check(core.guid().equals(new java.util.UUID(0x0102030405060708L, 0x090A0B0C0D0E0F10L)),
                "SampleIdentity.guid() is the UUID of the sixteen octets");
        check(core.equals(new SampleIdentity(guid, (7L << 32) | 9L))
                        && core.hashCode() == identity.hashCode(),
                "and the identity has value equality");
        Sample<String> sample = Sample.of("x", identity, SampleIdentity.RELATED_SAMPLE_IDENTITY_INVALID);
        check(sample.getIdentity().equals(identity), "Sample.getIdentity()");
        check(sample.getRelatedIdentity().equals(SampleIdentity.RELATED_SAMPLE_IDENTITY_INVALID),
                "Sample.getRelatedIdentity()");
    }

    // -- §7.11.2.2.7: Sample.Iterator -------------------------------------

    static List<Sample<String>> samples() {
        List<Sample<String>> out = new ArrayList<>();
        out.add(Sample.of("a", SampleIdentity.UNKNOWN, SampleIdentity.UNKNOWN));
        out.add(Sample.of("b", SampleIdentity.UNKNOWN, SampleIdentity.UNKNOWN));
        return out;
    }

    static void sampleIterator() throws Exception {
        check(Closeable.class.isAssignableFrom(Sample.Iterator.class),
                "[DDS-Java-PSM] §7.7.2: Sample.Iterator is java.io.Closeable");
        check(java.util.ListIterator.class.isAssignableFrom(Sample.Iterator.class),
                "and a java.util.ListIterator");
        check(Sample.Iterator.class == org.omg.dds.sub.Sample.Iterator.class,
                "§7.11.2.2.7: the Sample.Iterator of rpc.Sample is the one of [DDS-Java-PSM]");

        org.zerodds.ddsrpc.RpcSampleIterator<String> loan =
                (org.zerodds.ddsrpc.RpcSampleIterator<String>) Sample.iterate(samples());
        check(loan.getClass().getMethod("close").getExceptionTypes().length == 0,
                "the runtime's close() raises no checked exception");
        check(!loan.isClosed() && loan.hasNext(), "an iterator starts open");
        check(loan.next().getData().equals("a"), "it walks the samples");
        loan.close();
        check(loan.isClosed() && !loan.hasNext() && !loan.hasPrevious(),
                "close() returns the loan: nothing is left to walk");
        boolean refused = false;
        try {
            loan.next();
        } catch (IllegalStateException expected) {
            refused = true;
        }
        check(refused, "next() after close() raises IllegalStateException");
        loan.close();
        loan.returnLoan();
        check(loan.isClosed(), "closing again is allowed");

        // try-with-resources, the use Closeable is for.
        int seen = 0;
        org.zerodds.ddsrpc.RpcSampleIterator<String> escaped;
        try (org.zerodds.ddsrpc.RpcSampleIterator<String> walk =
                (org.zerodds.ddsrpc.RpcSampleIterator<String>) Sample.iterate(samples())) {
            escaped = walk;
            while (walk.hasNext()) {
                walk.next();
                ++seen;
            }
        }
        check(seen == 2, "the iterator walks both samples");
        check(escaped.isClosed(), "leaving try-with-resources returned the loan");

        // The iterator of a requester is closeable too.
        RpcRuntimeRoundtrip.Bus bus = new RpcRuntimeRoundtrip.Bus();
        RpcRuntimeRoundtrip.LoopbackParticipant participant =
                new RpcRuntimeRoundtrip.LoopbackParticipant(bus);
        Requester<Integer, Integer> requester =
                new RequesterImpl<>(RpcRuntimeRoundtrip.enhancedRequester(participant));
        try (Sample.Iterator<Integer> replies = requester.replyIterator()) {
            check(!replies.hasNext(), "no reply has arrived");
        }
        requester.close();
    }

    // -- the Server, the factories of the runtime -------------------------

    static void server() throws Exception {
        RpcRuntimeRoundtrip.Bus bus = new RpcRuntimeRoundtrip.Bus();
        RpcRuntimeRoundtrip.LoopbackParticipant participant =
                new RpcRuntimeRoundtrip.LoopbackParticipant(bus);
        RPCRuntime runtime = RPCRuntime.over(participant);

        Server server = runtime.createServer();
        check(server.getEnvironment() == runtime, "a server returns the runtime it was made by");
        check(!server.isNull(), "a server is not null");
        boolean loose = false;
        try {
            Server.create().getEnvironment();
        } catch (IllegalStateException expected) {
            loose = true;
        }
        check(loose, "a server made without a runtime has no environment");

        // getDefaultServiceParams: bound to the participant of the runtime.
        ServiceParams defaults = server.getDefaultServiceParams();
        check(defaults.domainParticipant() == participant, "the default params hold the participant");
        check(defaults.serviceName().isEmpty() && defaults.instanceName().isEmpty(),
                "and no names");
        check(server.getDefaultServiceParams() != defaults, "each call returns a new object");
        check(runtime.createServiceParams().domainParticipant() == participant,
                "the runtime makes service params over its participant");
        check(runtime.createClientParams().domainParticipant() == participant,
                "and client params");

        // The params fill the params of one interface (§7.4.2.3).
        ServiceParams named = ServiceParams.create()
                .domainParticipant(participant)
                .serviceName("Left")
                .instanceName("left-1")
                .requestTopicNameSuffix("_Rq");
        ReplierParams replier = named.applyTo(ReplierParams.create());
        check(replier.domainParticipant() == participant && replier.serviceName().equals("Left")
                        && replier.instanceName().equals("left-1")
                        && replier.requestTopicNameSuffix().equals("_Rq"),
                "ServiceParams fill ReplierParams");
        RequesterParams requester =
                ClientParams.create().domainParticipant(participant).serviceName("Left")
                        .instanceName("left-1").applyTo(RequesterParams.create());
        check(requester.serviceName().equals("Left") && requester.instanceName().equals("left-1"),
                "ClientParams fill RequesterParams");

        // The generated constructors, through the runtime.
        bus.notify = true;
        ZdServices.robot.CalculatorServiceImpl service =
                runtime.createService(p -> new ZdServices.robot.CalculatorServiceImpl(p, new Adder()));
        ZdServices.robot.CalculatorClientImpl client =
                runtime.createClient(ZdServices.robot.CalculatorClientImpl::new);
        check(service.getEnvironment() == runtime, "a service made by the runtime returns it");
        check(client.getEnvironment() == runtime, "a client made by the runtime returns it");
        check(client.getRPCRuntime() == runtime && service.getRPCRuntime() == runtime
                        && server.getRPCRuntime() == runtime,
                "RPCEntity.getRPCRuntime() is the quick access to the runtime");
        check(client.add(2, 40) == 42, "the client calls the service through the runtime's factories");

        // ClientEndpoint: the entities at the client side.
        check(client.requestWriter() != null
                        && client.requestWriter()
                                == participant.createdOn(
                                        ZdServices.robot.CalculatorSupport.REQUEST_TOPIC_NAME, true).entity,
                "requestWriter() is the writer of the request topic");
        check(client.replyReader() != null
                        && client.replyReader()
                                == participant.createdOn(
                                        ZdServices.robot.CalculatorSupport.REPLY_TOPIC_NAME, false).entity,
                "replyReader() is the reader of the reply topic");

        // The instance-named constructors.
        ZdServices.robot.CalculatorServiceImpl named1 =
                runtime.createService(
                        (p, instance) -> new ZdServices.robot.CalculatorServiceImpl(p, new Adder(), instance),
                        "calc-7");
        ZdServices.robot.CalculatorClientImpl bound =
                runtime.createClient(ZdServices.robot.CalculatorClientImpl::new, "calc-7");
        check(bound.isBound() && bound.boundInstanceName().equals("calc-7"),
                "createClient(constructor, instance) binds the client");
        check(participant.createdOn(ZdServices.robot.CalculatorSupport.REQUEST_TOPIC_NAME, false) != null,
                "the services announced their readers");

        // A runtime over no participant cannot make an entity from a constructor.
        boolean none = false;
        try {
            RPCRuntime.over(null).createService(p -> new ZdServices.robot.CalculatorServiceImpl(p, new Adder()));
        } catch (IllegalStateException expected) {
            none = true;
        }
        check(none, "createService over no participant is refused");

        // Server.add: one server per service, each service once.
        server.add(service);
        server.add(named1);
        check(server.size() == 2, "the server holds two services");
        boolean again = false;
        try {
            server.add(service);
        } catch (IllegalStateException expected) {
            again = true;
        }
        check(again, "§7.11.1.5.1: a Service already added to this server is refused");
        boolean elsewhere = false;
        try {
            runtime.createServer().add(service);
        } catch (IllegalStateException expected) {
            elsewhere = true;
        }
        check(elsewhere, "§7.11.1.5.1: a Service bound to a server is refused by another");
        boolean nothing = false;
        try {
            server.add(null);
        } catch (NullPointerException expected) {
            nothing = true;
        }
        check(nothing, "adding no service is refused");

        // run(Duration): non-blocking, bounded by the duration.
        bus.notify = false;
        Duration wait = runtime.getSPI().newDuration(40, TimeUnit.MILLISECONDS);
        long started = System.nanoTime();
        server.run(wait);
        long elapsed = System.nanoTime() - started;
        check(elapsed >= TimeUnit.MILLISECONDS.toNanos(40),
                "run(Duration) waited the duration for a request that did not come");
        check(elapsed < TimeUnit.SECONDS.toNanos(10), "and returned");

        Future<Integer> pending = client.add_async(20, 22);
        check(!pending.isDone(), "a request nobody has answered is pending");
        started = System.nanoTime();
        server.run(runtime.getSPI().newDuration(10, TimeUnit.SECONDS));
        check(System.nanoTime() - started < TimeUnit.SECONDS.toNanos(5),
                "run(Duration) returns when a round answered a request");
        client.dispatchReplies();
        check(pending.isDone() && pending.get() == 42, "the request was answered by run(Duration)");

        // run(): blocks until close().
        Thread blocking = new Thread(server::run);
        blocking.start();
        Thread.sleep(50);
        check(blocking.isAlive(), "run() blocks");
        server.close();
        blocking.join(5000);
        check(!blocking.isAlive(), "close() ends run()");
        check(server.isNull() && server.size() == 0, "a closed server is null and holds nothing");
        check(service.isNull() && named1.isNull(), "close() closed the services");
        boolean closedAdd = false;
        try {
            server.add(service);
        } catch (IllegalStateException expected) {
            closedAdd = true;
        }
        check(closedAdd, "a closed server takes no service");

        // After close() the entities are gone.
        bound.close();
        client.close();
        boolean gone = false;
        try {
            client.requestWriter();
        } catch (IllegalStateException expected) {
            gone = true;
        }
        check(gone, "requestWriter() of a closed client is refused");
        boolean goneReader = false;
        try {
            client.replyReader();
        } catch (IllegalStateException expected) {
            goneReader = true;
        }
        check(goneReader, "replyReader() of a closed client is refused");
    }


    // -- the operations of omg-dds-rpc-java.zip ----------------------------

    static void specOperations() throws Exception {
        RpcRuntimeRoundtrip.Bus bus = new RpcRuntimeRoundtrip.Bus();
        RpcRuntimeRoundtrip.LoopbackParticipant participant =
                new RpcRuntimeRoundtrip.LoopbackParticipant(bus);
        RPCRuntime runtime = RPCRuntime.over(participant);
        Duration shortWait = runtime.getSPI().newDuration(30, TimeUnit.MILLISECONDS);
        Duration longWait = runtime.getSPI().newDuration(5, TimeUnit.SECONDS);

        org.omg.dds.rpc.Replier<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply>
                replier = new org.zerodds.ddsrpc.ReplierImpl<>(
                        RpcRuntimeRoundtrip.replierParams(participant, "calc-A"));
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> requester =
                new RequesterImpl<>(RpcRuntimeRoundtrip.requesterParams(participant));

        // The params, in the spelling of the Java file of the specification.
        RequesterParams params = requester.getRequesterParams();
        check(params.getRequestTopicName().equals(ZdServices.robot.CalculatorSupport.REQUEST_TOPIC_NAME)
                        && params.getReplyTopicName()
                                .equals(ZdServices.robot.CalculatorSupport.REPLY_TOPIC_NAME),
                "getRequestTopicName and getReplyTopicName");
        check(params.getDomainParticipant() == null,
                "a participant that is not the PSM's has no PSM participant");
        RequesterParams copy = params.clone();
        copy.withServiceName("Other");
        check(params.getServiceName().isEmpty() && copy.getServiceName().equals("Other"),
                "clone() is a copy: withServiceName on it leaves the original");
        check(replier.getReplierParams().getInstanceName().equals("calc-A"),
                "ReplierParams.getInstanceName");

        // ServiceProxy: instances, waiting, bind.
        check(requester.getDiscoveredServiceInstances().contains("calc-A"),
                "getDiscoveredServiceInstances lists the instance the replier announced");
        // The waits of the Java files return nothing; one that runs out throws TimeoutError.
        requester.waitForService("calc-A");
        requester.waitForServices(1);
        requester.waitForService(shortWait);
        requester.waitForServices(shortWait, "calc-A");
        check(requester.isServiceMatched(), "waitForService / waitForServices return once matched");
        boolean ranOut = false;
        long started = System.nanoTime();
        try {
            requester.waitForService(shortWait, "nobody");
        } catch (org.omg.dds.rpc.TimeoutError expected) {
            ranOut = System.nanoTime() - started >= TimeUnit.MILLISECONDS.toNanos(25);
        }
        check(ranOut,
                "waitForService(maxWait, name) throws TimeoutError at maxWait for an instance that is not there");
        check(requester.waitForServicesAsync("calc-A").get(5, TimeUnit.SECONDS) == null
                        && requester.waitForServiceAsync().get(5, TimeUnit.SECONDS) == null,
                "the Async forms are futures that become ready");
        check(requester.bind("calc-A") && requester.getBoundInstanceName().equals("calc-A")
                        && requester.unbind() && !requester.unbind(),
                "bind and unbind report what they did");
        // The replier answers the instance it was made for (§7.11.1.4.3).
        requester.bind("calc-A");

        // Replier: wait, read (leaves), take (removes), into a SampleHolder.
        org.omg.dds.rpc.SampleHolder<ZdServices.robot.Calculator_Request> holder =
                new org.omg.dds.rpc.SampleHolder<>();
        check(!holder.validData() && holder.getData() == null, "a holder before a call is empty");
        check(!replier.takeRequest(holder) && !replier.waitForRequests(shortWait),
                "no request: take is false and wait runs out");
        SampleIdentity first = SampleIdentity.of(requester.sendRequest(RpcRuntimeRoundtrip.addRequest(2, 3)));
        check(replier.waitForRequests(longWait) && replier.waitForRequests(1, longWait),
                "waitForRequests sees the request");
        check(replier.readRequest(holder) && holder.validData(), "readRequest fills the holder");
        check(replier.readRequests(10).hasNext(), "readRequests leaves the request");
        org.omg.dds.rpc.SampleHolder<ZdServices.robot.Calculator_Request> taken =
                new org.omg.dds.rpc.SampleHolder<>();
        check(replier.receiveRequest(taken, longWait) && taken.identity().equals(first),
                "receiveRequest takes the request under the identity sendRequest returned");
        check(!replier.takeRequest(taken), "and it is gone");
        boolean refused = false;
        try {
            replier.takeRequest(Sample.of(null, SampleIdentity.UNKNOWN, SampleIdentity.UNKNOWN));
        } catch (IllegalArgumentException expected) {
            refused = true;
        }
        check(refused, "a sample that is not a SampleHolder is refused");

        // Requester: read leaves, take removes, by identity.
        replier.sendReply(RpcRuntimeRoundtrip.refusalReply(), first);
        check(requester.waitForReplies(1, longWait) && requester.waitForReplies(longWait)
                        && requester.waitForReplies(1, longWait, first),
                "waitForReplies sees the reply");
        check(requester.readReply() != null && requester.readReply(first) != null
                        && requester.readReplies(5).hasNext()
                        && requester.readReplies(first) != null,
                "readReply and readReplies leave the reply");
        check(requester.takeReply(first) != null && requester.takeReply() == null,
                "takeReply(id) removes it");

        // Several replies; the one asked for, the oldest, the iterator.
        SampleIdentity a = SampleIdentity.of(requester.sendRequest(RpcRuntimeRoundtrip.addRequest(1, 1)));
        SampleIdentity b = SampleIdentity.of(requester.sendRequest(RpcRuntimeRoundtrip.addRequest(2, 2)));
        SampleIdentity c = SampleIdentity.of(requester.sendRequest(RpcRuntimeRoundtrip.addRequest(3, 3)));
        for (Sample<ZdServices.robot.Calculator_Request> request : replier.receiveRequests()) {
            replier.sendReply(RpcRuntimeRoundtrip.refusalReply(), request.identity());
        }
        check(requester.takeReply(b) != null && requester.takeReply(b) == null,
                "takeReply(id) takes that reply only");
        try (Sample.Iterator<ZdServices.robot.Calculator_Reply> two = requester.takeReplies(1)) {
            check(two.hasNext()
                            && ((Sample<ZdServices.robot.Calculator_Reply>) two.next())
                                    .relatedIdentity().equals(a)
                            && !two.hasNext(),
                    "takeReplies(1) takes the oldest");
        }
        try (Sample.Iterator<ZdServices.robot.Calculator_Reply> rest = requester.receiveReplies(longWait)) {
            check(rest.hasNext()
                            && ((Sample<ZdServices.robot.Calculator_Reply>) rest.next())
                                    .relatedIdentity().equals(c),
                    "receiveReplies(maxWait) takes what is left");
        }
        SampleIdentity d = SampleIdentity.of(requester.sendRequest(RpcRuntimeRoundtrip.addRequest(4, 4)));
        for (Sample<ZdServices.robot.Calculator_Request> request : replier.receiveRequests()) {
            replier.sendReply(RpcRuntimeRoundtrip.refusalReply(), request.identity());
        }
        Sample<ZdServices.robot.Calculator_Reply> got = requester.receiveReply(longWait);
        check(got != null && got.relatedIdentity().equals(d), "receiveReply(timeout)");
        check(requester.receiveReply(shortWait) == null, "receiveReply(timeout) is null when none comes");

        // sendRequestOneway: the reply is dropped.
        requester.sendRequestOneway(RpcRuntimeRoundtrip.addRequest(9, 9));
        for (Sample<ZdServices.robot.Calculator_Request> request : replier.receiveRequests()) {
            replier.sendReply(RpcRuntimeRoundtrip.refusalReply(), request.identity());
        }
        check(!requester.waitForReplies(1, shortWait) && requester.takeReply() == null,
                "the reply to a oneway request is dropped");
        check(!requester.receiveNondataSamples(true) && requester.receiveNondataSamples(false),
                "receiveNondataSamples returns the previous setting");

        // The executor runs the listener; the listener in the spelling of the file.
        java.util.concurrent.ExecutorService executor =
                java.util.concurrent.Executors.newSingleThreadExecutor();
        java.util.concurrent.CountDownLatch heard = new java.util.concurrent.CountDownLatch(1);
        Thread[] ran = new Thread[1];
        org.omg.dds.rpc.SimpleRequesterListener<ZdServices.robot.Calculator_Reply> listener =
                new org.omg.dds.rpc.SimpleRequesterListener<ZdServices.robot.Calculator_Reply>() {
                    @Override
                    public void process_reply(
                            Sample<ZdServices.robot.Calculator_Reply> sample,
                            org.omg.dds.core.SampleIdentity identity) {
                        ran[0] = Thread.currentThread();
                        heard.countDown();
                    }
                };
        org.omg.dds.rpc.Replier<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply>
                other = new org.zerodds.ddsrpc.ReplierImpl<>(
                        RpcRuntimeRoundtrip.replierParams(participant, "")
                                .requestTopicName("Exec_Request").replyTopicName("Exec_Reply"));
        Requester<ZdServices.robot.Calculator_Request, ZdServices.robot.Calculator_Reply> viaExecutor =
                new RequesterImpl<>(
                        RpcRuntimeRoundtrip.requesterParams(participant)
                                .requestTopicName("Exec_Request").replyTopicName("Exec_Reply")
                                .simpleRequesterListener(listener).withExecutor(executor));
        bus.notify = true;
        viaExecutor.sendRequest(RpcRuntimeRoundtrip.addRequest(1, 2));
        for (Sample<ZdServices.robot.Calculator_Request> request : other.receiveRequests()) {
            other.sendReply(RpcRuntimeRoundtrip.refusalReply(), request.identity());
        }
        check(heard.await(5, TimeUnit.SECONDS) && ran[0] != Thread.currentThread(),
                "the listener of the params runs on the executor of the params");
        bus.notify = false;
        // sendRequestAsync(request, executor): the completion listener runs there as well.
        // The type is java.util.concurrent.Future, as in the machine-readable files; the
        // object is the binding's Future, which takes the completion listener.
        Future<Sample<ZdServices.robot.Calculator_Reply>> future =
                (Future<Sample<ZdServices.robot.Calculator_Reply>>)
                        requester.sendRequestAsync(RpcRuntimeRoundtrip.addRequest(5, 5), executor);
        for (Sample<ZdServices.robot.Calculator_Request> request : replier.receiveRequests()) {
            replier.sendReply(RpcRuntimeRoundtrip.refusalReply(), request.identity());
        }
        java.util.concurrent.CountDownLatch done = new java.util.concurrent.CountDownLatch(1);
        future.withCompletionListener(f -> done.countDown());
        requester.receiveReplies();
        check(done.await(5, TimeUnit.SECONDS), "the future of sendRequestAsync(request, executor) completes");
        executor.shutdown();

        // §7.4.1: the request-reply style has no interface name, so no interface segment
        // (and, §7.4.3, no aliases: they would only repeat the topic name).
        org.omg.dds.rpc.TopicNames plain =
                org.omg.dds.rpc.TopicNames.resolve("", "", "", "", "", "", "",
                        Arrays.asList("Base"));
        check(plain.request().equals("Service_Request") && plain.reply().equals("Service_Reply")
                        && plain.requestAliases().isEmpty() && plain.replyAliases().isEmpty(),
                "an empty interfaceName composes <service>_Request without aliases");
        check(org.omg.dds.rpc.TopicNames.resolve("", "Left", "", "", "", "", "")
                        .request().equals("Left_Request"),
                "and a run-time service name replaces the default");
        // §7.4.3: one alias per base interface, same suffix.
        org.omg.dds.rpc.TopicNames derived =
                org.omg.dds.rpc.TopicNames.resolve("Derived", "", "", "", "", "", "",
                        Arrays.asList("Base", "Other"));
        check(derived.request().equals("Derived_Service_Request")
                        && derived.requestAliases()
                                .equals(Arrays.asList("Base_Service_Request", "Other_Service_Request"))
                        && derived.replyAliases()
                                .equals(Arrays.asList("Base_Service_Reply", "Other_Service_Reply")),
                "§7.4.3: each base interface gives one alias of the request and of the reply topic");
        check(org.omg.dds.rpc.TopicNames.resolve("Derived", "", "", "", "", "Whole_Request", "",
                        Arrays.asList("Base")).requestAliases().isEmpty(),
                "a whole topic name has no interface segment to replace, so no alias");

        // ServerParams, ServiceStatus, getServiceParams, getClientParams.
        bus.notify = true;
        ZdServices.robot.CalculatorServiceImpl service =
                runtime.createService(p -> new ZdServices.robot.CalculatorServiceImpl(p, new Adder()));
        bus.notify = false;
        check(service.getServiceStatus() == org.omg.dds.rpc.ServiceStatus.RUNNING, "RUNNING");
        service.pause();
        check(service.getServiceStatus() == org.omg.dds.rpc.ServiceStatus.PAUSED, "PAUSED");
        service.close();
        check(service.getServiceStatus() == org.omg.dds.rpc.ServiceStatus.CLOSED, "CLOSED");
        check(service.getServiceParams().getDomainParticipant() == null,
                "getServiceParams gives the params the service was made with");
        java.util.concurrent.ExecutorService pool = java.util.concurrent.Executors.newFixedThreadPool(2);
        org.omg.dds.rpc.ServerParams serverParams = runtime.createServerParams()
                .withExecutor(pool)
                .withDefaultServiceParams(runtime.createServiceParams().withServiceName("S"));
        Server server = runtime.createServer(serverParams);
        check(server.getDefaultServiceParams().getServiceName().equals("S")
                        && serverParams.getExecutor() == pool,
                "ServerParams carry the default service params and the executor");
        check(server.runOnce() == 0, "a server with an executor dispatches its rounds");
        pool.shutdown();
        server.close();
        requester.close();
        replier.close();
        other.close();
        viaExecutor.close();
    }

    public static void main(String[] args) throws Exception {
        interfaces();
        figure75();
        zipShapes();
        sampleIterator();
        server();
        specOperations();
        if (failures != 0) {
            System.err.println(failures + " checks failed");
            System.exit(1);
        }
        System.out.println("org.omg.dds.rpc inheritance: every check held");
    }
}
