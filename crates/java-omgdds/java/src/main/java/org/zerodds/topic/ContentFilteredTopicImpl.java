// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Predicate;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicDescription;
import org.omg.dds.type.TypeSupport;
import org.zerodds.core.AlreadyClosedException;
import org.zerodds.core.PreconditionNotMetException;
import org.zerodds.sub.ContentFilter;

/**
 * ZeroDDS {@link ContentFilteredTopic} (DDS-DCPS 1.4 §2.2.2.3.3).
 *
 * <p>A content-filtered topic lets a {@code DataReader} subscribe to a
 * subset of the samples of the related topic: only samples whose content
 * satisfies the filter expression are delivered. The filter is evaluated
 * reader-side, on the deserialized sample.
 */
public final class ContentFilteredTopicImpl<TYPE>
implements ContentFilteredTopic<TYPE> {

    private final ServiceEnvironment environment;
    private final String name;
    private final Topic<? extends TYPE> relatedTopic;
    private final String filterExpression;
    private volatile Predicate<TYPE> filter;
    private volatile List<String> expressionParameters;

    private volatile boolean closed = false;
    private final java.util.concurrent.atomic.AtomicInteger readers =
            new java.util.concurrent.atomic.AtomicInteger();
    private final List<Runnable> closeHooks = new java.util.concurrent.CopyOnWriteArrayList<>();

    public ContentFilteredTopicImpl(ServiceEnvironment environment,
            String name, Topic<? extends TYPE> relatedTopic,
            String filterExpression,
            List<String> expressionParameters,
            Predicate<TYPE> filter) {
        this.environment = environment;
        this.name = name;
        this.relatedTopic = relatedTopic;
        this.filterExpression = filterExpression;
        this.expressionParameters = expressionParameters == null
                ? java.util.Collections.emptyList() : java.util.Collections.unmodifiableList(new java.util.ArrayList<>(expressionParameters));
        this.filter = filter;
        if (relatedTopic instanceof TopicImpl) {
            // the related Topic cannot be deleted while this filtered topic exists
            ((TopicImpl<?>) relatedTopic).use();
        }
    }

    // --- TopicDescription ---------------------------------------------------

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public TypeSupport<TYPE> getTypeSupport() {
        @SuppressWarnings("unchecked")
        TypeSupport<TYPE> support = (TypeSupport<TYPE>) relatedTopic.getTypeSupport();
        return support;
    }

    @SuppressWarnings("unchecked")
    @Override
    public <OTHER> TopicDescription<OTHER> cast() {
        return (TopicDescription<OTHER>) this;
    }

    @Override
    public String getTypeName() {
        return relatedTopic.getTypeName();
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public DomainParticipant getParent() {
        return relatedTopic.getParent();
    }

    /**
     * DDS 1.4 §2.2.2.3.3 delete_contentfilteredtopic: PRECONDITION_NOT_MET while a DataReader
     * still reads through it. The related Topic is not closed (§7.2.3): the application closes
     * it through its own handle, which it can once this filtered topic is gone.
     */
    @Override
    public void close() {
        if (closed) {
            return;
        }
        if (readers.get() > 0) {
            throw new PreconditionNotMetException("the ContentFilteredTopic " + name
                    + " is still used by " + readers.get() + " DataReader");
        }
        closed = true;
        if (relatedTopic instanceof TopicImpl) {
            ((TopicImpl<?>) relatedTopic).release();
        }
        for (Runnable hook : closeHooks) {
            hook.run();
        }
        closeHooks.clear();
    }

    public boolean isClosed() {
        return closed;
    }

    /** What the containing participant does when this topic is closed. */
    public void addCloseHook(Runnable hook) {
        closeHooks.add(hook);
    }

    /** A DataReader reads through this filtered topic. */
    public void use() {
        if (closed) {
            throw new AlreadyClosedException("ContentFilteredTopic is closed");
        }
        readers.incrementAndGet();
    }

    /** That DataReader was closed. */
    public void release() {
        readers.decrementAndGet();
    }

    // --- ContentFilteredTopic ------------------------------------------------

    @Override
    public String getFilterExpression() {
        return filterExpression;
    }

    @Override
    public List<String> getExpressionParameters() {
        return expressionParameters;
    }

    /**
     * DDS 1.4 §2.2.2.3.3.4 set_expression_parameters: the filter is compiled again with the
     * new parameters, so the samples read from now on are filtered by them. A parameter list
     * the expression cannot take (a missing %n) is refused and the old one stays.
     */
    @Override
    public void setExpressionParameters(List<String> expressionParameters) {
        List<String> updated = expressionParameters == null
                ? java.util.Collections.<String>emptyList()
                : java.util.Collections.unmodifiableList(new ArrayList<>(expressionParameters));
        Predicate<TYPE> compiled = ContentFilter.compile(filterExpression, updated);
        this.filter = compiled;
        this.expressionParameters = updated;
    }

    @Override
    public Topic<? extends TYPE> getRelatedTopic() {
        return relatedTopic;
    }

    /** Whether a deserialized sample satisfies the filter expression. */
    public boolean evaluate(TYPE sample) {
        return filter.test(sample);
    }

    @Override
    public String toString() {
        return "ContentFilteredTopic[" + name + "," + filterExpression + "]";
    }
}