// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.AlreadyClosedException;
import org.omg.dds.core.DDSException;
import org.omg.dds.core.DDSObject;
import org.omg.dds.core.IllegalOperationException;
import org.omg.dds.core.ImmutablePolicyException;
import org.omg.dds.core.InconsistentPolicyException;
import org.omg.dds.core.NotEnabledException;
import org.omg.dds.core.OutOfResourcesException;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.ServiceConfigurationException;
import org.omg.dds.core.ServiceInitializationException;

import java.io.File;
import java.io.InputStream;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.CodeSource;
import java.util.ArrayList;
import java.util.Enumeration;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

/**
 * DDS-Java-PSM 1.0 Annex A and Annex B: {@code omgdds.jar} holds the classes of the PSM, an
 * application is compiled against them and runs against any implementation, and the
 * sources in {@code omgdds_src.zip} are normative for the interfaces. The package
 * {@code org.omg.dds} of this binding is the OMG source; an implementation lives in
 * {@code org.zerodds}.
 *
 * <p>What can be checked from the classes: the exceptions are the abstract classes of the OMG
 * source, with its {@code serialVersionUID} values and the {@code DDSObject} interface of
 * {@code DDSException}, and no class of {@code org.omg.dds} refers to the implementation (so
 * that the same classes work against another one). The package {@code org.omg.dds.rpc} belongs
 * to the RPC specification, not to this PSM, and is not part of the check.
 */
class TypesPsmApiTest {

    /** The abstract exception classes of org.omg.dds.core and the serialVersionUID of the OMG source. */
    private static Map<Class<?>, Long> exceptions() {
        Map<Class<?>, Long> out = new LinkedHashMap<>();
        out.put(DDSException.class, -7560788483511488746L);
        out.put(AlreadyClosedException.class, 8818392556145975401L);
        out.put(IllegalOperationException.class, -6782657920034098505L);
        out.put(ImmutablePolicyException.class, -7676868650200410524L);
        out.put(InconsistentPolicyException.class, 5264713016409453514L);
        out.put(NotEnabledException.class, -1435103296817695008L);
        out.put(OutOfResourcesException.class, 6105570967607779945L);
        out.put(PreconditionNotMetException.class, -6204457556635480597L);
        out.put(ServiceConfigurationException.class, -3247742026162310484L);
        out.put(ServiceInitializationException.class, 323168673886924537L);
        return out;
    }

    private static boolean isServiceException(Class<?> cls) {
        return cls == ServiceConfigurationException.class
                || cls == ServiceInitializationException.class;
    }

    @Test
    void theExceptionsAreAbstractAndHaveTheSerialVersionUidOfTheOmgSource() throws Exception {
        for (Map.Entry<Class<?>, Long> e : exceptions().entrySet()) {
            Class<?> cls = e.getKey();
            // the OMG source: DDSException and what extends it are abstract; the two exceptions of
            // the service lookup are concrete RuntimeExceptions of their own
            assertEquals(!isServiceException(cls), Modifier.isAbstract(cls.getModifiers()),
                    cls.getSimpleName() + " is abstract");
            assertTrue(RuntimeException.class.isAssignableFrom(cls), cls.getSimpleName());
            Field f = cls.getDeclaredField("serialVersionUID");
            f.setAccessible(true);
            assertEquals(e.getValue().longValue(), f.getLong(null), cls.getSimpleName());
        }
    }

    @Test
    void ddsExceptionIsADdsObjectAsInTheOmgSource() throws Exception {
        assertTrue(DDSObject.class.isAssignableFrom(DDSException.class));
        // Every exception of the PSM is one, since all extend DDSException.
        for (Class<?> cls : exceptions().keySet()) {
            if (isServiceException(cls)) {
                continue; // plain RuntimeExceptions in the OMG source
            }
            assertTrue(DDSObject.class.isAssignableFrom(cls), cls.getSimpleName());
        }
        // The implementation answers getEnvironment (the abstract method of DDSObject).
        assertNotNull(new org.zerodds.core.PreconditionNotMetException("x").getEnvironment());
        assertNotNull(new org.zerodds.core.AlreadyClosedException("x").getEnvironment());
        assertNotNull(new org.zerodds.core.IllegalOperationException("x").getEnvironment());
        assertNotNull(new org.zerodds.core.NotEnabledException("x").getEnvironment());
        assertNotNull(new org.zerodds.core.OutOfResourcesException("x").getEnvironment());
        assertNotNull(new org.zerodds.core.ImmutablePolicyException("x").getEnvironment());
        assertNotNull(new org.zerodds.core.InconsistentPolicyException("x").getEnvironment());
        DDSObject asObject = new org.zerodds.core.AlreadyClosedException("x");
        assertEquals(org.zerodds.ServiceEnvironmentImpl.getDefaultEnvironment(), asObject.getEnvironment());
    }

    @Test
    void noClassOfTheApiPackageRefersToTheImplementation() throws Exception {
        CodeSource source = DDSException.class.getProtectionDomain().getCodeSource();
        assumeTrue(source != null, "the classes come from a place that can be listed");
        URL location = source.getLocation();
        File root = new File(location.toURI());
        byte[] needle = "org/zerodds".getBytes(StandardCharsets.US_ASCII);
        List<String> offenders = new ArrayList<>();
        int inspected = 0;
        if (root.isDirectory()) {
            Path api = root.toPath().resolve("org").resolve("omg").resolve("dds");
            List<Path> classes;
            try (Stream<Path> walk = Files.walk(api)) {
                classes = walk.filter(p -> p.toString().endsWith(".class")
                        && !api.relativize(p).startsWith("rpc")).collect(Collectors.toList());
            }
            for (Path p : classes) {
                inspected++;
                if (contains(Files.readAllBytes(p), needle)) {
                    offenders.add(api.relativize(p).toString());
                }
            }
        } else {
            try (ZipFile zip = new ZipFile(root)) {
                Enumeration<? extends ZipEntry> entries = zip.entries();
                while (entries.hasMoreElements()) {
                    ZipEntry entry = entries.nextElement();
                    if (entry.getName().startsWith("org/omg/dds/")
                            && !entry.getName().startsWith("org/omg/dds/rpc/")
                            && entry.getName().endsWith(".class")) {
                        inspected++;
                        try (InputStream in = zip.getInputStream(entry)) {
                            if (contains(readAll(in), needle)) {
                                offenders.add(entry.getName());
                            }
                        }
                    }
                }
            }
        }
        assertTrue(inspected > 100, "the classes of the API package were found: " + inspected);
        assertEquals(new ArrayList<String>(), offenders,
                "classes of org.omg.dds that refer to org.zerodds");
    }

    private static byte[] readAll(InputStream in) throws java.io.IOException {
        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int n;
        while ((n = in.read(buffer)) > 0) {
            out.write(buffer, 0, n);
        }
        return out.toByteArray();
    }

    private static boolean contains(byte[] haystack, byte[] needle) {
        outer:
        for (int i = 0; i + needle.length <= haystack.length; i++) {
            for (int j = 0; j < needle.length; j++) {
                if (haystack[i + j] != needle[j]) {
                    continue outer;
                }
            }
            return true;
        }
        return false;
    }
}
