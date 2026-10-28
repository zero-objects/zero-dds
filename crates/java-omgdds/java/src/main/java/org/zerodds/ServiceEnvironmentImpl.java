// SPDX-License-Identifier: Apache-2.0
package org.zerodds;

import org.omg.dds.core.Duration;
import org.omg.dds.core.GuardCondition;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ModifiableTime;
import org.omg.dds.core.QosProvider;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.Time;
import org.omg.dds.core.WaitSet;
import org.omg.dds.core.status.DataAvailableStatus;
import org.omg.dds.core.status.DataOnReadersStatus;
import org.omg.dds.core.status.InconsistentTopicStatus;
import org.omg.dds.core.status.LivelinessChangedStatus;
import org.omg.dds.core.status.LivelinessLostStatus;
import org.omg.dds.core.status.OfferedDeadlineMissedStatus;
import org.omg.dds.core.status.OfferedIncompatibleQosStatus;
import org.omg.dds.core.status.PublicationMatchedStatus;
import org.omg.dds.core.status.RequestedDeadlineMissedStatus;
import org.omg.dds.core.status.RequestedIncompatibleQosStatus;
import org.omg.dds.core.status.SampleLostStatus;
import org.omg.dds.core.status.SampleRejectedStatus;
import org.omg.dds.core.status.Status;
import org.omg.dds.core.status.SubscriptionMatchedStatus;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.type.TypeSupport;
import org.omg.dds.type.builtin.KeyedBytes;
import org.omg.dds.type.builtin.KeyedString;
import org.omg.dds.type.dynamic.DynamicDataFactory;
import org.omg.dds.type.dynamic.DynamicTypeFactory;
import org.zerodds.cdr.BuiltinTypes;
import org.zerodds.cdr.DynamicDataFactoryImpl;
import org.zerodds.cdr.DynamicTypeFactoryImpl;
import org.zerodds.cdr.TypeSupportImpl;
import org.zerodds.cdr.TypeSupportResolver;
import org.zerodds.core.DurationImpl;
import org.zerodds.core.InstanceHandleImpl;
import org.zerodds.core.ModifiableInstanceHandleImpl;
import org.zerodds.core.ModifiableTimeImpl;
import org.zerodds.core.TimeImpl;
import org.zerodds.core.condition.GuardConditionImpl;
import org.zerodds.core.condition.WaitSetImpl;
import org.zerodds.core.policy.PolicyFactoryImpl;
import org.zerodds.core.qos.QosProviderImpl;
import org.zerodds.domain.DomainParticipantFactoryImpl;

import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.TimeUnit;

/**
 * The ZeroDDS {@link ServiceEnvironment} (DDS-Java-PSM 1.0 §7.3.1).
 *
 * <p>This is the class an application names when bootstrapping:
 *
 * <pre>{@code
 * var env = ServiceEnvironment.createInstance();
 * var factory = DomainParticipantFactory.getInstance(env);
 * }</pre>
 *
 * <p>ZeroDDS is pure Java — there is no native library to load — so the
 * environment is a lightweight root that carries its {@linkplain #getSPI()
 * service-provider interface}. §7.3.1 requires every DDS object to be created
 * through that interface, and §7.8.1.1 requires the {@link DynamicTypeFactory}
 * to be a per-environment singleton; both hold because the SPI belongs to one
 * environment instance.
 */
public class ServiceEnvironmentImpl extends ServiceEnvironment {

    /** Reflectively invoked by {@link ServiceEnvironment#createInstance(ClassLoader)}. */
    public ServiceEnvironmentImpl() {
        // The SPI is created on first use: building it hands this object to the
        // factories, which must not happen before a subclass is initialized.
    }

    /** The {@code Map} constructor {@link ServiceEnvironment#createInstance} looks for first. */
    public ServiceEnvironmentImpl(Map<String, Object> environment) {
        this();
    }

    private volatile ServiceProviderInterface spi;

    /**
     * A ready-made environment of this implementation, for the binding's own
     * value types and for callers that do not go through
     * {@link ServiceEnvironment#createInstance(ClassLoader)}. An application
     * that follows §7.3.1 names this class in
     * {@code org.omg.dds.serviceClassName} and gets an equally valid
     * environment from {@code createInstance}.
     */
    private static final ServiceEnvironmentImpl DEFAULT = new ServiceEnvironmentImpl();

    /** The shared environment of {@link #DEFAULT}. */
    public static ServiceEnvironmentImpl getDefaultEnvironment() {
        return DEFAULT;
    }

    @Override
    public ServiceProviderInterface getSPI() {
        ServiceProviderInterface current = spi;
        if (current == null) {
            synchronized (this) {
                current = spi;
                if (current == null) {
                    current = new ZeroSPI(this);
                    spi = current;
                }
            }
        }
        return current;
    }

    /**
     * The ZeroDDS service-provider interface (DDS-Java-PSM 1.0 §7.3.1): the
     * factories that create every ZeroDDS object of one environment.
     */
    private static final class ZeroSPI implements ServiceProviderInterface {

        private final ServiceEnvironmentImpl environment;
        private final PolicyFactory policyFactory;
        private final DynamicTypeFactory typeFactory;
        private final DynamicDataFactory dynamicDataFactory;
        private volatile DomainParticipantFactory participantFactory;

