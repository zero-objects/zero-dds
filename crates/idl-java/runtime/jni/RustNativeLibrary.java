// Copyright (c) zerodds contributors
// SPDX-License-Identifier: Apache-2.0
package org.zerodds.dds.jni;

/**
 * Lazy-Loader fuer die native ZeroDDS-JNI-Bibliothek
 * (`libdds_java_jni.{so,dylib,dll}`).
 *
 * <p>Aufruf-Pattern: jeder JNI-Wrapper ruft im Static-Initializer
 * {@link #ensureLoaded()}, sodass der Library-Load auch dann passiert,
 * wenn der Wrapper isoliert geladen wird.
 *
 * <p>Die Bibliothek wird per {@link System#loadLibrary} aus dem
 * {@code java.library.path} gesucht. Ueberschreiben mit der Property
 * {@code -Dorg.zerodds.dds.jni.path=/abs/path} ist moeglich.
 */
public final class RustNativeLibrary {
    private static volatile boolean loaded = false;
    private static final Object LOCK = new Object();

    private RustNativeLibrary() {
        // Util-Klasse — kein Konstruktor.
    }

    /**
     * Stellt sicher, dass die Native-Library genau einmal geladen wird.
     * Idempotent + thread-safe.
     */
    public static void ensureLoaded() {
        if (loaded) {
            return;
        }
        synchronized (LOCK) {
            if (loaded) {
                return;
            }
            String override = System.getProperty("org.zerodds.dds.jni.path");
            if (override != null && !override.isEmpty()) {
                System.load(override);
            } else {
                System.loadLibrary("dds_java_jni");
            }
            loaded = true;
        }
    }

    /** Sichtbarkeit fuer Tests: ist die Bibliothek geladen? */
    public static boolean isLoaded() {
        return loaded;
    }
}
