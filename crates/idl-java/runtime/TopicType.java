// DDS-Java-PSM marker interface (analogous to org.omg.dds.topic.TopicType).
//
// Stub for C5.4-b — full DDS-Java-PSM Service-Environment / TypeSupport
// land with C5.5. Until then, this marker lets generic DDS code
// distinguish top-level (non-`@nested`) topic types from inner aggregate
// types.
//
// See dds-java-psm-1.0 §5.x ("Marker-Type-Pattern").
package org.omg.dds.topic;

/**
 * Marker interface implemented by every IDL top-level struct that is
 * eligible as a DDS topic type (i.e., not annotated `@nested`).
 *
 * @param <T> the implementing topic-type itself (CRTP-style binding).
 */
public interface TopicType<T> {
}