        ZeroSPI(ServiceEnvironmentImpl environment) {
            this.environment = environment;
            this.policyFactory = new PolicyFactoryImpl(environment);
            this.typeFactory = new DynamicTypeFactoryImpl(environment);
            this.dynamicDataFactory = new DynamicDataFactoryImpl(environment);
        }

        @Override
        public DomainParticipantFactory getParticipantFactory() {
            // §7.4.1: one factory per ServiceEnvironment, also when several
            // threads ask for it at the same time (§7.2.4).
            DomainParticipantFactory current = participantFactory;
            if (current == null) {
                synchronized (this) {
                    current = participantFactory;
                    if (current == null) {
                        current = new DomainParticipantFactoryImpl(environment);
                        participantFactory = current;
                    }
                }
            }
            return current;
        }

        @Override
        public DynamicTypeFactory getTypeFactory() {
            return typeFactory;
        }

        @Override
        public <TYPE> TypeSupport<TYPE> newTypeSupport(Class<TYPE> type, String registeredName) {
            // an abstract built-in type has no constructor: the SPI makes its samples
            java.util.function.Supplier<TYPE> samples = null;
            if (type == KeyedString.class) {
                samples = () -> type.cast(newKeyedString());
            } else if (type == KeyedBytes.class) {
                samples = () -> type.cast(newKeyedBytes());
            }
            // §7.5.1: without a name the type is registered under the fully qualified name of
            // the class. TypeSupport.newTypeSupport(Class, ServiceEnvironment) of the OMG
            // source passes type.getClass().getName(), which is "java.lang.Class" whatever the
            // type: that name stands for "no name given".
            String name = registeredName == null
                    || (type != Class.class && Class.class.getName().equals(registeredName))
                    ? type.getName() : registeredName;
            // the codec, and with it the writers and readers of a topic made from this
            // TypeSupport, use the registered name and no other
            return new TypeSupportImpl<>(environment, name, type,
                    org.zerodds.internal.RegisteredNameTypeSupport.named(
                            TypeSupportResolver.resolve(type), name), samples);
        }

        @Override
        public Duration newDuration(long duration, TimeUnit unit) {
            return DurationImpl.of(environment, duration, unit);
        }

        @Override
        public Duration infiniteDuration() {
            return DurationImpl.infinite(environment);
        }

        @Override
        public Duration zeroDuration() {
            return DurationImpl.zero(environment);
        }

        @Override
        public ModifiableTime newTime(long time, TimeUnit units) {
            // §7.3.4: a negative instant, in any unit, is the invalid time;
            // TimeImpl.split yields the invalid pair for it.
            long[] parts = TimeImpl.split(time, units);
            return new ModifiableTimeImpl(environment, parts[0], parts[1]);
        }

        @Override
        public Time invalidTime() {
            return new TimeImpl(environment, -1, 0);
        }

        @Override
        public ModifiableInstanceHandle newInstanceHandle() {
            return new ModifiableInstanceHandleImpl(environment, new byte[16]);
        }

        @Override
        public InstanceHandle nilHandle() {
            return new InstanceHandleImpl(environment, new byte[16]);
        }

        @Override
        public GuardCondition newGuardCondition() {
            return new GuardConditionImpl(environment);
        }

        @Override
        public WaitSet newWaitSet() {
            return new WaitSetImpl(environment);
        }

        @Override
        public Set<Class<? extends Status>> allStatusKinds() {
            return new LinkedHashSet<>(ALL_STATUS_KINDS);
        }

        @Override
        public Set<Class<? extends Status>> noStatusKinds() {
            return Collections.emptySet();
        }

        @Override
        public QosProvider newQosProvider(String uri, String profile) {
            return new QosProviderImpl(environment, uri, profile);
        }

        @Override
        public PolicyFactory getPolicyFactory() {
            return policyFactory;
        }

        @Override
        public DynamicDataFactory getDynamicDataFactory() {
            return dynamicDataFactory;
        }

        @Override
        public KeyedString newKeyedString() {
            return new BuiltinTypes.KeyedStringImpl(environment);
        }

        @Override
        public KeyedBytes newKeyedBytes() {
            return new BuiltinTypes.KeyedBytesImpl(environment);
        }
    }

    /** The thirteen communication statuses of DDS 1.2 §7.1.4.1. */
    private static final Set<Class<? extends Status>> ALL_STATUS_KINDS = allStatusKinds();

    private static Set<Class<? extends Status>> allStatusKinds() {
        Set<Class<? extends Status>> kinds = new LinkedHashSet<>();
        Collections.addAll(kinds,
                InconsistentTopicStatus.class,
                OfferedDeadlineMissedStatus.class,
                RequestedDeadlineMissedStatus.class,
                OfferedIncompatibleQosStatus.class,
                RequestedIncompatibleQosStatus.class,
                SampleLostStatus.class,
                SampleRejectedStatus.class,
                LivelinessLostStatus.class,
                LivelinessChangedStatus.class,
                PublicationMatchedStatus.class,
                SubscriptionMatchedStatus.class,
                DataAvailableStatus.class,
                DataOnReadersStatus.class);
        return Collections.unmodifiableSet(kinds);
    }
}