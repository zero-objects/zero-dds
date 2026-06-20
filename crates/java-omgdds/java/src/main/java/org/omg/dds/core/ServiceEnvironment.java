// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core;

/**
 * OMG DDS Java-PSM {@code ServiceEnvironment} — Spec §7.3.1.
 *
 * <p>A {@code ServiceEnvironment} object represents an instantiation of a
 * Service implementation within a JVM; it is the "root" for all other DDS
 * objects. An application instantiates one by means of the static
 * {@link #createInstance(String)} factory, which looks up a concrete
 * {@code ServiceEnvironment} subclass — exactly the RTI Connext / OpenSplice
 * bootstrap idiom.
 *
 * <p>Per the spec the concrete subclass is named either by the
 * {@code implClassName} argument or, when that argument is {@code null}, by the
 * Java system property {@value #IMPLEMENTATION_CLASS_NAME_PROPERTY}. The named
 * class must be a public subclass of {@code ServiceEnvironment} with a public
 * no-argument constructor.
 *
 * <p>The ZeroDDS implementation is pure Java (no native library load); the
 * concrete subclass is {@code org.zerodds.ServiceEnvironmentImpl}.
 */
public abstract class ServiceEnvironment {

    /**
     * The Java system property consulted by {@link #createInstance(String)}
     * when no explicit implementation class name is supplied (Spec §7.3.1).
     */
    public static final String IMPLEMENTATION_CLASS_NAME_PROPERTY =
            "org.omg.dds.serviceClassName";

    protected ServiceEnvironment() {}

    /**
     * Instantiates a concrete {@code ServiceEnvironment} by reflectively
     * loading the named subclass (Spec §7.3.1).
     *
     * @param implClassName fully-qualified name of a {@code ServiceEnvironment}
     *     subclass; when {@code null}, the
     *     {@value #IMPLEMENTATION_CLASS_NAME_PROPERTY} system property is used
     *     instead.
     * @return a new {@code ServiceEnvironment} instance.
     * @throws ServiceConfigurationException if no class name is available or the
     *     named class cannot be loaded / instantiated as a
     *     {@code ServiceEnvironment}.
     */
    public static ServiceEnvironment createInstance(String implClassName) {
        String className = implClassName;
        if (className == null) {
            className = System.getProperty(IMPLEMENTATION_CLASS_NAME_PROPERTY);
        }
        if (className == null || className.isEmpty()) {
            throw new ServiceConfigurationException(
                    "no ServiceEnvironment implementation class name supplied "
                            + "(pass it to createInstance or set the system property "
                            + IMPLEMENTATION_CLASS_NAME_PROPERTY + ")");
        }
        try {
            Class<?> cls = Class.forName(
                    className, true, Thread.currentThread().getContextClassLoader() != null
                            ? Thread.currentThread().getContextClassLoader()
                            : ServiceEnvironment.class.getClassLoader());
            Object instance = cls.getDeclaredConstructor().newInstance();
            if (!(instance instanceof ServiceEnvironment)) {
                throw new ServiceConfigurationException(
                        className + " is not a subclass of ServiceEnvironment");
            }
            return (ServiceEnvironment) instance;
        } catch (ServiceConfigurationException e) {
            throw e;
        } catch (ReflectiveOperationException e) {
            throw new ServiceConfigurationException(
                    "cannot instantiate ServiceEnvironment '" + className + "': " + e, e);
        }
    }

    /**
     * Thrown when a {@code ServiceEnvironment} implementation cannot be located
     * or instantiated. Unchecked per Spec §7.3.2 (all DDS exceptions extend
     * {@link RuntimeException}).
     */
    public static final class ServiceConfigurationException extends RuntimeException {
        private static final long serialVersionUID = 1L;

        public ServiceConfigurationException(String message) {
            super(message);
        }

        public ServiceConfigurationException(String message, Throwable cause) {
            super(message, cause);
        }
    }
}
