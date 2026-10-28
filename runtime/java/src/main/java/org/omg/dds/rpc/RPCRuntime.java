// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.BiFunction;
import java.util.function.Function;
import org.omg.dds.core.ServiceEnvironment;

/**
 * DDS-RPC 1.0 §7.11.2.2.3: "RPCRuntime is the only abstract class with
 * placeholder implementation in the Java language binding. It extends
 * ServiceEnvironment abstract class defined in [DDS-Java-PSM]. Its
 * purpose is to provide access to the RPCRuntime singleton, which serves
 * as a factory for all other entities."
 *
 * <p>The singleton is one runtime per participant: {@link #over} returns
 * the same instance for the same participant, so an entity's
 * {@code getEnvironment()} is the runtime it was made from and two
 * calls agree.
 */
public abstract class RPCRuntime extends ServiceEnvironment implements Runnable {

    private static final Map<DomainParticipant, RPCRuntime> RUNTIMES = new ConcurrentHashMap<>();

    /** The runtime over no participant: it makes entities that hold no DDS entity. */
    private static final RPCRuntime DETACHED = newRuntime(null);

    private final DomainParticipant participant;

    /**
     * A runtime over no participant — the constructor of the machine-readable
     * {@code RPCRuntime.java}, for a subclass.
     */
    protected RPCRuntime() {
        this(null);
    }

    protected RPCRuntime(DomainParticipant participant) {
        this.participant = participant;
    }

    private final java.util.List<Server> servers = new java.util.concurrent.CopyOnWriteArrayList<>();

    /**
     * §7.11.2.2.3: access to the RPCRuntime singleton. The runtime of an application that
     * has not named a participant has none; give the entities made from it a participant
     * through their params, or use {@link #over}.
     *
     * @param classloader the class loader of the application (the runtime is one per JVM)
     * @return the runtime over no participant
     */
    public static RPCRuntime getInstance(ClassLoader classloader) {
        return DETACHED;
    }

    /**
     * §7.11.2.2.3 {@code run()}: serve every {@link Server} this runtime created until all of
     * them are closed. Returns at once when it has made none.
     */
    @Override
    public void run() {
        for (Server server : servers) {
            server.run();
        }
    }

    /** The participant every entity this runtime makes is created from. */
    public DomainParticipant domainParticipant() {
        return participant;
    }

    /**
     * The requester {@code params} describe. A participant the params name
     * ({@link RequesterParams#withDomainParticipant}) is the one the entities are created from;
     * params that name none get this runtime's.
     */
    public <TReq, TRep> Requester<TReq, TRep> createRequester(RequesterParams params) {
        if (params.domainParticipant() == null) {
            params.domainParticipant(participant);
        }
        return new org.zerodds.ddsrpc.RequesterImpl<>(this, params);
    }

    /** The replier {@code params} describe; the participant as for {@link #createRequester(RequesterParams)}. */
    public <TReq, TRep> Replier<TReq, TRep> createReplier(ReplierParams params) {
        if (params.domainParticipant() == null) {
            params.domainParticipant(participant);
        }
        return new org.zerodds.ddsrpc.ReplierImpl<>(this, params);
    }

    /**
     * The ServiceProviderInterface of an RPC runtime. It delegates the DDS-Java-PSM
     * 1.0 §7.3.1 object factories to the environment of the participant the
     * runtime was made over, so an RPC entity and the DCPS entities around it
     * share one ServiceEnvironment.
     */
    private static final class RuntimeSPI implements org.omg.dds.core.ServiceEnvironment.ServiceProviderInterface {
        @Override
        public org.omg.dds.domain.DomainParticipantFactory getParticipantFactory() {
            throw new UnsupportedOperationException(
                    "an RPCRuntime does not create DomainParticipants");
        }

        @Override
        public org.omg.dds.type.dynamic.DynamicTypeFactory getTypeFactory() {
            return environment().getSPI().getTypeFactory();
        }

        @Override
        public <TYPE> org.omg.dds.type.TypeSupport<TYPE> newTypeSupport(
                Class<TYPE> type, String registeredName) {
            return environment().getSPI().newTypeSupport(type, registeredName);
        }

