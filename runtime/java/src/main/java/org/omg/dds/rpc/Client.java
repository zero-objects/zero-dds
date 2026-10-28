// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.5.4 through §7.11.2.3: "Client is a reference type
 * and provides functions to invoke operations on a remote service
 * synchronously and asynchronously. A Client instantiates the underlying
 * DDS entities and makes them discoverable. Every Client inherits
 * publicly from '${interface}', '${interface}Async', and ClientEndpoint
 * (public virtual)."
 *
 * <p>The generated {@code <I>Client} of
 * {@code crates/lang-rules/src/emit/interface_java.rs} is that type — an
 * interface, because a Java class cannot have three supertypes and an
 * interface can — and this is the third of its three.
 */
public interface Client extends ClientEndpoint {}
