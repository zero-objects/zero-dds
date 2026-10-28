// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.Duration;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.Time;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.status.Status;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;

import java.io.File;
import java.io.IOException;
import java.lang.reflect.Modifier;
import java.net.URISyntaxException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Enumeration;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.jar.JarEntry;
import java.util.jar.JarFile;
import java.util.stream.Collectors;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

/**
 * DDS-Java-PSM 1.0 §7.2.1 and §7.2.6: every type of the OMG API is an interface
 * or an abstract class, except the few concrete classes of the OMG source, and
 * the files of org.omg.dds equal the OMG source except for the recorded
 * deviations.
 */
class CoreApiShapeTest {

    /** The packages of the OMG API checked by the reflection test. */
    private static final String[] PACKAGES = {
        "core", "core/policy", "core/status", "core/event", "domain", "pub", "sub", "topic", "type",
        "type/builtin", "type/dynamic", "type/typeobject",
    };

    /**
     * The concrete classes the OMG source itself defines: two exceptions that
     * are not part of Table 7.1, the listener adapters, the type id, and the
     * constant holders nested in the type and policy interfaces.
     */
    private static final Set<String> CONCRETE_IN_THE_OMG_SOURCE = new HashSet<>(Arrays.asList(
        "org.omg.dds.core.ServiceConfigurationException",
        "org.omg.dds.core.ServiceInitializationException",
        "org.omg.dds.domain.DomainParticipantAdapter",
        "org.omg.dds.pub.PublisherAdapter",
        "org.omg.dds.pub.DataWriterAdapter",
        "org.omg.dds.sub.SubscriberAdapter",
        "org.omg.dds.sub.DataReaderAdapter",
        "org.omg.dds.topic.TopicAdapter",
        "org.omg.dds.type.typeobject.TypeId",
        "org.omg.dds.type.TypeKind$Values",
        "org.omg.dds.core.policy.DataRepresentation$Id"));

    private static List<String> classNamesUnder(String packagePath) throws IOException, URISyntaxException {
        Path root = Paths.get(Time.class.getProtectionDomain().getCodeSource().getLocation().toURI());
        List<String> names = new ArrayList<>();
        String prefix = "org/omg/dds/" + packagePath + "/";
        if (Files.isDirectory(root)) {
            Path dir = root.resolve(prefix);
            if (!Files.isDirectory(dir)) {
                return names;
            }
            try (Stream<Path> files = Files.list(dir)) {
                for (Path file : files.collect(Collectors.toList())) {
                    String name = file.getFileName().toString();
                    if (name.endsWith(".class")) {
                        names.add((prefix + name.substring(0, name.length() - 6)).replace('/', '.'));
                    }
                }
            }
        } else {
            try (JarFile jar = new JarFile(new File(root.toString()))) {
                Enumeration<JarEntry> entries = jar.entries();
                while (entries.hasMoreElements()) {
                    String entry = entries.nextElement().getName();
                    if (entry.startsWith(prefix) && entry.endsWith(".class")
                            && entry.indexOf('/', prefix.length()) < 0) {
                        names.add(entry.substring(0, entry.length() - 6).replace('/', '.'));
                    }
                }
            }
        }
        return names;
    }

    @Test
    void everyTypeOfTheOmgApiIsAnInterfaceOrAbstractExceptTheConcreteOnesOfTheOmgSource() throws Exception {
        int checked = 0;
        List<String> concrete = new ArrayList<>();
        for (String pkg : PACKAGES) {
            for (String name : classNamesUnder(pkg)) {
                if (name.endsWith("package-info")) {
                    continue;
                }
                Class<?> type = Class.forName(name, false, Time.class.getClassLoader());
                checked++;
                checkType(type, concrete);
                for (Class<?> nested : type.getDeclaredClasses()) {
                    if (Modifier.isPublic(nested.getModifiers())) {
                        checkType(nested, concrete);
                    }
                }
            }
        }
        assertTrue(checked > 100, "classes checked: " + checked);
        assertEquals(Collections.<String>emptyList(), concrete,
                "concrete classes in org.omg.dds that the OMG source does not have");
    }

    private static void checkType(Class<?> type, List<String> concrete) {
        if (type.isInterface() || type.isEnum() || type.isAnnotation()
                || Modifier.isAbstract(type.getModifiers())) {
            return;
        }
        if (CONCRETE_IN_THE_OMG_SOURCE.contains(type.getName()) || type.getSimpleName().equals("MemberId")) {
            return;
        }
        concrete.add(type.getName());
    }

