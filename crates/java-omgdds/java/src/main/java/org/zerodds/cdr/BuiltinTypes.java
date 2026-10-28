// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.type.builtin.KeyedBytes;
import org.omg.dds.type.builtin.KeyedString;

/**
 * ZeroDDS {@link KeyedString} and {@link KeyedBytes} — the built-in types of
 * DDS-XTypes, mapped to modifiable value types (DDS-Java-PSM 1.0 §7.8.2).
 *
 * <p>Both live in one file because they differ only in the type of their value.
 */
public final class BuiltinTypes {

    private BuiltinTypes() {}

    /** DDS::KeyedString: a key and a string value. */
    public static final class KeyedStringImpl extends KeyedString {
        private static final long serialVersionUID = 5185023295888681379L;

        private transient ServiceEnvironment environment;
        private String key = "";
        private String value = "";

        public KeyedStringImpl(ServiceEnvironment environment) {
            this.environment = environment;
        }

        @Override
        public ServiceEnvironment getEnvironment() {
            return environment;
        }

        @Override
        public String getKey() {
            return key;
        }

        @Override
        public KeyedString setKey(CharSequence key) {
            this.key = key == null ? "" : key.toString();
            return this;
        }

        @Override
        public String getValue() {
            return value;
        }

        @Override
        public KeyedString setValue(CharSequence value) {
            this.value = value == null ? "" : value.toString();
            return this;
        }

        @Override
        public void copyFrom(KeyedString src) {
            this.key = src.getKey();
            this.value = src.getValue();
        }

        @Override
        public KeyedStringImpl clone() {
            KeyedStringImpl copy = new KeyedStringImpl(environment);
            copy.copyFrom(this);
            return copy;
        }

        @Override
        public boolean equals(Object o) {
            if (!(o instanceof KeyedString)) {
                return false;
            }
            KeyedString other = (KeyedString) o;
            return key.equals(other.getKey()) && value.equals(other.getValue());
        }

        @Override
        public int hashCode() {
            return key.hashCode() * 31 + value.hashCode();
        }

        @Override
        public String toString() {
            return "KeyedString[key=" + key + ", value=" + value + "]";
        }

        /**
         * Java serialization (DDS-Java-PSM 1.0 §7.3.3) carries the value, not
         * its {@link ServiceEnvironment}; a deserialized object belongs to the
         * default environment of this implementation.
         */
        private void readObject(java.io.ObjectInputStream in)
                throws java.io.IOException, ClassNotFoundException {
            in.defaultReadObject();
            environment = ServiceEnvironmentImpl.getDefaultEnvironment();
        }
    }

    /** DDS::KeyedBytes: a key and a byte-sequence value. */
    public static final class KeyedBytesImpl extends KeyedBytes {
        private static final long serialVersionUID = 2666227708313407807L;

        private transient ServiceEnvironment environment;
        private String key = "";
        private byte[] value = new byte[0];

        public KeyedBytesImpl(ServiceEnvironment environment) {
            this.environment = environment;
        }

        @Override
        public ServiceEnvironment getEnvironment() {
            return environment;
        }

        @Override
        public String getKey() {
            return key;
        }

        @Override
        public KeyedBytes setKey(CharSequence key) {
            this.key = key == null ? "" : key.toString();
            return this;
        }

        @Override
        public byte[] getValue() {
            return value.clone();
        }

        @Override
        public KeyedBytes setValue(byte value[]) {
            this.value = value.clone();
            return this;
        }

        @Override
        public KeyedBytes setValue(byte value[], int offset, int length) {
            this.value = java.util.Arrays.copyOfRange(value, offset, offset + length);
            return this;
        }

        @Override
        public void copyFrom(KeyedBytes src) {
            this.key = src.getKey();
            this.value = src.getValue();
        }

        @Override
        public KeyedBytesImpl clone() {
            KeyedBytesImpl copy = new KeyedBytesImpl(environment);
            copy.copyFrom(this);
            return copy;
        }

        @Override
        public boolean equals(Object o) {
            if (!(o instanceof KeyedBytes)) {
                return false;
            }
            KeyedBytes other = (KeyedBytes) o;
            return key.equals(other.getKey()) && java.util.Arrays.equals(value, other.getValue());
        }

        @Override
        public int hashCode() {
            return key.hashCode() * 31 + java.util.Arrays.hashCode(value);
        }

        @Override
        public String toString() {
            return "KeyedBytes[key=" + key + ", value=" + value.length + " bytes]";
        }

        /**
         * Java serialization (DDS-Java-PSM 1.0 §7.3.3) carries the value, not
         * its {@link ServiceEnvironment}; a deserialized object belongs to the
         * default environment of this implementation.
         */
        private void readObject(java.io.ObjectInputStream in)
                throws java.io.IOException, ClassNotFoundException {
            in.defaultReadObject();
            environment = ServiceEnvironmentImpl.getDefaultEnvironment();
        }
    }
}