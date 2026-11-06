// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.omg.CORBA;

/**
 * The CORBA {@code Object} interface: IDL4 to Java Language Mapping 1.0
 * Annex A.1.3, "mapped to Java according to the mapping rules for
 * Interfaces – Full ... placed in the org.omg.CORBA package ... according
 * to the IDL Naming Scheme".
 *
 * <p>The operations are those of CORBA 3.3 part 1 §8.3 in the order the
 * clause lists them; {@code respository_id} there is {@code repository_id},
 * as §8.3.1.2 names it. {@code out} and {@code inout} parameters are
 * {@link org.omg.type.Holder}s (§7.1.3); the types the operations name are
 * the Java of the CORBA module ({@code orb.idl} and
 * {@code runtime/corba/CORBA_PIDL.idl}).
 */
public interface Object {
    InterfaceDef get_interface();

    boolean is_nil();

    Object duplicate();

    void release();

    boolean is_a(java.lang.String logical_type_id);

    boolean non_existent();

    boolean is_equivalent(Object other_object);

    int hash(int maximum);

    void create_request(
        Context ctx,
        java.lang.String operation,
        NVList arg_list,
        org.omg.type.Holder<NamedValue> result,
        org.omg.type.Holder<Request> req,
        int req_flag);

    Policy get_policy(int policy_type);

    java.util.List<DomainManager> get_domain_managers();

    Object set_policy_overrides(java.util.List<Policy> policies, SetOverrideType set_add)
        throws InvalidPolicies;

    Policy get_client_policy(int type);

    java.util.List<Policy> get_policy_overrides(org.omg.type.IntegerSeq types);

    boolean validate_connection(org.omg.type.Holder<java.util.List<Policy>> inconsistent_policies);

    Object get_component();

    java.lang.String repository_id();

    ORB get_orb();
}
