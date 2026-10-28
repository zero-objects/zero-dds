// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Predicate;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.topic.MultiTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicDescription;
import org.omg.dds.type.TypeSupport;
import org.zerodds.sub.ContentFilter;

/**
 * ZeroDDS {@link MultiTopic} (DDS-DCPS 1.4 §2.2.2.3.4, Annex B).
 *
 * <p>A MultiTopic combines the samples of several topics into the samples of
 * one resulting type. Its subscription expression is the topic expression of
 * Annex B:
 *
 * <pre>
 * SELECT flight_name, x, y, z AS height
 *   FROM 'Location' NATURAL JOIN 'FlightPlan'
 *   WHERE height &lt; 1000 AND x &lt; 23
 * </pre>
 *
 * <p>{@code FROM} names the topics (all of them created by the same
 * participant), {@code NATURAL JOIN} joins them on the members they share by
 * name (see {@link MultiTopicPlan} for the exact rule and the
 * {@code zerodds.multitopic.join} setting), the field list projects and
 * renames the members, and {@code WHERE} filters the joined row with the
 * filter grammar of Annex B; a field name of the {@code WHERE} clause is a
 * member of a joined topic or an alias of the field list. The type given to
 * {@code createMultiTopic} receives the output fields by name; a member it
 * has that the expression does not produce takes the default of its Java
 * type.
 *
 * <p>The expression is checked when the MultiTopic is created: its syntax,
 * that every topic exists, that the members of the join have the same type,
 * that every field exists and fits the member of the result type that
 * receives it. Each fault is an {@link IllegalArgumentException}
 * (RETCODE_BAD_PARAMETER, DDS-Java-PSM 1.0 §7.3.2).
 *
 * <p>A DataReader created on a MultiTopic with a single source and the
 * projection {@code *} onto that source's own type reads that topic with the
 * {@code WHERE} clause as content filter. Any other MultiTopic is evaluated
 * by a {@link MultiTopicEngine} that belongs to the reader.
 */
public final class MultiTopicImpl<TYPE> implements MultiTopic<TYPE> {

    private final ServiceEnvironment environment;
    private final String name;
    private final TypeSupport<TYPE> typeSupport;
    private final DomainParticipant parent;
    private final String subscriptionExpression;
    private final MultiTopicExpression expression;

    private volatile List<String> parameters;
    private volatile Predicate<Object> rowFilter;
    private volatile List<Topic<?>> sources = java.util.Collections.emptyList();
    private volatile MultiTopicPlan plan;
    private volatile boolean direct;
    private volatile boolean closed = false;
    private final java.util.concurrent.atomic.AtomicInteger readers =
            new java.util.concurrent.atomic.AtomicInteger();
    private final List<Runnable> closeHooks = new java.util.concurrent.CopyOnWriteArrayList<>();

    private MultiTopicImpl(ServiceEnvironment environment, String name,
            TypeSupport<TYPE> typeSupport, DomainParticipant parent,
            String subscriptionExpression, List<String> parameters) {
        this.environment = environment;
        this.name = name;
        this.typeSupport = typeSupport;
        this.parent = parent;
        this.subscriptionExpression = subscriptionExpression;
        this.expression = MultiTopicExpression.parse(subscriptionExpression);
        this.parameters = parameters == null
                ? java.util.Collections.<String>emptyList()
                : java.util.Collections.unmodifiableList(new ArrayList<>(parameters));
        this.rowFilter = compile(this.parameters);
    }

    /**
     * A MultiTopic of {@code subscriptionExpression} with the result type
     * {@code typeSupport}. The topics of the {@code FROM} clause are bound
     * with {@link #bind(List)} before the MultiTopic is used.
     *
     * @throws IllegalArgumentException if the expression does not parse.
     */
    public static <TYPE> MultiTopicImpl<TYPE> of(ServiceEnvironment environment,
            String name, TypeSupport<TYPE> typeSupport,
            DomainParticipant parent, String subscriptionExpression,
            List<String> parameters) {
        return new MultiTopicImpl<>(environment, name, typeSupport, parent,
                subscriptionExpression, parameters);
    }

    // --- TopicDescription ---------------------------------------------------

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public TypeSupport<TYPE> getTypeSupport() {
        return typeSupport;
    }

    @SuppressWarnings("unchecked")
    @Override
    public <OTHER> TopicDescription<OTHER> cast() {
        return (TopicDescription<OTHER>) this;
    }

