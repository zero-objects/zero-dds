// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
// Consumer of fresh generated files, never a patched generated fixture.
import java.security.MessageDigest;
import java.util.Arrays;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.XcdrException;
import org.zerodds.rtps.DdsWireBridge;
import org.zerodds.rtps.SerializedSample;

public final class Consumer {
    private static void check(boolean condition, String why) {
        if (!condition) throw new AssertionError(why);
    }
    private static byte[] hex(String value) {
        return java.util.HexFormat.of().parseHex(value);
    }
    private static void refuses(Class<? extends RuntimeException> expected, Runnable operation) {
        try { operation.run(); }
        catch (RuntimeException e) {
            if (expected.isInstance(e)) return;
            throw new AssertionError("Wrong refusal: " + e, e);
        }
        throw new AssertionError("Expected " + expected.getName());
    }
    public static void main(String[] args) throws Exception {
        var support = ZdTypeSupports.Support_bridge_Point.INSTANCE;
        int representation = support.writeRepresentation();
        check(representation == 0 || representation == 2, "not a DDS representation id");
        check(Arrays.equals(support.readRepresentations(), new int[] {representation}), "wrong capability");
        check(support.getTypeName().equals("bridge::Point"), "lost IDL scoped name");
        check(support.typeObject().length > 0 && support.typeIdentifier().length == 14, "missing source metadata");
        byte[] standard = support.typeInformation();
        var registry = ZdTypeSupports.Support_bridge_Composite.INSTANCE.typeObjects();
        check(registry.entries().size() == 6, "dependent minimal/complete closure incomplete");
        check(registry.mappings().size() == 3, "complete-to-minimal mapping incomplete");
        for (var pair : registry.mappings()) {
            check(registry.find(pair.complete()) != null && registry.find(pair.minimal()) != null,
                    "mapping object unavailable");
        }
        int dependentObjects = 0;
        for (var entry : registry.entries()) {
            var dependencies = registry.dependencies(entry.identifier());
            if (!dependencies.isEmpty()) {
                dependentObjects++;
                check(dependencies.size() == 2, "Composite direct dependency count");
                for (var dependency : dependencies)
                    check(dependency.serializedSize() == dependency.object().length, "dependency size lost");
                var ids = entry.dependencyIdentifiers();
                ids[0][0] ^= 1;
                check(!Arrays.equals(ids[0], entry.dependencyIdentifiers()[0]), "mutable dependency escaped");
                var incomplete = new org.zerodds.cdr.TypeObjectRegistry.Entry(entry.identifier(), entry.object());
                var entries = new java.util.ArrayList<>(registry.entries());
                entries.add(0, incomplete);
                refuses(IllegalArgumentException.class, () -> new org.zerodds.cdr.TypeObjectRegistry(
                        entries.toArray(new org.zerodds.cdr.TypeObjectRegistry.Entry[0])));
            }
        }
        check(dependentObjects == 2, "minimal and complete dependency graph missing");
        check(support.typeObjects().entries().size() == 2, "unrelated objects leaked into Point registry");
        for (var entry : support.typeObjects().entries()) {
            var found = registry.find(entry.identifier());
            check(found != null && Arrays.equals(found.object(), entry.object()), "announced dependency unavailable");
            byte[] copy = found.object();
            copy[0] ^= 1;
            check(!Arrays.equals(copy, found.object()), "mutable registry object escaped");
            refuses(IllegalArgumentException.class, () -> new org.zerodds.cdr.TypeObjectRegistry.Entry(found.identifier(), copy));
        }
        check(registry.find(new byte[15]) == null, "unknown ID fabricated an object");
        var metadata = java.nio.ByteBuffer.wrap(standard).order(java.nio.ByteOrder.LITTLE_ENDIAN);
        check(standard.length == 100 && metadata.getInt(0) == 96, "primitive Point TypeInformation framing");
        check(metadata.getInt(4) == 0x40001001 && metadata.getInt(52) == 0x40001002, "standard member IDs and LC4");
        check(standard[20] == (byte)0xf1 && standard[68] == (byte)0xf2, "minimal/complete TypeIdentifier tags");
        check(Arrays.equals(Arrays.copyOfRange(standard, 69, 83), support.typeIdentifier()), "complete metadata hash differs from actual object hash");
        check(metadata.getInt(84) == support.typeObject().length, "complete object serialized size");
        check(metadata.getInt(40) == 0 && metadata.getInt(88) == 0, "primitive Point has no dependencies");
        for (boolean writer : new boolean[] {true, false}) {
            byte[] announcement = org.zerodds.rtps.DiscoveryData.buildEndpoint(writer, new byte[12], new byte[16], "Point", support.getTypeName(),
                    org.zerodds.rtps.Rtps.RELIABILITY_BEST_EFFORT, org.zerodds.rtps.Rtps.DURABILITY_VOLATILE,
                    new int[] {representation}, org.zerodds.rtps.Locator.udpV4(new byte[] {127,0,0,1}, 7411), support.typeIdentifier(), standard);
            var endpoint = org.zerodds.rtps.DiscoveryData.parseEndpoint(writer, announcement);
            check(endpoint != null && Arrays.equals(endpoint.typeInformation, standard), "generated metadata lost in SEDP");
        }
        System.out.println("SOURCE_TYPE_INFORMATION=" + java.util.HexFormat.of().formatHex(standard));
        check(support.isKeyed(), "key omitted");
        var point = new ZdTypes.bridge.Point();
        point.id = 42;
        point.reading = 65535;
        byte[] little = hex("2a000000ffff");
        byte[] big = hex("0000002affff");
        check(Arrays.equals(support.encode(point), little), "LE independent octet oracle");
        check(Arrays.equals(support.encode(point, EndianMode.BIG_ENDIAN), big), "BE independent octet oracle");
        var decoded = support.decode(big, 0, big.length, EndianMode.BIG_ENDIAN, representation);
        check(decoded.id == 42 && decoded.reading == 65535, "BE decode values");
        byte[] key = hex("0000002a000000000000000000000000");
        check(key.length == 16, "independent key oracle must contain exactly 16 bytes");
        check(Arrays.equals(support.keyHash(point), key), "short key must be zero-padded");
        point.reading = 7;
        check(Arrays.equals(support.keyHash(point), key), "non-key changed instance");
        point.id = 43;
        check(!Arrays.equals(support.keyHash(point), key), "changed key was ignored");
        var named = new ZdTypes.bridge.Named();
        // Variable length is not an unbounded declared maximum.
        var shortName = new ZdTypes.bridge.ShortName();
        shortName.name = "abcdX";
        shortName.value = 8;
        byte[] shortKey = hex("00000006616263645800000000000000");
        check(Arrays.equals(ZdTypeSupports.Support_bridge_ShortName.INSTANCE.keyHash(shortName), shortKey),
                "bounded string must be zero-padded");
        shortName.value = 9;
        check(Arrays.equals(ZdTypeSupports.Support_bridge_ShortName.INSTANCE.keyHash(shortName), shortKey),
                "bounded string non-key changed identity");
        shortName.name = "abcdY";
        check(Arrays.equals(ZdTypeSupports.Support_bridge_ShortName.INSTANCE.keyHash(shortName),
                hex("00000006616263645900000000000000")), "bounded string key suffix lost");
        named.name = "a";
        check(Arrays.equals(ZdTypeSupports.Support_bridge_Named.INSTANCE.keyHash(named),
                MessageDigest.getInstance("MD5").digest(hex("000000026100"))), "maximum key size selects MD5");
        refuses(org.zerodds.wire.WireException.Truncated.class,
                () -> support.decode(Arrays.copyOf(little, little.length - 1)));
        refuses(XcdrException.class, () -> support.decode(Arrays.copyOf(little, little.length + 1)));
        refuses(XcdrException.class, () -> support.decode(little, 0, little.length, EndianMode.LITTLE_ENDIAN,
                representation == 0 ? 2 : 0));
        refuses(IndexOutOfBoundsException.class, () -> support.decode(little, 0, little.length + 1));

        var emptySupport = ZdTypeSupports.Support_bridge_Empty.INSTANCE;
        check(emptySupport.encode(new ZdTypes.bridge.Empty()).length == 0, "invented empty payload");
        check(emptySupport.decode(new byte[0]) != null, "valid empty type refused");
        var extendedSupport = ZdTypeSupports.Support_bridge_Extended.INSTANCE;
        var extended = new ZdTypes.bridge.Extended();
        extended.id = 13;
        check(extendedSupport.decode(extendedSupport.encode(extended)).calibration == null, "optional absence");
        extended.calibration = 1.25;
        byte[] body = extendedSupport.encode(extended);
        byte[] payload = new byte[body.length + 4];
        System.arraycopy(DdsWireBridge.encapFor(extendedSupport), 0, payload, 0, 4);
        System.arraycopy(body, 0, payload, 4, body.length);
        var received = DdsWireBridge.decode(extendedSupport, SerializedSample.parse(payload));
        check(received.id == 13 && received.calibration == 1.25, "real descriptor/encapsulation bridge");
        System.out.println("JAVA_GENERATED_SUPPORT_PASS representation=" + representation
                + " metadata LE BE strict-decode key-oracles optional empty transport-bridge");
    }
}
