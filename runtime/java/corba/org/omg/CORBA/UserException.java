// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.omg.CORBA;

/**
 * The base of every IDL exception on the CORBA platform: IDL4 to Java
 * Language Mapping 1.0 Annex A.1.1, "The resulting Java class shall inherit
 * from the org.omg.corba.UserException class, which is defined as follows:
 * {@code public class UserException extends java.lang.RuntimeException {}}".
 */
public class UserException extends java.lang.RuntimeException {
    private static final long serialVersionUID = 1L;
}
