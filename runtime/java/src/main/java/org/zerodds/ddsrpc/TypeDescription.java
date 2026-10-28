// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import java.util.Base64;
import org.zerodds.cdr.Md5;
import org.zerodds.cdr.TypeObjectRegistry;

/**
 * The XTypes description of a topic type of the Enhanced Service Mapping
 * (DDS-RPC 1.0 §7.5.1.2, XTypes 1.3 §7.6.3): the {@code PID_TYPE_INFORMATION}
 * payload, the COMPLETE {@code TypeObject} of the type and the objects it
 * names, and which object names which. With it the endpoints announce the
 * type and serve its objects, so a peer matches by assignability and not by
 * the type name alone (§7.4.3, §7.7.2.5).
 *
 * <p>The generator writes the values as Base64 text, because a Java class
 * file bounds the size of a method that initialises byte arrays.
 */
public final class TypeDescription {

    private final byte[] information;
    private final byte[][] objects;
    private final int[][] references;

    /**
     * @param information the {@code PID_TYPE_INFORMATION} payload, XCDR2
     *     little-endian, or empty for none
     * @param objects the serialized COMPLETE objects; the topic type first
     * @param references for each object the indexes of the objects it names
     *     directly
     */
    public TypeDescription(byte[] information, byte[][] objects, int[][] references) {
        if (objects.length == 0 || objects.length != references.length) {
            throw new IllegalArgumentException("a description has an object and its references");
        }
        this.information = information.clone();
        this.objects = new byte[objects.length][];
        for (int at = 0; at < objects.length; at++) {
            this.objects[at] = objects[at].clone();
        }
        this.references = new int[references.length][];
        for (int at = 0; at < references.length; at++) {
            this.references[at] = references[at].clone();
        }
    }

    /**
     * The description a generated binding writes.
     *
     * @param informationBase64 the {@code PID_TYPE_INFORMATION} payload, Base64
     * @param objectsBase64 the objects, Base64, separated by {@code ;}
     * @param references for each object the indexes it names, separated by
     *     {@code ,}; the lists are separated by {@code ;}
     */
    public static TypeDescription of(
            String informationBase64, String objectsBase64, String references) {
        String[] encoded = objectsBase64.split(";", -1);
        byte[][] objects = new byte[encoded.length][];
        for (int at = 0; at < encoded.length; at++) {
            objects[at] = Base64.getDecoder().decode(encoded[at]);
        }
        String[] lists = references.split(";", -1);
        int[][] indexes = new int[lists.length][];
        for (int at = 0; at < lists.length; at++) {
            if (lists[at].isEmpty()) {
                indexes[at] = new int[0];
                continue;
            }
            String[] parts = lists[at].split(",");
            indexes[at] = new int[parts.length];
            for (int i = 0; i < parts.length; i++) {
                indexes[at][i] = Integer.parseInt(parts[i]);
            }
        }
        return new TypeDescription(Base64.getDecoder().decode(informationBase64), objects, indexes);
    }

    /** The {@code PID_TYPE_INFORMATION} payload; empty for none. */
    public byte[] information() {
        return information.clone();
    }

    /** The serialized COMPLETE object of the topic type. */
    public byte[] object() {
        return objects[0].clone();
    }

    /** The identifier of the object at {@code at}: the kind octet and the 14-octet hash. */
    private byte[] identifier(int at) {
        byte[] hash = Md5.hash(objects[at]);
        byte[] id = new byte[15];
        id[0] = objects[at][4];
        System.arraycopy(hash, 0, id, 1, 14);
        return id;
    }

    /** The objects, as the type-lookup service of the PSM serves them. */
    public TypeObjectRegistry registry() {
        TypeObjectRegistry.Entry[] entries = new TypeObjectRegistry.Entry[objects.length];
        for (int at = 0; at < objects.length; at++) {
            byte[][] named = new byte[references[at].length][];
            for (int i = 0; i < named.length; i++) {
                named[i] = identifier(references[at][i]);
            }
            entries[at] = new TypeObjectRegistry.Entry(identifier(at), objects[at], named);
        }
        return new TypeObjectRegistry(entries);
    }
}
