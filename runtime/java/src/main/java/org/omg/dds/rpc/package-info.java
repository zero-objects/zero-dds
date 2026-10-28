// SPDX-License-Identifier: Apache-2.0
/**
 * The Java request-reply and function-call language bindings of
 * DDS-RPC 1.0 §7.11.2.
 *
 * <p>§7.11.2.2.1 fixes the surface: "The {@code org.omg.dds}, and
 * {@code org.omg.dds.rpc} packages define the interfaces for the
 * request-reply style language binding. Specifically, the language
 * binding includes Requester, Replier, ServiceProxy, RPCEntity,
 * RPCRuntime, SimpleReplierListener, ReplierListener,
 * SimpleRequesterListener, RequesterListener, RequesterParams,
 * ReplierParams, Future, FutureCompletionListener, Sample, and
 * Sample.Iterator." §7.11.2.3 adds the five function-call types of
 * §7.11.1.5: Service, ServiceEndpoint, Server, Client and
 * ClientEndpoint. Every one of them is in this package.
 *
 * <h2>The two inheritances the clauses name</h2>
 *
 * <p>§7.11.2.2.2 has {@code RPCEntity} inherit "from the DDSObject
 * interface defined in [DDS-Java-PSM] and java.io.Closeable".
 * {@link org.omg.dds.core.DDSObject} is [DDS-Java-PSM] §7.3.1's — "All
 * stateful types in this PSM implement an interface DDSObject, through a
 * getEnvironment method on which they can provide access to the
 * ServiceEnvironment from which they are ultimately derived" — and it
 * lives in {@code crates/java-omgdds}, in the {@code org.omg.dds.core}
 * package that same clause maps the Infrastructure Module to.
 * {@link org.omg.dds.rpc.RPCEntity} extends both, and carries the two
 * operations §7.11.1.4.2 names — {@code close()} and {@code isNull()}.
 * §7.11.2.2.3 makes {@link org.omg.dds.rpc.RPCRuntime} a
 * {@code ServiceEnvironment}, so an entity a runtime created returns that
 * runtime from {@code getEnvironment} — which is the "quick access
 * to the RPCRuntime object" §7.11.2.2.2 gives as the interface's purpose.
 *
 * <p>§7.11.2.2.6 has {@code rpc.Sample<T>} extend
 * {@code dds.sub.Sample<T>}, and [DDS-Java-PSM] §7.7.2 declares that
 * supertype an interface: "Each sample is represented by an instance of
 * the org.omg.dds.sub.Sample interface." {@code org.omg.dds.sub.Sample}
 * is that interface, and {@link org.omg.dds.rpc.Sample} extends it and
 * adds the two identity operations of the clause.
 *
 * <h2>What the entities are</h2>
 *
 * <p>§7.11.1.4.23's {@code dds_entity_traits} exists so the binding is
 * written "irrespective of the DDS C++ language bindings"; the Java
 * binding needs the same seam and takes it in the same place.
 * {@link org.omg.dds.rpc.DomainParticipant},
 * {@link org.omg.dds.rpc.DataWriter} and
 * {@link org.omg.dds.rpc.DataReader} are that seam, and they are
 * deliberately not {@code org.omg.dds.pub.DataWriter} and
 * {@code org.omg.dds.sub.DataReader}: those are typed over a sample and
 * these carry the octets of §7.5.1.1.1's framing.
 *
 * <p>The C++ counterpart is {@code runtime/cpp/include/dds/rpc.hpp},
 * and the two are held against each other by
 * {@code crates/rpc/tests/java_rpc_runtime.rs}.
 */
package org.omg.dds.rpc;