    @Test
    void entitiesValueTypesPoliciesAndStatusesAreInterfacesOrAbstractClasses() {
        for (Class<?> entity : new Class<?>[] {DomainParticipant.class, DomainParticipantFactory.class,
                Publisher.class, Subscriber.class, Topic.class, DataWriter.class, DataReader.class}) {
            assertTrue(entity.isInterface() || Modifier.isAbstract(entity.getModifiers()), entity.getName());
        }
        for (Class<?> value : new Class<?>[] {Time.class, Duration.class, InstanceHandle.class}) {
            assertTrue(Modifier.isAbstract(value.getModifiers()), value.getName());
        }
        assertTrue(QosPolicy.class.isInterface());
        assertTrue(Modifier.isAbstract(Status.class.getModifiers()) || Status.class.isInterface());
        assertTrue(Modifier.isAbstract(org.omg.dds.core.status.LivelinessChangedStatus.class.getModifiers()));
        assertTrue(Modifier.isAbstract(org.omg.dds.core.status.PublicationMatchedStatus.class.getModifiers()));
        assertTrue(org.omg.dds.core.policy.Reliability.class.isInterface());
        assertTrue(org.omg.dds.core.policy.History.class.isInterface());
    }

    // ---- the files of org.omg.dds against the OMG source -------------------

    /** The files of org.omg.dds that deviate from the OMG source: none. */
    private static final Set<String> DEVIATIONS = new HashSet<>();

    /** The text of a source file without comments and white space. */
    private static String code(Path file) throws IOException {
        String text = new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
        return text.replaceAll("(?s)/\\*.*?\\*/", "").replaceAll("//[^\\n]*", "").replaceAll("\\s+", "");
    }

    private static Path findOmgSource() {
        Path dir = Paths.get("").toAbsolutePath();
        while (dir != null) {
            Path candidate = dir.resolve(".agent/omg-java/srcJava/org/omg/dds");
            if (Files.isDirectory(candidate)) {
                return candidate;
            }
            dir = dir.getParent();
        }
        return null;
    }

    private static List<String> javaFiles(Path root) throws IOException {
        try (Stream<Path> walk = Files.walk(root)) {
            return walk.filter(p -> p.toString().endsWith(".java"))
                    .map(p -> root.relativize(p).toString().replace(File.separatorChar, '/'))
                    .sorted().collect(Collectors.toList());
        }
    }

    @Test
    void theFilesOfOrgOmgDdsEqualTheOmgSourceExceptForTheRecordedDeviations() throws Exception {
        Path omg = findOmgSource();
        Path ours = Paths.get("src/main/java/org/omg/dds").toAbsolutePath();
        // the OMG source is not part of the repository; the test runs where it is checked out
        assumeTrue(omg != null, "the OMG Java source (.agent/omg-java/srcJava) is not present");
        assumeTrue(Files.isDirectory(ours), "the module sources are not present");
        assertEquals(javaFiles(omg), javaFiles(ours), "the set of files");
        List<String> different = new ArrayList<>();
        for (String file : javaFiles(omg)) {
            if (!code(omg.resolve(file)).equals(code(ours.resolve(file)))) {
                different.add(file);
            }
        }
        List<String> expected = new ArrayList<>(DEVIATIONS);
        Collections.sort(expected);
        assertEquals(expected, different);
    }

    /**
     * Annex B of DDS-Java-PSM 1.0: the Java source of omgdds_src.zip is normative for the
     * interfaces and the comments. The SHA-256 of each file of srcJava/org/omg/dds is in the test
     * resource {@code omgdds-srcjava.sha256}; every file of this binding has the same text (line
     * ends read as LF) except the recorded deviations. Unlike the test above, this one needs no
     * copy of the OMG source.
     */
    @Test
    void theFilesOfOrgOmgDdsHaveTheDigestsOfTheOmgSource() throws Exception {
        Path ours = Paths.get("src/main/java/org/omg/dds").toAbsolutePath();
        assumeTrue(Files.isDirectory(ours), "the module sources are not present");
        java.util.Map<String, String> expected = new java.util.TreeMap<>();
        try (java.io.InputStream in = CoreApiShapeTest.class.getResourceAsStream("/omgdds-srcjava.sha256")) {
            for (String line : new String(in.readAllBytes(), StandardCharsets.UTF_8).split("\n")) {
                if (!line.isEmpty() && !line.startsWith("#")) {
                    expected.put(line.substring(66), line.substring(0, 64));
                }
            }
        }
        assertEquals(new ArrayList<>(expected.keySet()), javaFiles(ours), "the set of files");
        java.security.MessageDigest sha = java.security.MessageDigest.getInstance("SHA-256");
        List<String> different = new ArrayList<>();
        for (String file : expected.keySet()) {
            String text = new String(Files.readAllBytes(ours.resolve(file)), StandardCharsets.UTF_8)
                    .replace("\r\n", "\n");
            StringBuilder hex = new StringBuilder();
            for (byte b : sha.digest(text.getBytes(StandardCharsets.UTF_8))) {
                hex.append(String.format("%02x", b));
            }
            if (!hex.toString().equals(expected.get(file))) {
                different.add(file);
            }
        }
        List<String> recorded = new ArrayList<>(DEVIATIONS);
        Collections.sort(recorded);
        assertEquals(recorded, different);
    }
}
