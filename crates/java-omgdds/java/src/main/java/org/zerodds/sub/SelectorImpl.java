// SPDX-License-Identifier: Apache-2.0
package org.zerodds.sub;

import java.util.EnumSet;
import java.util.List;
import java.util.Set;

import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.ReadCondition;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.ViewState;

/**
 * ZeroDDS {@link DataReader.Selector} (DDS-Java-PSM 1.0 §7.7.3).
 *
 * <p>The five settings read and take select on: instance handle, next
 * instance, data state, content filter and maximum samples.
 */
public final class SelectorImpl<TYPE> implements DataReader.Selector<TYPE> {

    private final ServiceEnvironment environment;
    private InstanceHandle instance;
    private boolean nextInstance;
    private Subscriber.DataState dataState;
    private String queryExpression;
    private List<String> queryParameters;
    private int maxSamples = ResourceLimits.LENGTH_UNLIMITED;
    private ReadCondition<TYPE> condition;

    public SelectorImpl(ServiceEnvironment environment) {
        this.environment = environment;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public SelectorImpl<TYPE> instance(InstanceHandle handle) {
        this.instance = handle;
        return this;
    }

    @Override
    public SelectorImpl<TYPE> nextInstance(boolean retrieveNextInstance) {
        this.nextInstance = retrieveNextInstance;
        return this;
    }

    @Override
    public SelectorImpl<TYPE> dataState(Subscriber.DataState state) {
        this.dataState = state;
        return this;
    }

    @Override
    public SelectorImpl<TYPE> Content(String queryExpression, List<String> queryParameters) {
        this.queryExpression = queryExpression;
        this.queryParameters = queryParameters;
        return this;
    }

    @Override
    public SelectorImpl<TYPE> maxSamples(int max) {
        // DDS-Java-PSM 1.0 §7.7.3: the default is "unlimited", which is
        // ResourceLimits.LENGTH_UNLIMITED; any other negative value is no count.
        if (max < 0 && max != ResourceLimits.LENGTH_UNLIMITED) {
            throw new IllegalArgumentException(
                    "maxSamples is a count or LENGTH_UNLIMITED, got " + max);
        }
        this.maxSamples = max;
        return this;
    }

    @Override
    public InstanceHandle getInstance() {
        return instance;
    }

    @Override
    public boolean retrieveNextInstance() {
        return nextInstance;
    }

    @Override
    public Subscriber.DataState getDataState() {
        // §7.7.3: the default data state is "any", all sample, view and instance states.
        return dataState == null ? DataStateImpl.any(environment) : dataState;
    }

    @Override
    public String getQueryExpression() {
        return queryExpression;
    }

    @Override
    public List<String> getQueryParameters() {
        return queryParameters;
    }

    @Override
    public int getMaxSamples() {
        return maxSamples;
    }

    @Override
    public ReadCondition<TYPE> getCondition() {
        return condition;
    }

    /**
     * read_w_condition / take_w_condition (DDS 1.4 §2.2.2.5.3.8): a read or take with this
     * selector returns the samples the {@code ReadCondition} of the reader selects on, in
     * addition to what the other settings of the selector select.
     */
    public SelectorImpl<TYPE> condition(ReadCondition<TYPE> value) {
        this.condition = value;
        return this;
    }

    // --- The read/take the selector performs (§7.7.3) ----------------------

    @Override
    public org.omg.dds.sub.Sample.Iterator<TYPE> read() {
        return readFrom(reader, null);
    }

    @Override
    public List<Sample<TYPE>> read(List<Sample<TYPE>> samples) {
        List<Sample<TYPE>> list = samples == null ? new java.util.ArrayList<Sample<TYPE>>() : samples;
        readFrom(reader, list);
        return list;
    }

    @Override
    public org.omg.dds.sub.Sample.Iterator<TYPE> take() {
        return takeFrom(reader, null);
    }

    @Override
    public List<Sample<TYPE>> take(List<Sample<TYPE>> samples) {
        List<Sample<TYPE>> list = samples == null ? new java.util.ArrayList<Sample<TYPE>>() : samples;
        takeFrom(reader, list);
        return list;
    }

    private DataReader<TYPE> reader;

    /** The reader this selector reads from or takes from. */
    public SelectorImpl<TYPE> reader(DataReader<TYPE> reader) {
        this.reader = reader;
        return this;
    }

    @SuppressWarnings("unchecked")
    private org.omg.dds.sub.Sample.Iterator<TYPE> readFrom(
            DataReader<TYPE> reader, List<Sample<TYPE>> into) {
        if (reader instanceof DataReaderImpl) {
            return ((DataReaderImpl<TYPE>) reader).readWith(this, into);
        }
        throw new UnsupportedOperationException("reader of another implementation");
    }

    private org.omg.dds.sub.Sample.Iterator<TYPE> takeFrom(
            DataReader<TYPE> reader, List<Sample<TYPE>> into) {
        if (reader instanceof DataReaderImpl) {
            return ((DataReaderImpl<TYPE>) reader).takeWith(this, into);
        }
        throw new UnsupportedOperationException("reader of another implementation");
    }

    /** The instance states the selector selects on. */
    public Set<InstanceState> instanceStates() {
        return dataState == null
                ? EnumSet.allOf(InstanceState.class) : dataState.getInstanceStates();
    }

    /** The sample states the selector selects on. */
    public Set<SampleState> sampleStates() {
        return dataState == null
                ? EnumSet.allOf(SampleState.class) : dataState.getSampleStates();
    }

    /** The view states the selector selects on. */
    public Set<ViewState> viewStates() {
        return dataState == null
                ? EnumSet.allOf(ViewState.class) : dataState.getViewStates();
    }

    @Override
    public String toString() {
        return "Selector[" + (instance == null ? "*" : instance) + ",max=" + maxSamples + "]";
    }
}