    @Override
    public String getTypeName() {
        return typeSupport.getTypeName();
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public DomainParticipant getParent() {
        return parent;
    }

    /**
     * DDS 1.4 §2.2.2.3.4 delete_multitopic: PRECONDITION_NOT_MET while a DataReader still
     * reads it. The join of a reader belongs to the reader and ends with it (§7.2.3); the
     * topics of the FROM clause are released.
     */
    @Override
    public void close() {
        if (closed) {
            return;
        }
        if (readers.get() > 0) {
            throw new org.zerodds.core.PreconditionNotMetException("the MultiTopic " + name
                    + " is still used by " + readers.get() + " DataReader");
        }
        closed = true;
        for (Topic<?> source : sources) {
            if (source instanceof TopicImpl) {
                ((TopicImpl<?>) source).release();
            }
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

    /** A DataReader reads this MultiTopic. */
    public void use() {
        if (closed) {
            throw new org.zerodds.core.AlreadyClosedException("MultiTopic is closed");
        }
        readers.incrementAndGet();
    }

    /** That DataReader was closed. */
    public void release() {
        readers.decrementAndGet();
    }

    // --- MultiTopic ---------------------------------------------------------

    @Override
    public String getSubscriptionExpression() {
        return subscriptionExpression;
    }

    @Override
    public List<String> getExpressionParameters() {
        return java.util.Collections.unmodifiableList(new ArrayList<>(parameters));
    }

    @Override
    public void setExpressionParameters(List<String> expressionParameters) {
        // §7.4.2: the expression is compiled again, so a parameter set that
        // no longer fits is refused and the old one is kept.
        List<String> updated = expressionParameters == null
                ? java.util.Collections.<String>emptyList()
                : java.util.Collections.unmodifiableList(new ArrayList<>(expressionParameters));
        MultiTopicPlan current = plan;
        if (current != null) {
            // The names the WHERE clause reads are re-checked with the new
            // parameters (a parameter index out of range is the fault).
            MultiTopicPlan.of(expression, sources, current.resultClass, updated);
        }
        Predicate<Object> compiled = compile(updated);
        this.parameters = updated;
        this.rowFilter = compiled;
    }

    private Predicate<Object> compile(List<String> expressionParameters) {
        return ContentFilter.compile(expression.getWhere(), expressionParameters);
    }

    // --- What a reader on this MultiTopic evaluates -------------------------

    /** The parsed subscription expression. */
    public MultiTopicExpression getExpression() {
        return expression;
    }

    /** The names of the topics the expression reads. */
    public List<String> sourceNames() {
        return expression.getTopics();
    }

    /**
     * Binds the topics the {@code FROM} clause names, in clause order, and
     * checks the expression against them and the result type.
     *
     * @throws IllegalArgumentException if the expression does not fit the
     *         topics or the result type.
     */
    public void bind(List<Topic<?>> topics) {
        MultiTopicPlan checked = MultiTopicPlan.of(expression, topics,
                typeSupport.getType(), parameters);
        this.sources = java.util.Collections.unmodifiableList(new ArrayList<>(topics));
        for (Topic<?> source : topics) {
            if (source instanceof TopicImpl) {
                // a source topic cannot be deleted while this MultiTopic exists
                ((TopicImpl<?>) source).use();
            }
        }
        this.plan = checked;
        this.direct = expression.isStar() && topics.size() == 1
                && topics.get(0).getTypeSupport().getType().equals(typeSupport.getType());
    }

    /** The topics of the {@code FROM} clause, in clause order. */
    public List<Topic<?>> getSources() {
        return sources;
    }

    /**
     * Whether a reader reads the single source directly and applies
     * {@link #evaluate} as a content filter ({@code SELECT * FROM <topic>}
     * onto that topic's own type).
     */
    public boolean isDirect() {
        return direct;
    }

    /** The source topic of a {@linkplain #isDirect direct} MultiTopic. */
    public Topic<?> source() {
        return direct ? sources.get(0) : null;
    }

    /** Whether a sample of the source topic is part of the result (direct form). */
    public boolean evaluate(TYPE sample) {
        return rowFilter.test(sample);
    }

    /** The {@code WHERE} clause as a predicate over a row (the current parameters). */
    Predicate<Object> rowFilter() {
        return rowFilter;
    }

    /**
     * The join for one reader: the engine that forms the result samples. The
     * caller creates the reader on {@link MultiTopicEngine#resultTopic()} and
     * then calls {@link MultiTopicEngine#start}.
     */
    public MultiTopicEngine<TYPE> newEngine(org.zerodds.sub.SubscriberImpl subscriber,
            int domainId, org.zerodds.cdr.TopicTypeSupport<TYPE> codec) {
        MultiTopicPlan current = plan;
        if (current == null) {
            throw new IllegalStateException("the MultiTopic " + name
                    + " is not bound to its topics");
        }
        return new MultiTopicEngine<>(this, current, subscriber, domainId, codec,
                parent, environment);
    }

    @Override
    public String toString() {
        return "MultiTopic[" + name + "," + subscriptionExpression + "]";
    }
}
