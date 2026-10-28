// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Immutable source-derived XTypes objects. This is not a TypeLookup service.
 * Identifiers and objects are serialized by the shared generator, XCDR2 LE.
 * Only equivalence-hash identifiers are registrable, never primitive types.
 */
public final class TypeObjectRegistry {
    public static final class Entry {
        private final byte[] identifier;
        private final byte[] object;
        private final byte[][] dependencies;

        public Entry(byte[] identifier, byte[] object) {
            this(identifier, object, new byte[0][]);
        }

        public Entry(byte[] identifier, byte[] object, byte[][] dependencies) {
            this.identifier = Objects.requireNonNull(identifier, "identifier").clone();
            this.object = Objects.requireNonNull(object, "object").clone();
            this.dependencies = Arrays.stream(Objects.requireNonNull(dependencies, "dependencies"))
                    .map(value -> Objects.requireNonNull(value, "dependency").clone()).toArray(byte[][]::new);
            if (this.identifier.length != 15
                    || (this.identifier[0] != (byte) 0xf1 && this.identifier[0] != (byte) 0xf2))
                throw new IllegalArgumentException("Expected minimal or complete equivalence identifier");
            if (this.object.length < 5 || this.object[4] != this.identifier[0])
                throw new IllegalArgumentException("TypeObject kind differs from identifier");
            byte[] hash = Md5.hash(this.object);
            for (int i = 0; i < 14; i++)
                if (hash[i] != this.identifier[i + 1])
                    throw new IllegalArgumentException("TypeObject hash differs from identifier");
        }

        public byte[] identifier() { return identifier.clone(); }
        public byte[] object() { return object.clone(); }
        public int serializedSize() { return object.length; }
        public byte[][] dependencyIdentifiers() {
            return Arrays.stream(dependencies).map(byte[]::clone).toArray(byte[][]::new);
        }
    }

    public static final class IdentifierPair {
        private final byte[] complete;
        private final byte[] minimal;
        public IdentifierPair(byte[] complete, byte[] minimal) {
            this.complete = Objects.requireNonNull(complete, "complete").clone();
            this.minimal = Objects.requireNonNull(minimal, "minimal").clone();
            if (complete.length != 15 || complete[0] != (byte) 0xf2
                    || minimal.length != 15 || minimal[0] != (byte) 0xf1)
                throw new IllegalArgumentException("Expected complete-to-minimal equivalence identifiers");
        }
        public byte[] complete() { return complete.clone(); }
        public byte[] minimal() { return minimal.clone(); }
    }

    private final Map<String, Entry> entries;
    private final Map<String, IdentifierPair> mappings;

    public TypeObjectRegistry(Entry[] values) {
        this(values, new IdentifierPair[0]);
    }

    public TypeObjectRegistry(Entry[] values, IdentifierPair[] pairs) {
        Map<String, Entry> collected = new LinkedHashMap<>();
        for (Entry entry : Objects.requireNonNull(values, "values")) {
            Objects.requireNonNull(entry, "entry");
            String key = HexBytes.encode(entry.identifier);
            Entry previous = collected.putIfAbsent(key, entry);
            if (previous != null && (!Arrays.equals(previous.object, entry.object)
                    || !Arrays.deepEquals(previous.dependencies, entry.dependencies)))
                throw new IllegalArgumentException("Conflicting TypeObject or dependency metadata for identifier");
        }
        entries = java.util.Collections.unmodifiableMap(collected);
        for (Entry entry : entries.values()) {
            for (byte[] dependency : entry.dependencies) {
                if (dependency.length != 15 || dependency[0] != entry.identifier[0] || find(dependency) == null)
                    throw new IllegalArgumentException("Missing or wrong-kind dependency object");
            }
        }
        Map<String, IdentifierPair> mapped = new LinkedHashMap<>();
        for (IdentifierPair pair : Objects.requireNonNull(pairs, "pairs")) {
            Objects.requireNonNull(pair, "pair");
            if (find(pair.complete) == null || find(pair.minimal) == null)
                throw new IllegalArgumentException("Mapping references an absent object");
            String key = HexBytes.encode(pair.complete);
            IdentifierPair previous = mapped.putIfAbsent(key, pair);
            if (previous != null && !Arrays.equals(previous.minimal, pair.minimal))
                throw new IllegalArgumentException("Conflicting complete-to-minimal mapping");
        }
        mappings = java.util.Collections.unmodifiableMap(mapped);
    }

    /** Unknown identifiers are absent, not fabricated empty TypeObjects. */
    public Entry find(byte[] identifier) {
        return entries.get(HexBytes.encode(Objects.requireNonNull(identifier, "identifier")));
    }

    public List<Entry> entries() {
        return java.util.Collections.unmodifiableList(new java.util.ArrayList<>(entries.values()));
    }
    public List<IdentifierPair> mappings() {
        return java.util.Collections.unmodifiableList(new java.util.ArrayList<>(mappings.values()));
    }

    /** Direct dependencies retain each object's actual serialized size. */
    public List<Entry> dependencies(byte[] identifier) {
        Entry entry = find(identifier);
        if (entry == null) throw new IllegalArgumentException("Unknown type identifier");
        return java.util.Collections.unmodifiableList(Arrays.stream(entry.dependencies)
                .map(this::find).collect(java.util.stream.Collectors.toList()));
    }
}
