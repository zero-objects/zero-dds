// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.omg.dds.core.QosProvider;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.zerodds.internal.InProcessBus;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * {@code QosProvider} (DDS-Java-PSM 1.0 §7.3.5.4): the QoS of the profiles of an XML QoS library
 * in the syntax of DDS-XML 1.0 §7.3.2, read from a {@code file://} URI; any other scheme is
 * {@code UnsupportedOperationException}. Domain ids 1400 to 1499.
 */
class QosProviderTest {

    @TempDir
    Path dir;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    /** "raw_bytes", "topic_data" and "Group" in base64; {@code AQID} is the octets 1, 2, 3. */
    private static final String LIBRARY = """
            <?xml version="1.0" encoding="UTF-8"?>
            <dds>
              <qos_library name="Lib">
                <qos_profile name="Base">
                  <domain_participant_qos name="p">
                    <user_data><value>cmF3X2J5dGVz</value></user_data>
                    <entity_factory><autoenable_created_entities>false</autoenable_created_entities></entity_factory>
                  </domain_participant_qos>
                  <topic_qos name="t">
                    <topic_data><value>dG9waWNfZGF0YQ==</value></topic_data>
                    <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
                    <deadline><period><sec>3</sec><nanosec>500</nanosec></period></deadline>
                    <resource_limits>
                      <max_samples>100</max_samples>
                      <max_instances>LENGTH_UNLIMITED</max_instances>
                      <max_samples_per_instance>10</max_samples_per_instance>
                    </resource_limits>
                    <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>5</depth></history>
                  </topic_qos>
                  <publisher_qos name="pub">
                    <presentation>
                      <access_scope>GROUP_PRESENTATION_QOS</access_scope>
                      <coherent_access>true</coherent_access>
                      <ordered_access>1</ordered_access>
                    </presentation>
                    <partition><name><element>A</element><element>B</element></name></partition>
                    <group_data><value>R3JvdXA=</value></group_data>
                  </publisher_qos>
                  <subscriber_qos name="sub">
                    <partition><name><element>S</element></name></partition>
                  </subscriber_qos>
                  <datawriter_qos name="w">
                    <reliability>
                      <kind>RELIABLE_RELIABILITY_QOS</kind>
                      <max_blocking_time><sec>1</sec><nanosec>0</nanosec></max_blocking_time>
                    </reliability>
                    <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
                    <ownership><kind>EXCLUSIVE_OWNERSHIP_QOS</kind></ownership>
                    <ownership_strength><value>42</value></ownership_strength>
                    <lifespan><duration><sec>DURATION_INFINITY</sec></duration></lifespan>
                    <writer_data_lifecycle>
                      <autodispose_unregistered_instances>false</autodispose_unregistered_instances>
                    </writer_data_lifecycle>
                    <durability_service>
                      <service_cleanup_delay><sec>7</sec></service_cleanup_delay>
                      <history_kind>KEEP_ALL_HISTORY_QOS</history_kind>
                      <history_depth>1</history_depth>
                      <max_samples>LENGTH_UNLIMITED</max_samples>
                      <max_instances>4</max_instances>
                    </durability_service>
                    <transport_priority><value>3</value></transport_priority>
                    <latency_budget><duration><sec>0</sec><nanosec>20000000</nanosec></duration></latency_budget>
                    <liveliness>
                      <kind>MANUAL_BY_TOPIC_LIVELINESS_QOS</kind>
                      <lease_duration><sec>2</sec></lease_duration>
                    </liveliness>
                    <destination_order><kind>BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS</kind></destination_order>
                    <user_data><value>AQID</value></user_data>
                  </datawriter_qos>
                  <datareader_qos name="r">
                    <time_based_filter><minimum_separation><sec>0</sec><nanosec>250000000</nanosec></minimum_separation></time_based_filter>
                    <reader_data_lifecycle>
                      <autopurge_nowriter_samples_delay><sec>4</sec></autopurge_nowriter_samples_delay>
                      <autopurge_disposed_samples_delay><sec>DURATION_INFINITY</sec></autopurge_disposed_samples_delay>
                    </reader_data_lifecycle>
                  </datareader_qos>
                </qos_profile>
                <qos_profile name="Child" base_name="Base">
                  <datawriter_qos name="w">
                    <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>9</depth></history>
                    <ownership_strength><value>7</value></ownership_strength>
                  </datawriter_qos>
                  <datawriter_qos name="w2" base_name="w">
                    <ownership_strength><value>8</value></ownership_strength>
                  </datawriter_qos>
                </qos_profile>
                <qos_profile name="Live">
                  <datawriter_qos><ownership_strength><value>5</value></ownership_strength></datawriter_qos>
                  <datareader_qos>
                    <time_based_filter><minimum_separation><sec>0</sec><nanosec>100000000</nanosec></minimum_separation></time_based_filter>
                  </datareader_qos>
                  <topic_qos><lifespan><duration><sec>30</sec></duration></lifespan></topic_qos>
                  <publisher_qos><partition><name><element>X</element></name></partition></publisher_qos>
                  <subscriber_qos><partition><name><element>Y</element></name></partition></subscriber_qos>
                  <domain_participant_qos><user_data><value>AQID</value></user_data></domain_participant_qos>
                </qos_profile>
              </qos_library>
            </dds>
            """;

    private String write(String name, String content) throws IOException {
        Path file = dir.resolve(name);
        Files.write(file, content.getBytes(StandardCharsets.UTF_8));
        return file.toUri().toString();
    }

    private QosProvider provider(String profile) throws IOException {
        return QosProvider.newQosProvider(write("library.xml", LIBRARY), profile, Qos.env());
    }

    // --- the source -----------------------------------------------------------------------

    @Test
    void aFileUriIsReadAndTheProviderIsObtainedFromTheEnvironment() throws IOException {
        String uri = write("library.xml", LIBRARY);
        assertTrue(uri.startsWith("file:"), uri);
        QosProvider viaStatic = QosProvider.newQosProvider(uri, "Lib::Base", Qos.env());
        QosProvider viaSpi = Qos.env().getSPI().newQosProvider(uri, "Lib::Base");
        assertNotNull(viaStatic);
        assertNotNull(viaSpi);
        assertEquals(viaStatic.getDataWriterQos(), viaSpi.getDataWriterQos());
        assertEquals(Qos.env(), viaStatic.getEnvironment());
    }

    @Test
    void anotherSchemeIsUnsupported() {
        assertThrows(UnsupportedOperationException.class,
                () -> QosProvider.newQosProvider("http://example.org/qos.xml", "P", Qos.env()));
        assertThrows(UnsupportedOperationException.class,
                () -> QosProvider.newQosProvider("ftp://example.org/qos.xml", null, Qos.env()));
        assertThrows(UnsupportedOperationException.class,
                () -> QosProvider.newQosProvider("data:text/xml,%3Cdds%2F%3E", null, Qos.env()));
    }

    @Test
    void aUriThatIsNoUriAndAFileThatIsNotThereAreRefused() {
        assertThrows(IllegalArgumentException.class,
                () -> QosProvider.newQosProvider("not a uri with spaces", null, Qos.env()));
        assertThrows(IllegalArgumentException.class,
                () -> QosProvider.newQosProvider(null, null, Qos.env()));
        assertThrows(IllegalArgumentException.class, () -> QosProvider.newQosProvider(
                dir.resolve("missing.xml").toUri().toString(), null, Qos.env()));
    }

    // --- profiles ---------------------------------------------------------------------------

    @Test
    void aProfileIsSelectedByQualifiedNameByUniqueNameOrAsTheFirst() throws IOException {
        assertEquals(42, provider("Lib::Base").getDataWriterQos().getOwnershipStrength().getValue());
        assertEquals(42, provider("Base").getDataWriterQos().getOwnershipStrength().getValue());
        assertEquals(42, provider(null).getDataWriterQos().getOwnershipStrength().getValue());
        assertEquals(7, provider("Child").getDataWriterQos().getOwnershipStrength().getValue());
        assertThrows(IllegalArgumentException.class, () -> provider("Nope"));
        assertThrows(IllegalArgumentException.class, () -> provider("Other::Base"));
    }

    @Test
    void aProfileNameThatTwoProfilesShareIsNotUnique() throws IOException {
        String uri = write("twice.xml", """
                <dds>
                  <qos_library name="L1"><qos_profile name="P"><topic_qos/></qos_profile></qos_library>
                  <qos_library name="L2"><qos_profile name="P"><topic_qos/></qos_profile></qos_library>
                </dds>
                """);
        assertThrows(IllegalArgumentException.class, () -> QosProvider.newQosProvider(uri, "P", Qos.env()));
        assertNotNull(QosProvider.newQosProvider(uri, "L2::P", Qos.env()).getTopicQos());
        // the same name twice in one library is a document that names no profile uniquely
        String same = write("same.xml", """
                <dds><qos_library name="L">
                  <qos_profile name="P"><topic_qos/></qos_profile>
                  <qos_profile name="P"><topic_qos/></qos_profile>
                </qos_library></dds>
                """);
        assertThrows(IllegalArgumentException.class, () -> QosProvider.newQosProvider(same, null, Qos.env()));
    }

    // --- the QoS of every entity kind -------------------------------------------------------

    @Test
    void theQosOfEveryEntityKindCarriesTheValuesOfTheDocument() throws IOException {
        QosProvider base = provider("Lib::Base");

        org.omg.dds.domain.DomainParticipantQos participant = base.getDomainParticipantQos();
        assertArrayEquals("raw_bytes".getBytes(StandardCharsets.US_ASCII), participant.getUserData().getValue());
        assertFalse(participant.getEntityFactory().isAutoEnableCreatedEntities());
        assertEquals(participant, base.getDomainParticipantQos("p"));

        org.omg.dds.topic.TopicQos topic = base.getTopicQos();
        assertArrayEquals("topic_data".getBytes(StandardCharsets.US_ASCII), topic.getTopicData().getValue());
        assertEquals(Durability.Kind.TRANSIENT_LOCAL, topic.getDurability().getKind());
        assertEquals(3_000_000_500L, topic.getDeadline().getPeriod().getDuration(TimeUnit.NANOSECONDS));
        assertEquals(100, topic.getResourceLimits().getMaxSamples());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, topic.getResourceLimits().getMaxInstances());
        assertEquals(10, topic.getResourceLimits().getMaxSamplesPerInstance());
        assertEquals(History.Kind.KEEP_LAST, topic.getHistory().getKind());
        assertEquals(5, topic.getHistory().getDepth());
        // what the document does not say is the default of the topic
        assertEquals(Reliability.Kind.BEST_EFFORT, topic.getReliability().getKind());
        assertEquals(topic, base.getTopicQos("t"));

        org.omg.dds.pub.PublisherQos publisher = base.getPublisherQos();
        assertEquals(Presentation.AccessScopeKind.GROUP, publisher.getPresentation().getAccessScope());
        assertTrue(publisher.getPresentation().isCoherentAccess());
        assertTrue(publisher.getPresentation().isOrderedAccess());
        assertEquals(new LinkedHashSet<>(Arrays.asList("A", "B")), publisher.getPartition().getName());
        assertArrayEquals("Group".getBytes(StandardCharsets.US_ASCII), publisher.getGroupData().getValue());
        assertTrue(publisher.getEntityFactory().isAutoEnableCreatedEntities());

        org.omg.dds.sub.SubscriberQos subscriber = base.getSubscriberQos();
        assertEquals(Collections.singleton("S"), subscriber.getPartition().getName());
        assertEquals(subscriber, base.getSubscriberQos("sub"));
    }

    @Test
    void theWriterAndReaderQosCarryTheValuesOfTheDocumentAndKeepTheDefaultsOfWhatItLeavesOut() throws IOException {
        QosProvider base = provider("Lib::Base");
        DataWriterQos writer = base.getDataWriterQos();
        assertEquals(Reliability.Kind.RELIABLE, writer.getReliability().getKind());
        assertEquals(1, writer.getReliability().getMaxBlockingTime().getDuration(TimeUnit.SECONDS));
        assertEquals(History.Kind.KEEP_ALL, writer.getHistory().getKind());
        assertEquals(1, writer.getHistory().getDepth(), "the depth the document leaves out stays 1");
        assertEquals(Ownership.Kind.EXCLUSIVE, writer.getOwnership().getKind());
        assertEquals(42, writer.getOwnershipStrength().getValue());
        assertTrue(writer.getLifespan().getDuration().isInfinite());
        assertFalse(writer.getWriterDataLifecycle().isAutDisposeUnregisteredInstances());
        assertEquals(7, writer.getDurabilityService().getServiceCleanupDelay().getDuration(TimeUnit.SECONDS));
        assertEquals(History.Kind.KEEP_ALL, writer.getDurabilityService().getHistoryKind());
        assertEquals(1, writer.getDurabilityService().getHistoryDepth());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, writer.getDurabilityService().getMaxSamples());
        assertEquals(4, writer.getDurabilityService().getMaxInstances());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, writer.getDurabilityService().getMaxSamplesPerInstance());
        assertEquals(3, writer.getTransportPriority().getValue());
        assertEquals(20_000_000L, writer.getLatencyBudget().getDuration().getDuration(TimeUnit.NANOSECONDS));
        assertEquals(Liveliness.Kind.MANUAL_BY_TOPIC, writer.getLiveliness().getKind());
        assertEquals(2, writer.getLiveliness().getLeaseDuration().getDuration(TimeUnit.SECONDS));
        assertEquals(DestinationOrder.Kind.BY_SOURCE_TIMESTAMP, writer.getDestinationOrder().getKind());
        assertArrayEquals(new byte[] {1, 2, 3}, writer.getUserData().getValue());
        // a policy the document does not mention
        assertEquals(Durability.Kind.VOLATILE, writer.getDurability().getKind());
        assertTrue(writer.getDeadline().getPeriod().isInfinite());
        assertEquals(writer, base.getDataWriterQos("w"));

        DataReaderQos reader = base.getDataReaderQos();
        assertEquals(250, reader.getTimeBasedFilter().getMinimumSeparation().getDuration(TimeUnit.MILLISECONDS));
        assertEquals(4, reader.getReaderDataLifecycle().getAutoPurgeNoWriterSamplesDelay().getDuration(TimeUnit.SECONDS));
        assertTrue(reader.getReaderDataLifecycle().getAutoPurgeDisposedSamplesDelay().isInfinite());
        assertEquals(Reliability.Kind.BEST_EFFORT, reader.get(Reliability.class).getKind());
        assertEquals(reader, base.getDataReaderQos("r"));
    }

    @Test
    void aProfileWithoutAnElementOfAKindHasNoQosOfThatKind() throws IOException {
        QosProvider base = provider("Lib::Base");
        // the document has no domain_participant_factory_qos
        assertThrows(IllegalArgumentException.class, base::getDomainParticipantFactoryQos);
        assertThrows(IllegalArgumentException.class, () -> base.getDataWriterQos("nobody"));
        assertThrows(IllegalArgumentException.class, () -> base.getDataWriterQos(null));
    }

    @Test
    void theFactoryQosHasItsOwnElement() throws IOException {
        String uri = write("factory.xml", """
                <dds><qos_library name="L"><qos_profile name="P">
                  <domain_participant_factory_qos>
                    <entity_factory><autoenable_created_entities>false</autoenable_created_entities></entity_factory>
                  </domain_participant_factory_qos>
                </qos_profile></qos_library></dds>
                """);
        QosProvider provider = QosProvider.newQosProvider(uri, null, Qos.env());
        assertFalse(provider.getDomainParticipantFactoryQos().getEntityFactory().isAutoEnableCreatedEntities());
    }

    // --- inheritance -----------------------------------------------------------------------------

    @Test
    void aProfileInheritsFromItsBaseAndOverridesFieldByField() throws IOException {
        QosProvider child = provider("Lib::Child");
        DataWriterQos w = child.getDataWriterQos();
        assertEquals(7, w.getOwnershipStrength().getValue(), "overridden");
        assertEquals(History.Kind.KEEP_LAST, w.getHistory().getKind(), "overridden");
        assertEquals(9, w.getHistory().getDepth(), "overridden");
        assertEquals(Reliability.Kind.RELIABLE, w.getReliability().getKind(), "inherited");
        assertEquals(1, w.getReliability().getMaxBlockingTime().getDuration(TimeUnit.SECONDS), "inherited");
        assertEquals(Ownership.Kind.EXCLUSIVE, w.getOwnership().getKind(), "inherited");
        // the elements of the base that the child does not mention are the child's too
        assertEquals(Collections.singleton("S"), child.getSubscriberQos().getPartition().getName());
        // and the base itself is as it was
        assertEquals(42, provider("Lib::Base").getDataWriterQos().getOwnershipStrength().getValue());
        assertEquals(History.Kind.KEEP_ALL, provider("Lib::Base").getDataWriterQos().getHistory().getKind());
    }

    @Test
    void aQosElementInheritsFromAnElementOfTheSameKindByName() throws IOException {
        QosProvider child = provider("Lib::Child");
        DataWriterQos w2 = child.getDataWriterQos("w2");
        assertEquals(8, w2.getOwnershipStrength().getValue());
        assertEquals(9, w2.getHistory().getDepth(), "from w of the child, which has the 9");
        assertEquals(Reliability.Kind.RELIABLE, w2.getReliability().getKind());
    }

    @Test
    void anIdCanNameAnotherProfileOrAnElementOfIt() throws IOException {
        QosProvider base = provider("Lib::Base");
        assertEquals(7, base.getDataWriterQos("Lib::Child").getOwnershipStrength().getValue());
        assertEquals(7, base.getDataWriterQos("Child").getOwnershipStrength().getValue());
        assertEquals(8, base.getDataWriterQos("Lib::Child::w2").getOwnershipStrength().getValue());
        assertThrows(IllegalArgumentException.class, () -> base.getDataWriterQos("Lib::Child::nobody"));
        assertThrows(IllegalArgumentException.class, () -> base.getDataWriterQos("Lib::Nope"));
    }

    @Test
    void aCycleAndABaseDefinedLaterAndAMissingBaseAreRefused() throws IOException {
        String cycle = write("cycle.xml", """
                <dds><qos_library name="L">
                  <qos_profile name="A" base_name="B"><topic_qos/></qos_profile>
                  <qos_profile name="B" base_name="A"><topic_qos/></qos_profile>
                </qos_library></dds>
                """);
        assertThrows(IllegalArgumentException.class, () -> QosProvider.newQosProvider(cycle, "A", Qos.env()));
        String later = write("later.xml", """
                <dds><qos_library name="L">
                  <qos_profile name="A" base_name="B"><topic_qos/></qos_profile>
                  <qos_profile name="B"><topic_qos/></qos_profile>
                </qos_library></dds>
                """);
        assertThrows(IllegalArgumentException.class, () -> QosProvider.newQosProvider(later, "A", Qos.env()));
        String missing = write("missing-base.xml", """
                <dds><qos_library name="L">
                  <qos_profile name="A" base_name="Z"><topic_qos/></qos_profile>
                </qos_library></dds>
                """);
        assertThrows(IllegalArgumentException.class, () -> QosProvider.newQosProvider(missing, "A", Qos.env()));
        String elementMissing = write("missing-element.xml", """
                <dds><qos_library name="L">
                  <qos_profile name="A"><topic_qos name="t" base_name="nobody"/></qos_profile>
                </qos_library></dds>
                """);
        assertThrows(IllegalArgumentException.class,
                () -> QosProvider.newQosProvider(elementMissing, "A", Qos.env()));
    }

    // --- what is wrong in a document is said --------------------------------------------------------

    @Test
    void anythingTheSchemaDoesNotKnowIsRefusedAndNotIgnored() throws IOException {
        String[] bodies = {
            "<topic_qos><reliability><kind>RELIABLE</kind></reliability></topic_qos>",  // not a schema literal
            "<topic_qos><reliability><kinds>RELIABLE_RELIABILITY_QOS</kinds></reliability></topic_qos>",
            "<topic_qos><no_such_policy/></topic_qos>",
            "<topic_qos><time_based_filter/></topic_qos>",   // a reader policy, not a topic one
            "<no_such_qos/>",
            "<topic_qos><topic_data><value>%%%</value></topic_data></topic_qos>",
            "<datawriter_qos><ownership_strength><value>many</value></ownership_strength></datawriter_qos>",
            "<datawriter_qos><deadline><period><sec>soon</sec></period></deadline></datawriter_qos>",
            "<publisher_qos><presentation><coherent_access>maybe</coherent_access></presentation></publisher_qos>",
        };
        for (String body : bodies) {
            String uri = write("bad.xml", "<dds><qos_library name=\"L\"><qos_profile name=\"P\">"
                    + body + "</qos_profile></qos_library></dds>");
            assertThrows(IllegalArgumentException.class,
                    () -> QosProvider.newQosProvider(uri, null, Qos.env()), body);
        }
    }

    @Test
    void aDocumentThatIsNotWellFormedOrHasADocumentTypeIsRefused() throws IOException {
        assertThrows(IllegalArgumentException.class, () -> QosProvider.newQosProvider(
                write("broken.xml", "<dds><qos_library name=\"L\">"), null, Qos.env()));
        assertThrows(IllegalArgumentException.class, () -> QosProvider.newQosProvider(
                write("doctype.xml", "<!DOCTYPE dds [<!ENTITY x \"y\">]><dds><qos_library name=\"L\">"
                        + "<qos_profile name=\"P\"><topic_qos/></qos_profile></qos_library></dds>"),
                null, Qos.env()));
        assertThrows(IllegalArgumentException.class, () -> QosProvider.newQosProvider(
                write("empty.xml", "<dds/>"), null, Qos.env()));
    }

    // --- applying what the provider gives -------------------------------------------------------------

    @Test
    void entitySetQosWithAnUriAndAProfileAppliesTheQosOfThatProfile() throws Exception {
        String uri = write("library.xml", LIBRARY);
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.setQos(uri, "Lib::Live");
            assertEquals(5, writer.getQos().getOwnershipStrength().getValue());

            DataReader<QosFixtures.Item> reader = rig.reader(Qos.reader());
            reader.setQos(uri, "Lib::Live");
            assertEquals(100, reader.getQos().getTimeBasedFilter().getMinimumSeparation()
                    .getDuration(TimeUnit.MILLISECONDS));

            rig.topic.setQos(uri, "Lib::Live");
            assertEquals(30, rig.topic.getQos().getLifespan().getDuration().getDuration(TimeUnit.SECONDS));
            rig.publisher.setQos(uri, "Lib::Live");
            assertEquals(Collections.singleton("X"), rig.publisher.getQos().getPartition().getName());
            rig.subscriber.setQos(uri, "Lib::Live");
            assertEquals(Collections.singleton("Y"), rig.subscriber.getQos().getPartition().getName());
            rig.participant.setQos(uri, "Lib::Live");
            assertArrayEquals(new byte[] {1, 2, 3}, rig.participant.getQos().getUserData().getValue());

            // a profile that changes a policy that cannot change once enabled is refused as set_qos is
            assertThrows(org.omg.dds.core.ImmutablePolicyException.class, () -> writer.setQos(uri, "Lib::Base"));
            assertThrows(UnsupportedOperationException.class, () -> writer.setQos("http://example.org/q.xml", "P"));
        }
    }

    @Test
    void entitiesAreCreatedFromTheQosOfAProvider() throws Exception {
        QosProvider live = provider("Lib::Live");
        QosProvider base = provider("Lib::Base");
        try (QosFixtures.Rig rig = new QosFixtures.Rig(live.getPublisherQos(), live.getSubscriberQos())) {
            assertEquals(live.getPublisherQos(), rig.publisher.getQos());
            assertEquals(live.getSubscriberQos(), rig.subscriber.getQos());
            DataWriter<QosFixtures.Item> writer = rig.writer(live.getDataWriterQos());
            assertEquals(live.getDataWriterQos(), writer.getQos());
            DataWriter<QosFixtures.Item> other = rig.writer(base.getDataWriterQos("w"));
            assertEquals(base.getDataWriterQos(), other.getQos());
            DataReader<QosFixtures.Item> reader = rig.reader(live.getDataReaderQos());
            assertEquals(live.getDataReaderQos(), reader.getQos());
        }
        DomainParticipant participant = Qos.factory().createParticipant(
                QosFixtures.nextDomain(), live.getDomainParticipantQos(), null, null);
        try {
            assertEquals(live.getDomainParticipantQos(), participant.getQos());
        } finally {
            participant.close();
        }
    }
}
