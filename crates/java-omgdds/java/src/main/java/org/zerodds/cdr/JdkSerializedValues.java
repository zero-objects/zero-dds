// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InvalidClassException;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.io.ObjectStreamClass;
import java.io.Serializable;
import java.lang.reflect.Modifier;

import org.zerodds.wire.Reader;
import org.zerodds.wire.Writer;

/**
 * The form of a {@code Serializable} class of the JDK that has neither an entry in Table 8.1 nor a
 * form in {@link JdkValueTypes}: DDS-Java-PSM 1.0 §8.1 asks that every {@code Serializable} type is
 * available for publishing, and the fields of a JDK class cannot be read reflectively. The value
 * is its Java serialization, a {@code sequence<octet>} (an unsigned 32-bit length and the octets).
 * Only Java programs read it.
 *
 * <p>Reading never trusts the octets: the stream may name only the declared class and
 * {@code java.lang} classes (and arrays of them), so no other class of the JDK or the application
 * is instantiated; a stream that names another class, or is longer than {@link #MAX_OCTETS}, is an
 * {@link XcdrException}.
 */
final class JdkSerializedValues {

    /** The longest serialized value accepted: 1 MiB. */
    static final int MAX_OCTETS = 1 << 20;

    private JdkSerializedValues() {}

    /** Whether {@code c} is a concrete, Serializable class of the JDK this form can carry. */
    static boolean supports(Class<?> c) {
        return Serializable.class.isAssignableFrom(c) && !c.isInterface()
                && !Modifier.isAbstract(c.getModifiers()) && !c.isArray() && !c.isEnum();
    }

    static void write(Writer w, Class<?> c, Object value) {
        if (value.getClass() != c) {
            throw new XcdrException("the value is a " + value.getClass().getName()
                    + " where the declared type is " + c.getName());
        }
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        try (ObjectOutputStream stream = new ObjectOutputStream(out)) {
            stream.writeObject(value);
        } catch (IOException e) {
            throw new XcdrException("cannot serialize a " + c.getName() + ": " + e.getMessage(), e);
        }
        byte[] octets = out.toByteArray();
        if (octets.length > MAX_OCTETS) {
            throw new XcdrException("a serialized " + c.getName() + " of " + octets.length
                    + " octets exceeds the limit of " + MAX_OCTETS);
        }
        w.writeU32(octets.length);
        for (byte b : octets) {
            w.writeU8(Byte.toUnsignedInt(b));
        }
    }

    static Object read(Reader r, final Class<?> c) {
        long length = r.readU32();
        if (length > MAX_OCTETS || length > r.remaining()) {
            throw new XcdrException("a serialized " + c.getName() + " of " + length
                    + " octets does not fit the limit or the data that is left");
        }
        byte[] octets = new byte[(int) length];
        for (int i = 0; i < octets.length; i++) {
            octets[i] = (byte) r.readU8();
        }
        try (ObjectInputStream in = new ObjectInputStream(new ByteArrayInputStream(octets)) {
            @Override
            protected Class<?> resolveClass(ObjectStreamClass desc)
                    throws IOException, ClassNotFoundException {
                String name = desc.getName();
                String element = name;
                while (element.startsWith("[")) {
                    element = element.substring(1);
                }
                boolean primitiveArray = element.length() == 1;
                if (!name.equals(c.getName()) && !primitiveArray && !element.startsWith("L" + "java.lang.")
                        && !element.startsWith("java.lang.")) {
                    throw new InvalidClassException(name, "not the declared class " + c.getName());
                }
                return super.resolveClass(desc);
            }
        }) {
            Object value = in.readObject();
            if (value == null || value.getClass() != c) {
                throw new XcdrException("the octets are no " + c.getName());
            }
            return value;
        } catch (IOException | ClassNotFoundException e) {
            throw new XcdrException("the octets are no " + c.getName() + ": " + e.getMessage(), e);
        }
    }
}
