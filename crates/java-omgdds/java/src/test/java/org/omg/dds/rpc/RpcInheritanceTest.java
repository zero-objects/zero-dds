// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import org.zerodds.ddsrpc.ReplierImpl;
import org.zerodds.ddsrpc.RequesterImpl;

import org.zerodds.core.Values;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.Closeable;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.util.Arrays;
import java.util.LinkedHashSet;
import java.util.Set;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.DDSObject;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.Time;

/**
 * The two inheritances DDS-RPC 1.0 §7.11.2.2.2 and §7.11.2.2.6 fix, and
 * the [DDS-Java-PSM] §7.3.1 / §7.7.2 types they reach into.
 *
 * <p>The {@code org.omg.dds.rpc} package is compiled from
 * {@code runtime/java}, which this module's POM registers as a test
 * source root — the inheritance crosses the two trees, so the check has
 * to see both.
 */
class RpcInheritanceTest {

    /**
     * §7.11.2.2.2: "It inherits from the DDSObject interface defined in
     * [DDS-Java-PSM] and java.io.Closeable."
     */
    @Test
    void rpcEntityInheritsDdsObjectAndCloseable() {
        Set<Class<?>> supertypes =
                new LinkedHashSet<>(Arrays.asList(RPCEntity.class.getInterfaces()));
        assertEquals(
                new LinkedHashSet<>(Arrays.asList(DDSObject.class, Closeable.class)),
                supertypes,
                "RPCEntity's direct supertypes");
    }

    /**
     * [DDS-Java-PSM] §7.3.1: "All stateful types in this PSM implement an
     * interface DDSObject, through a getEnvironment method on
     * which they can provide access to the ServiceEnvironment from which
     * they are ultimately derived."
     */
    @Test
    void ddsObjectCarriesTheOneOperationTheClauseNames() {
        Method[] declared = DDSObject.class.getDeclaredMethods();
        assertEquals(1, declared.length, "DDSObject declares one operation");
        assertEquals("getEnvironment", declared[0].getName());
        assertEquals(ServiceEnvironment.class, declared[0].getReturnType());
        assertEquals(0, declared[0].getParameterCount());
        assertEquals("org.omg.dds.core", DDSObject.class.getPackage().getName());
    }

    /**
     * §7.3.1: "(ServiceEnvironment itself implements this interface; a
     * ServiceEnvironment always returns this from its
     * getEnvironment operation.)"
     */
    @Test
    void aServiceEnvironmentReturnsItself() {
        assertTrue(DDSObject.class.isAssignableFrom(ServiceEnvironment.class));
        // §7.11.2.2.3 makes the RPC runtime one of them.
        RPCRuntime runtime = RPCRuntime.over(null);
        assertSame(runtime, runtime.getEnvironment());
    }

    /**
     * §7.11.2.2.2's stated purpose — "to provide quick access to the
     * RPCRuntime object" — over the operation §7.3.1 gives the interface.
     */
    @Test
    void anEntityAnRpcRuntimeMadeReturnsThatRuntime() {
        RPCRuntime runtime = RPCRuntime.over(null);
        Requester<byte[], byte[]> requester = runtime.createRequester(RequesterParams.create());
        Replier<byte[], byte[]> replier = runtime.createReplier(ReplierParams.create());
        assertSame(runtime, requester.getEnvironment());
        assertSame(runtime, replier.getEnvironment());
    }

    /** An entity derived from no environment says so; it does not answer null. */
    @Test
    void anEntityDerivedFromNoRuntimeRaisesRatherThanReturningNull() {
        Requester<byte[], byte[]> loose = new RequesterImpl<>(RequesterParams.create());
        assertThrows(IllegalStateException.class, loose::getEnvironment);
        Replier<byte[], byte[]> looseReplier = new ReplierImpl<>(ReplierParams.create());
        assertThrows(IllegalStateException.class, looseReplier::getEnvironment);
    }

    /**
     * [DDS-Java-PSM] §7.7.2: "Each sample is represented by an instance of
     * the org.omg.dds.sub.Sample interface" — an interface, which is what
     * §7.11.2.2.6's {@code extends} needs.
     */
    @Test
    void thePsmSampleIsAnInterface() {
        assertTrue(org.omg.dds.sub.Sample.class.isInterface(),
                "org.omg.dds.sub.Sample is an interface");
        assertFalse(Modifier.isFinal(org.omg.dds.sub.Sample.class.getModifiers()));
    }

    /**
     * §7.11.2.2.6: "The rpc.Sample&lt;T&gt; interface extends
     * dds.sub.Sample&lt;T&gt; interface and defines two additional
     * operations to retrieve the SampleIdentity and the related sample
     * identity, if any."
     */
    @Test
    void rpcSampleExtendsThePsmSampleOverTheSameParameter() {
        Type[] supertypes = Sample.class.getGenericInterfaces();
        assertEquals(1, supertypes.length, "rpc.Sample's direct supertypes");
        ParameterizedType supertype = (ParameterizedType) supertypes[0];
        assertEquals(org.omg.dds.sub.Sample.class, supertype.getRawType());
        assertEquals(
                Sample.class.getTypeParameters()[0],
                supertype.getActualTypeArguments()[0],
                "dds.sub.Sample<T> over rpc.Sample<T>'s own parameter");

        // The two operations the clause adds, and no third.
        Set<String> added = new LinkedHashSet<>();
        for (Method method : Sample.class.getDeclaredMethods()) {
            if (!method.isSynthetic() && !Modifier.isStatic(method.getModifiers())) {
                added.add(method.getName());
            }
        }
        assertTrue(added.contains("identity") && added.contains("relatedIdentity"),
                "the SampleIdentity and the related sample identity: " + added);
    }

    /** The inheritance as a caller sees it: one sample, both surfaces. */
    @Test
    void anRpcSampleIsAPsmSample() {
        Sample<String> sample = Sample.of("reply", SampleIdentity.UNKNOWN, SampleIdentity.UNKNOWN);
        org.omg.dds.sub.Sample<String> psm = sample;
        assertEquals("reply", psm.getData());
        assertEquals("reply", psm.getData());
        assertEquals(Values.nil(), psm.getInstanceHandle());
        assertEquals(org.zerodds.core.TimeImpl.invalid(Values.getDefaultEnvironment()),
                psm.getSourceTimestamp());
        assertEquals(org.omg.dds.sub.SampleState.NOT_READ, psm.getSampleState());
        assertEquals(org.omg.dds.sub.ViewState.NEW, psm.getViewState());
        assertEquals(org.omg.dds.sub.InstanceState.ALIVE, psm.getInstanceState());
        assertTrue(sample.validData());
        assertEquals(SampleIdentity.UNKNOWN, sample.identity());
        assertEquals(SampleIdentity.UNKNOWN, sample.relatedIdentity());
    }
}