        @Override
        public org.omg.dds.core.Duration newDuration(long duration, java.util.concurrent.TimeUnit unit) {
            return environment().getSPI().newDuration(duration, unit);
        }

        @Override
        public org.omg.dds.core.Duration infiniteDuration() {
            return environment().getSPI().infiniteDuration();
        }

        @Override
        public org.omg.dds.core.Duration zeroDuration() {
            return environment().getSPI().zeroDuration();
        }

        @Override
        public org.omg.dds.core.ModifiableTime newTime(long time, java.util.concurrent.TimeUnit units) {
            return environment().getSPI().newTime(time, units);
        }

        @Override
        public org.omg.dds.core.Time invalidTime() {
            return environment().getSPI().invalidTime();
        }

        @Override
        public org.omg.dds.core.ModifiableInstanceHandle newInstanceHandle() {
            return environment().getSPI().newInstanceHandle();
        }

        @Override
        public org.omg.dds.core.InstanceHandle nilHandle() {
            return environment().getSPI().nilHandle();
        }

        @Override
        public org.omg.dds.core.GuardCondition newGuardCondition() {
            return environment().getSPI().newGuardCondition();
        }

        @Override
        public org.omg.dds.core.WaitSet newWaitSet() {
            return environment().getSPI().newWaitSet();
        }

        @Override
        public java.util.Set<Class<? extends org.omg.dds.core.status.Status>> allStatusKinds() {
            return environment().getSPI().allStatusKinds();
        }

        @Override
        public java.util.Set<Class<? extends org.omg.dds.core.status.Status>> noStatusKinds() {
            return environment().getSPI().noStatusKinds();
        }

        @Override
        public org.omg.dds.core.QosProvider newQosProvider(String uri, String profile) {
            return environment().getSPI().newQosProvider(uri, profile);
        }

        @Override
        public org.omg.dds.core.policy.PolicyFactory getPolicyFactory() {
            return environment().getSPI().getPolicyFactory();
        }

        @Override
        public org.omg.dds.type.dynamic.DynamicDataFactory getDynamicDataFactory() {
            return environment().getSPI().getDynamicDataFactory();
        }

        @Override
        public org.omg.dds.type.builtin.KeyedString newKeyedString() {
            return environment().getSPI().newKeyedString();
        }

        @Override
        public org.omg.dds.type.builtin.KeyedBytes newKeyedBytes() {
            return environment().getSPI().newKeyedBytes();
        }

        private org.omg.dds.core.ServiceEnvironment environment() {
            org.omg.dds.core.ServiceEnvironment env =
                    org.zerodds.core.Values.getDefaultEnvironment();
            return env;
        }
    }

    /** A runtime over {@code participant}, whose SPI delegates the §7.3.1 factories. */
    private static RPCRuntime newRuntime(DomainParticipant participant) {
        return new RPCRuntime(participant) {
            @Override
            public org.omg.dds.core.ServiceEnvironment.ServiceProviderInterface getSPI() {
                return new RuntimeSPI();
            }
        };
    }

    /** Params for {@link #createRequester}, already bound to this runtime's participant. */
    public RequesterParams createRequesterParams() {
        return RequesterParams.create().domainParticipant(participant);
    }

    /** Params for {@link #createReplier}, already bound to this runtime's participant. */
    public ReplierParams createReplierParams() {
        return ReplierParams.create().domainParticipant(participant);
    }

    /** A server for the services of this runtime — §7.11.1.5.3. */
    public Server createServer() {
        Server server = new org.zerodds.ddsrpc.ServerImpl(this);
        servers.add(server);
        return server;
    }

    /** A server configured by {@code params} — §7.11.1.5.3. */
    public Server createServer(ServerParams params) {
        Server server = new org.zerodds.ddsrpc.ServerImpl(this, params);
        servers.add(server);
        return server;
    }

    /**
     * §7.11.2.2.3 {@code createRequester(Class, Class)}: the requester of the interface whose
     * request and reply types these are, with the names, type supports and mapping its
     * generated {@code <I>Support} gives.
     *
     * @throws IllegalArgumentException when the classes are not the topic types of a
     *     generated interface
     * @throws IllegalStateException when this runtime has no participant and no params name one
     */
    public <TReq, TRep> Requester<TReq, TRep> createRequester(
            Class<TReq> requestType, Class<TRep> replyType) {
        return createRequester(requestType, replyType, null);
    }

