// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.topic;

import java.util.function.Predicate;

/**
 * OMG DDS ContentFilteredTopic — DDS-DCPS 1.4 §2.2.2.3.3.
 *
 * <p>A ContentFilteredTopic is a specialization of a Topic that allows a
 * DataReader to subscribe to a subset of the samples published on the
 * related Topic: only samples whose content satisfies the
 * {@code filter_expression} are delivered (§2.2.2.3.3 — "a more
 * sophisticated subscription that filters by data content").
 *
 * <p>The pure-Java PSM models the filter as a typed {@link Predicate} over
 * the deserialized sample plus the literal expression string (for SQL-grammar
 * round-tripping / discovery). The filter is evaluated reader-side: samples
 * that fail the predicate are discarded before entering the reader cache.
 *
 * @param <T> the related Topic's data type
 */
public final class ContentFilteredTopic<T> {
    private final String name;
    private final Topic<T> relatedTopic;
    private final String filterExpression;
    private final Predicate<T> filter;

    public ContentFilteredTopic(String name, Topic<T> relatedTopic,
                                String filterExpression, Predicate<T> filter) {
        this.name = name;
        this.relatedTopic = relatedTopic;
        this.filterExpression = filterExpression;
        this.filter = filter;
    }

    public String getName() {
        return name;
    }

    /** The Topic this filter is layered on (§2.2.2.3.3 {@code related_topic}). */
    public Topic<T> getRelatedTopic() {
        return relatedTopic;
    }

    /** The SQL-grammar filter expression string (§2.2.2.3.3). */
    public String getFilterExpression() {
        return filterExpression;
    }

    /** Evaluate the filter against a deserialized sample. */
    public boolean evaluate(T sample) {
        return filter.test(sample);
    }
}
