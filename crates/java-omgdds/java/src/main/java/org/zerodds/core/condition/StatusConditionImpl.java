// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.condition;

import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Set;

import org.omg.dds.core.Entity;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.status.Status;
import org.zerodds.core.StatusTrigger;

/**
 * ZeroDDS {@link StatusCondition} (DDS-Java-PSM 1.0 §7.3.7.3).
 *
 * <p>The trigger value is the Boolean OR of the changed-status flags of the
 * statuses the condition is sensitive to. §7.3.7.3 fixes the default list to
 * all statuses of DDS 1.2 §7.1.4.1.
 */
public final class StatusConditionImpl<ENTITY extends Entity<?, ?>>
implements StatusCondition<ENTITY> {

    private final ServiceEnvironment environment;
    private final ENTITY parent;
    private final StatusTrigger trigger;
    private Set<Class<? extends Status>> enabled;

    public StatusConditionImpl(ServiceEnvironment environment,
            ENTITY parent, StatusTrigger trigger) {
        this.environment = environment;
        this.parent = parent;
        this.trigger = trigger;
        this.enabled = trigger.allStatuses();
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Set<Class<? extends Status>> getEnabledStatuses() {
        synchronized (this) {
            return Collections.unmodifiableSet(new LinkedHashSet<>(enabled));
        }
    }

    @Override
    public void setEnabledStatuses(Collection<Class<? extends Status>> statuses) {
        synchronized (this) {
            this.enabled = statuses == null
                    ? trigger.allStatuses()
                    : new LinkedHashSet<>(statuses);
        }
    }

    @Override
    public ENTITY getParent() {
        return parent;
    }

    @Override
    public boolean getTriggerValue() {
        Set<Class<? extends Status>> wanted;
        synchronized (this) {
            wanted = enabled;
        }
        Set<Class<? extends Status>> triggered = trigger.triggeredStatuses();
        for (Class<? extends Status> kind : wanted) {
            if (triggered.contains(kind)) {
                return true;
            }
        }
        return false;
    }

    @Override
    public String toString() {
        return "StatusCondition[triggerValue=" + getTriggerValue() + "]";
    }
}