    /**
     * As {@link #createRequester(Class, Class)}, with {@code params}: params that name no
     * interface get what the generated {@code <I>Support} gives (the names, the type supports,
     * the mapping) and keep the rest they carry (the participant, the Publisher, the
     * Subscriber, the QoS); params that name one, as {@code <I>Support.requesterParams} makes
     * them, are taken as they are. The participant is the one {@code params} name, else this
     * runtime's.
     */
    public <TReq, TRep> Requester<TReq, TRep> createRequester(
            Class<TReq> requestType, Class<TRep> replyType, RequesterParams params) {
        DomainParticipant bound = params != null && params.domainParticipant() != null
                ? params.domainParticipant()
                : requireParticipant("create from a class");
        RequesterParams made = generated(requestType, replyType, "requesterParams", bound);
        if (params != null && !params.interfaceName().isEmpty()) {
            made = params;
        } else if (params != null) {
            params.interfaceName(made.interfaceName())
                    .requestTypeName(made.requestTypeName())
                    .replyTypeName(made.replyTypeName())
                    .requestTypeSupport(made.requestTypeSupport())
                    .replyTypeSupport(made.replyTypeSupport())
                    .mapping(made.mapping())
                    .wireEncoding(made.wireEncoding())
                    .parents(made.parents())
                    .typeDescriptions(made.requestTypeDescription(), made.replyTypeDescription());
            if (params.requestTopicName().isEmpty()) {
                params.requestTopicName(made.requestTopicName());
            }
            if (params.replyTopicName().isEmpty()) {
                params.replyTopicName(made.replyTopicName());
            }
            made = params;
        }
        if (made.domainParticipant() == null) {
            made.domainParticipant(bound);
        }
        return createRequester(made);
    }

    /** §7.11.2.2.3 {@code createReplier(Class, Class)}; see {@link #createRequester(Class, Class)}. */
    public <TReq, TRep> Replier<TReq, TRep> createReplier(
            Class<TReq> requestType, Class<TRep> replyType) {
        return createReplier(requestType, replyType, null);
    }

    /** As {@link #createRequester(Class, Class, RequesterParams)}, for a replier. */
    public <TReq, TRep> Replier<TReq, TRep> createReplier(
            Class<TReq> requestType, Class<TRep> replyType, ReplierParams params) {
        DomainParticipant bound = params != null && params.domainParticipant() != null
                ? params.domainParticipant()
                : requireParticipant("create from a class");
        ReplierParams made = generated(requestType, replyType, "replierParams", bound);
        if (params != null && !params.interfaceName().isEmpty()) {
            made = params;
        } else if (params != null) {
            params.interfaceName(made.interfaceName())
                    .requestTypeName(made.requestTypeName())
                    .replyTypeName(made.replyTypeName())
                    .requestTypeSupport(made.requestTypeSupport())
                    .replyTypeSupport(made.replyTypeSupport())
                    .mapping(made.mapping())
                    .wireEncoding(made.wireEncoding())
                    .parents(made.parents())
                    .typeDescriptions(made.requestTypeDescription(), made.replyTypeDescription());
            if (params.requestTopicName().isEmpty()) {
                params.requestTopicName(made.requestTopicName());
            }
            if (params.replyTopicName().isEmpty()) {
                params.replyTopicName(made.replyTopicName());
            }
            made = params;
        }
        if (made.domainParticipant() == null) {
            made.domainParticipant(bound);
        }
        return createReplier(made);
    }

