// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.4.6 through §7.11.2.2.1: "ListenerBase is a
 * 'marker' abstract base class, which all generic listener abstract
 * classes inherit from. It defines no member functions."
 */
public interface ListenerBase extends java.util.EventListener {}
