// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.DDSObject;

import java.io.IOException;
import java.io.ObjectStreamClass;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;
import java.util.stream.Collectors;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 clause 2: the PDF, the JAR and the source files are normative, the JAR and
 * the sources before the PDF. The package {@code org.omg.dds} of this binding is the API of the
 * OMG source files, with the exceptions described in the test that compares them.
 */
class ParticipantOmgApiTest {

    // --- the exceptions are DDSObjects of the binding's environment -----------------------------

    @Test
    void everyExceptionOfTheBindingIsADdsObjectWithAnEnvironment() {
        Object[] exceptions = {
            new org.zerodds.core.AlreadyClosedException("x"),
            new org.zerodds.core.IllegalOperationException("x"),
            new org.zerodds.core.ImmutablePolicyException("x"),
            new org.zerodds.core.InconsistentPolicyException("x"),
            new org.zerodds.core.NotEnabledException("x"),
            new org.zerodds.core.OutOfResourcesException("x"),
            new org.zerodds.core.PreconditionNotMetException("x"),
        };
        for (Object exception : exceptions) {
            assertTrue(exception instanceof DDSObject, exception.getClass().getName());
            assertTrue(exception instanceof org.omg.dds.core.DDSException);
            assertNotNull(((DDSObject) exception).getEnvironment());
        }
    }

    @Test
    void theExceptionsHaveTheSerialVersionUidOfTheOmgSource() {
        assertEquals(-7560788483511488746L, uid(org.omg.dds.core.DDSException.class));
        assertEquals(-7676868650200410524L, uid(org.omg.dds.core.ImmutablePolicyException.class));
        assertEquals(323168673886924537L, uid(org.omg.dds.core.ServiceInitializationException.class));
        assertEquals(-6782657920034098505L, uid(org.omg.dds.core.IllegalOperationException.class));
        assertEquals(5264713016409453514L, uid(org.omg.dds.core.InconsistentPolicyException.class));
        assertEquals(-6204457556635480597L, uid(org.omg.dds.core.PreconditionNotMetException.class));
        assertEquals(-1435103296817695008L, uid(org.omg.dds.core.NotEnabledException.class));
    }

    private static long uid(Class<?> type) {
        return ObjectStreamClass.lookup(type).getSerialVersionUID();
    }

    // --- the API against the OMG source files ------------------------------------------------------

    /**
     * Compares every file of {@code org/omg/dds} with the OMG source file of the same path, with
     * comments and white space left out. The OMG sources are not part of this repository: the
     * test looks for {@code .agent/omg-java/srcJava} above the working directory, or for the
     * directory named by the system property {@code zerodds.omgdds.reference}, and is skipped
     * where there is none. The one difference allowed is the {@code @SuppressWarnings("unchecked")}
     * of {@code EntityQos.withPolicies}, which keeps the build free of lint warnings.
     */
    @Test
    void theApiIsTheOmgSourceCodeNotCountingCommentsAndWhiteSpace() throws IOException {
        Path reference = referenceSources();
        Assumptions.assumeTrue(reference != null, "the OMG source files are not available here");
        Path api = Paths.get("src/main/java/org/omg/dds").toAbsolutePath();
        Assumptions.assumeTrue(Files.isDirectory(api), "run from the module directory");

        TreeSet<String> apiFiles = javaFiles(api);
        TreeSet<String> referenceFiles = javaFiles(reference);
        assertEquals(referenceFiles, apiFiles, "the same files");

        List<String> different = new ArrayList<>();
        for (String name : apiFiles) {
            String ours = code(Files.readAllBytes(api.resolve(name)));
            String theirs = code(Files.readAllBytes(reference.resolve(name)));
            if (name.endsWith("EntityQos.java")) {
                ours = ours.replace("@SuppressWarnings(\"unchecked\")", "");
            }
            if (!ours.equals(theirs)) {
                different.add(name);
            }
        }
        assertEquals(new ArrayList<String>(), different, "files that differ from the OMG source");
    }

    private static Path referenceSources() {
        String property = System.getProperty("zerodds.omgdds.reference");
        if (property != null) {
            Path path = Paths.get(property, "org/omg/dds");
            return Files.isDirectory(path) ? path : null;
        }
        for (Path dir = Paths.get("").toAbsolutePath(); dir != null; dir = dir.getParent()) {
            Path path = dir.resolve(".agent/omg-java/srcJava/org/omg/dds");
            if (Files.isDirectory(path)) {
                return path;
            }
        }
        return null;
    }

    private static TreeSet<String> javaFiles(Path root) throws IOException {
        try (Stream<Path> walk = Files.walk(root)) {
            return walk.filter(p -> p.toString().endsWith(".java"))
                    .map(p -> root.relativize(p).toString())
                    .collect(Collectors.toCollection(TreeSet::new));
        }
    }

    /** The source text without comments and white space; string and character literals stay. */
    static String code(byte[] source) {
        String text = new String(source, StandardCharsets.UTF_8);
        StringBuilder out = new StringBuilder();
        int i = 0;
        int n = text.length();
        while (i < n) {
            char c = text.charAt(i);
            if (c == '/' && i + 1 < n && text.charAt(i + 1) == '/') {
                while (i < n && text.charAt(i) != '\n') {
                    i++;
                }
            } else if (c == '/' && i + 1 < n && text.charAt(i + 1) == '*') {
                int end = text.indexOf("*/", i + 2);
                i = end < 0 ? n : end + 2;
            } else if (c == '"' || c == '\'') {
                out.append(c);
                i++;
                while (i < n && text.charAt(i) != c) {
                    if (text.charAt(i) == '\\' && i + 1 < n) {
                        out.append(text.charAt(i++));
                    }
                    out.append(text.charAt(i++));
                }
                if (i < n) {
                    out.append(text.charAt(i++));
                }
            } else {
                if (!Character.isWhitespace(c)) {
                    out.append(c);
                }
                i++;
            }
        }
        return out.toString();
    }

    @Test
    void theCommentAndWhiteSpaceFilterKeepsLiteralsAndDropsComments() {
        String source = "class A { // c\n String s = \"a // b  c\"; /* x */ char q = '\\''; }";
        assertEquals("classA{Strings=\"a // b  c\";charq='\\'';}", code(source.getBytes(StandardCharsets.UTF_8)));
    }
}
