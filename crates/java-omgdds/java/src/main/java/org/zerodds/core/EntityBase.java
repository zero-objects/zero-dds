// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import java.util.Collection;
import java.util.EventListener;
import java.util.LinkedHashSet;
import java.util.Set;

import org.omg.dds.core.Entity;
import org.omg.dds.core.EntityQos;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.QosProvider;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.status.Status;
import org.zerodds.core.condition.StatusConditionImpl;

/**
 * The ZeroDDS half of every DDS entity (DDS-Java-PSM 1.0 §7.3.6): the
 * environment the entity belongs to, its instance handle, its listener and
 * its status condition.
 *
 * <p>§7.3.1 makes the {@link ServiceEnvironment} the root every object is
 * created from, so an entity holds the environment it came from rather than
 * looking one up. §7.3.7.2 makes the listener an {@link EventListener} that
 * is told only about the status changes of its mask; a {@code null} mask
 * means all of them.
 *
 * @param <LISTENER>    the listener interface of the entity
 * @param <QOS>         the QoS interface of the entity
 */
public abstract class EntityBase<LISTENER extends EventListener, QOS extends EntityQos<?>>
implements Entity<LISTENER, QOS>, StatusTrigger {

    private final ServiceEnvironment environment;
    private final InstanceHandle handle;
    private final Object statusLock = new Object();
    private final Set<Class<? extends Status>> triggered = new LinkedHashSet<>();

    private final java.util.List<Runnable> closeHooks =
            new java.util.concurrent.CopyOnWriteArrayList<>();

    private volatile LISTENER listener;
    private volatile Set<Class<? extends Status>> listenerStatuses;
    private volatile StatusConditionImpl<EntityBase<LISTENER, QOS>> statusCondition;

    protected EntityBase(ServiceEnvironment environment) {
        this.environment = environment;
        this.handle = Values.ofUuid(environment);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public InstanceHandle getInstanceHandle() {
        return handle;
    }

    // --- Listener (§7.3.7.2) ----------------------------------------------

    @Override
    public LISTENER getListener() {
        return listener;
    }

    @Override
    public void setListener(LISTENER listener) {
        this.listener = listener;
        this.listenerStatuses = null;
    }

    @Override
    public void setListener(LISTENER listener,
            Collection<Class<? extends Status>> statuses) {
        this.listener = listener;
        this.listenerStatuses = statuses == null
                ? null : new LinkedHashSet<>(statuses);
    }

    /**
     * Whether the listener is to be told about a status change.
     *
     * <p>DDS-Java-PSM 1.0 §7.3.7.2: a {@code null} collection signifies all
     * status changes, an empty one none.
     */
    protected final boolean listenerWants(Class<? extends Status> kind) {
        Set<Class<? extends Status>> mask = listenerStatuses;
        return mask == null || mask.contains(kind);
    }

    /**
     * Registers what the entity that contains this one does when this one is closed (it
     * forgets it, and the built-in topics retract it).
     */
    public final void addCloseHook(Runnable hook) {
        closeHooks.add(hook);
    }

    /** Runs the close hooks once; called by the entity when it passes into the closed state. */
    protected final void runCloseHooks() {
        for (Runnable hook : closeHooks) {
            hook.run();
        }
        closeHooks.clear();
    }

    /**
     * The entity's own listener if one is set and takes {@code kind}, else {@code null}.
     * The containing entity's listener is looked up by the entity classes
     * (see {@link ListenerUp}).
     */
    public final LISTENER ownListener(Class<? extends Status> kind) {
        LISTENER own = listener;
        return own != null && listenerWants(kind) ? own : null;
    }

    // --- QoS (§7.3.6) -----------------------------------------------------

    @Override
    public void setQos(String qosLibraryName, String qosProfileName) {
        QosProvider provider =
                QosProvider.newQosProvider(qosLibraryName, qosProfileName, environment);
        setQos(profileQos(provider));
    }

    /** The entity QoS the named profile of the library holds. */
    protected abstract QOS profileQos(QosProvider provider);

    // --- Status (§7.3.7.3) ------------------------------------------------

    @Override
    public StatusCondition<?> getStatusCondition() {
        StatusConditionImpl<EntityBase<LISTENER, QOS>> condition = statusCondition;
        if (condition == null) {
            synchronized (this) {
                condition = statusCondition;
                if (condition == null) {
                    condition = new StatusConditionImpl<>(environment, this, this);
                    statusCondition = condition;
                }
            }
        }
        return condition;
    }

    @Override
    public Set<Class<? extends Status>> getStatusChanges() {
        synchronized (statusLock) {
            // DDS 1.4 §2.2.2.1.1.2: the set of changed statuses; it is not cleared here,
            // a status is reset when its get_*_status is called.
            return new LinkedHashSet<>(triggered);
        }
    }

    @Override
    public Set<Class<? extends Status>> triggeredStatuses() {
        synchronized (statusLock) {
            return new LinkedHashSet<>(triggered);
        }
    }

    @Override
    public Set<Class<? extends Status>> allStatuses() {
        return Status.allStatuses(environment);
    }

    /** The status {@code kind} was read by the application: it is no longer changed. */
    protected final void clearStatus(Class<? extends Status> kind) {
        synchronized (statusLock) {
            triggered.remove(kind);
        }
    }

    /** Records a change of a communication status (DDS 1.2 §7.1.4.1). */
    protected final void triggerStatus(Class<? extends Status> kind) {
        if (kind == null) {
            return;
        }
        synchronized (statusLock) {
            triggered.add(kind);
        }
        // A StatusCondition on this entity may have become true.
        org.zerodds.core.condition.WaitSetImpl.conditionChanged();
    }

    @Override
    public void retain() {
        // §7.3.6: an entity the application still holds a reference to is in
        // use; ZeroDDS never closes an entity on its own.
    }

    /** The environment this entity was created from (§7.3.1). */
    protected final ServiceEnvironment env() {
        return environment;
    }
}