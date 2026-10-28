// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.RequestedOffered;
import org.omg.dds.core.policy.Presentation;

/**
 * ZeroDDS Presentation (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>The order and grouping of samples handed to the application (DDS 1.2 §7.1.3.8). Default: INSTANCE scope, neither coherent nor ordered.
 */
public final class PresentationImpl implements Presentation, RequestedOffered<Presentation>, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final Presentation.AccessScopeKind accessScope;
    private final boolean coherentAccess;
    private final boolean orderedAccess;

    public PresentationImpl(ServiceEnvironment environment,
            Presentation.AccessScopeKind accessScope,
            boolean coherentAccess,
            boolean orderedAccess) {
        this.environment = environment;
        this.accessScope = accessScope;
        this.coherentAccess = coherentAccess;
        this.orderedAccess = orderedAccess;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Presentation.AccessScopeKind getAccessScope() {
        return accessScope;
    }

    @Override
    public boolean isCoherentAccess() {
        return coherentAccess;
    }

    @Override
    public boolean isOrderedAccess() {
        return orderedAccess;
    }

    @Override
    public Presentation withAccessScope(Presentation.AccessScopeKind accessScope) {
        return new PresentationImpl(environment, accessScope, coherentAccess, orderedAccess);
    }

    @Override
    public Presentation withCoherentAccess(boolean coherentAccess) {
        return new PresentationImpl(environment, accessScope, coherentAccess, orderedAccess);
    }

    @Override
    public Presentation withOrderedAccess(boolean orderedAccess) {
        return new PresentationImpl(environment, accessScope, coherentAccess, orderedAccess);
    }

    @Override
    public Presentation withInstance() {
        return withAccessScope(AccessScopeKind.INSTANCE);
    }

    @Override
    public Presentation withTopic() {
        return withAccessScope(AccessScopeKind.TOPIC);
    }

    @Override
    public Presentation withGroup() {
        return withAccessScope(AccessScopeKind.GROUP);
    }

    @Override
    public int compareTo(Presentation other) {
        return accessScope.compareTo(other.getAccessScope());
    }

    /**
     * DDS 1.4 §2.2.3.6: the offered access scope must be at least the requested
     * one (INSTANCE &lt; TOPIC &lt; GROUP); a requested {@code coherent_access}
     * or {@code ordered_access} must be offered as well. The returned object
     * compares as 0 for a compatible offer and as 1 for an incompatible one.
     */
    @Override
    public Comparable<Presentation> requestedOfferedContract() {
        return offered -> {
            if (accessScope.compareTo(offered.getAccessScope()) > 0
                    || (coherentAccess && !offered.isCoherentAccess())
                    || (orderedAccess && !offered.isOrderedAccess())) {
                return 1;
            }
            return 0;
        };
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Presentation)) return false;
        Presentation other = (Presentation) o;
        return accessScope == other.getAccessScope()
                && coherentAccess == other.isCoherentAccess()
                && orderedAccess == other.isOrderedAccess();
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(accessScope, coherentAccess, orderedAccess);
    }

    @Override
    public String toString() {
        return "Presentation[scope=" + accessScope + ", coherent=" + coherentAccess
                + ", ordered=" + orderedAccess + "]";
    }

    /**
     * Java serialization (DDS-Java-PSM 1.0 §7.3.3) carries the value, not its
     * {@link ServiceEnvironment}: an environment is a runtime object and is not
     * serializable. A deserialized object belongs to the default environment of
     * this implementation.
     */
    private void readObject(java.io.ObjectInputStream in)
            throws java.io.IOException, ClassNotFoundException {
        in.defaultReadObject();
        environment = ServiceEnvironmentImpl.getDefaultEnvironment();
    }

    /** A copy by value (DDS-Java-PSM 1.0 7.3.3). */
    @Override
    public PresentationImpl clone() {
        try {
            return (PresentationImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