    /**
     * The params the generated {@code <I>Support} next to the topic types of an interface
     * makes: the request class is {@code <I>_Request}, the Support {@code <I>Support}, both
     * members of the same enclosing class.
     */
    @SuppressWarnings("unchecked")
    private static <P> P generated(
            Class<?> requestType, Class<?> replyType, String factory, DomainParticipant bound) {
        String request = requestType.getSimpleName();
        String reply = replyType.getSimpleName();
        Class<?> home = requestType.getEnclosingClass();
        if (home == null
                || !request.endsWith("_Request")
                || !reply.equals(request.substring(0, request.length() - 8) + "_Reply")
                || replyType.getEnclosingClass() != home) {
            throw new IllegalArgumentException(
                    request + " and " + reply + " are not the request and reply type of a"
                            + " generated interface");
        }
        String support = request.substring(0, request.length() - 8) + "Support";
        for (Class<?> member : home.getDeclaredClasses()) {
            if (!member.getSimpleName().equals(support)) {
                continue;
            }
            try {
                if (factory.equals("replierParams")) {
                    return (P) member.getMethod(factory, DomainParticipant.class, String.class)
                            .invoke(null, bound, "");
                }
                return (P) member.getMethod(factory, DomainParticipant.class).invoke(null, bound);
            } catch (ReflectiveOperationException failure) {
                throw new IllegalArgumentException(
                        support + " has no " + factory + "(DomainParticipant)", failure);
            }
        }
        throw new IllegalArgumentException("no " + support + " next to " + request);
    }

    /** Empty server params. */
    public ServerParams createServerParams() {
        return ServerParams.create();
    }

    /** Params for a function-call service, already bound to this runtime's participant. */
    public ServiceParams createServiceParams() {
        return ServiceParams.create().domainParticipant(participant);
    }

    /** Params for a function-call client, already bound to this runtime's participant. */
    public ClientParams createClientParams() {
        return ClientParams.create().domainParticipant(participant);
    }

    /**
     * Make a function-call service from the constructor the generator wrote
     * for its interface — §7.9.1: creating it activates it. The constructor
     * is given the participant of this runtime:
     *
     * <pre>{@code
     * runtime.createService(p -> new ZdServices.robot.CalculatorServiceImpl(p, implementation));
     * }</pre>
     *
     * @param constructor the generated {@code <I>ServiceImpl} constructor
     * @param <S> the service
     * @return the activated service
     * @throws IllegalStateException when this runtime has no participant
     */
    public <S extends Service> S createService(Function<DomainParticipant, S> constructor) {
        return constructor.apply(requireParticipant("createService"));
    }

    /**
     * As {@link #createService(Function)}, for the constructor that names the
     * service instance (§7.6.2.1.1):
     *
     * <pre>{@code
     * runtime.createService(
     *     (p, instance) -> new ZdServices.robot.CalculatorServiceImpl(p, implementation, instance),
     *     "left");
     * }</pre>
     */
    public <S extends Service> S createService(
            BiFunction<DomainParticipant, String, S> constructor, String instanceName) {
        return constructor.apply(requireParticipant("createService"), instanceName);
    }

    /**
     * Make a function-call client from the constructor the generator wrote
     * for its interface — §7.9.1: creating it activates it.
     *
     * <pre>{@code
     * runtime.createClient(ZdServices.robot.CalculatorClientImpl::new);
     * }</pre>
     *
     * @param constructor the generated {@code <I>ClientImpl} constructor
     * @param <C> the client
     * @return the activated client
     * @throws IllegalStateException when this runtime has no participant
     */
    public <C extends ClientEndpoint> C createClient(Function<DomainParticipant, C> constructor) {
        return constructor.apply(requireParticipant("createClient"));
    }

    /**
     * As {@link #createClient(Function)}, for the constructor that binds the
     * client to a service instance (§7.11.1.4.3).
     */
    public <C extends ClientEndpoint> C createClient(
            BiFunction<DomainParticipant, String, C> constructor, String instanceName) {
        return constructor.apply(requireParticipant("createClient"), instanceName);
    }

    private DomainParticipant requireParticipant(String what) {
        if (participant == null) {
            throw new IllegalStateException(
                    what + ": this RPCRuntime is over no DomainParticipant (RPCRuntime.over)");
        }
        return participant;
    }

    /**
     * The runtime over one participant: the same instance every time
     * until {@link #release} is called for it.
     */
    public static RPCRuntime over(DomainParticipant participant) {
        if (participant == null) {
            return DETACHED;
        }
        return RUNTIMES.computeIfAbsent(participant, RPCRuntime::newRuntime);
    }

    /**
     * Forget the runtime of a participant that is being closed, so the
     * registry does not keep the participant alive.
     *
     * @return the runtime that was registered, or {@code null}
     */
    public static RPCRuntime release(DomainParticipant participant) {
        return participant == null ? null : RUNTIMES.remove(participant);
    }
}
