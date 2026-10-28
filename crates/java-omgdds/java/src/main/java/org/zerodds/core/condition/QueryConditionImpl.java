// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.condition;

import java.util.Collections;
import java.util.List;
import java.util.Set;
import java.util.function.BooleanSupplier;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.QueryCondition;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;

/**
 * ZeroDDS {@link QueryCondition} (DDS-Java-PSM 1.0 §7.3.7.3): a
 * {@link ReadConditionImpl} that additionally filters the samples by a query
 * expression over their content (DDS 1.2 §7.1.2.5.5.4).
 */
public final class QueryConditionImpl<TYPE>
extends ReadConditionImpl<TYPE>
implements QueryCondition<TYPE> {

    private final String queryExpression;
    private volatile List<String> queryParameters;

    public QueryConditionImpl(ServiceEnvironment environment,
            DataReader<TYPE> parent,
            Set<SampleState> sampleStates,
            Set<ViewState> viewStates,
            Set<InstanceState> instanceStates,
            String queryExpression,
            List<String> queryParameters,
            BooleanSupplier trigger) {
        super(environment, parent, sampleStates, viewStates, instanceStates, trigger);
        this.queryExpression = queryExpression;
        this.queryParameters = queryParameters == null
                ? null : java.util.Collections.unmodifiableList(new java.util.ArrayList<>(queryParameters));
    }

    @Override
    public String getQueryExpression() {
        return queryExpression;
    }

    @Override
    public List<String> getQueryParameters() {
        List<String> current = queryParameters;
        return current == null
                ? Collections.emptyList()
                : Collections.unmodifiableList(current);
    }

    @Override
    public void setQueryParameters(List<String> queryParameters) {
        ensureOpen();
        this.queryParameters = queryParameters == null
                ? null : java.util.Collections.unmodifiableList(new java.util.ArrayList<>(queryParameters));
        // the trigger value may have changed: a blocked wait set evaluates its conditions again
        WaitSetImpl.conditionChanged();
    }

    @Override
    public String toString() {
        return "QueryCondition[" + queryExpression + "]";
    }
}