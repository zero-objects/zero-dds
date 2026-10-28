// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.Arrays;
import java.util.function.Function;

import org.omg.dds.type.dynamic.DynamicType;
import org.zerodds.cdr.TypeAssignability;
import org.zerodds.typelookup.CompleteTypes;
import org.zerodds.typelookup.TypeLookupService;
import org.zerodds.typelookup.generated.ZdCodec;
import org.zerodds.typelookup.generated.ZdTypes.DDS.XTypes;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.Reader;
import org.zerodds.wire.TryConstruct;

/**
 * The type part of matching a writer with a reader, DDS-XTypes 1.3 §7.6.3.4.2, for the
 * endpoints whose type support pairs by TypeInformation
 * ({@link org.zerodds.cdr.TopicTypeSupport#matchesByTypeInformation()}):
 *
 * <ul>
 *   <li>Step 1: both sides announced a TypeInformation (PID 0x0075). Equal COMPLETE or MINIMAL
 *       identifiers are the same type (§7.2.4.1); otherwise the type of the reader has to be
 *       assignable from the type of the writer (§7.2.4.4), judged on the COMPLETE objects. An
 *       object neither side has at hand makes the verdict {@link Kind#UNRESOLVED}, with the
 *       identifier to ask the peer's TypeLookup service for (§7.6.3.3).
 *   <li>Step 2: one side announced none. The type names decide, as in DDS without XTypes.
 * </ul>
 *
 * <p>The same decision as {@code decide_type_consistency} of the Rust runtime
 * ({@code crates/dcps}), there with the default TypeConsistencyEnforcement (type coercion
 * allowed, {@code force_type_validation} false). A side whose TypeInformation names no COMPLETE
 * identifier is judged by the type names: the Java PSM reads no MINIMAL object.
 */
final class TypeConsistency {

    /** What the types say about the match. */
    enum Kind {
        CONSISTENT,
        INCONSISTENT,
        UNRESOLVED
    }

    /** A verdict, with the reason of an inconsistent one and the object an unresolved one lacks. */
    static final class Verdict {
        final Kind kind;
        final String reason;
        final byte[] missing;

        private Verdict(Kind kind, String reason, byte[] missing) {
            this.kind = kind;
            this.reason = reason;
            this.missing = missing;
        }

        static final Verdict CONSISTENT = new Verdict(Kind.CONSISTENT, "", null);

        static Verdict inconsistent(String reason) {
            return new Verdict(Kind.INCONSISTENT, reason, null);
        }

        static Verdict unresolved(byte[] identifier) {
            return new Verdict(Kind.UNRESOLVED, "", identifier.clone());
        }
    }

    private TypeConsistency() {
    }

    /**
     * @param writerInformation the TypeInformation of the writer, empty for none
     * @param writerTypeName the type name of the writer
     * @param readerInformation the TypeInformation of the reader, empty for none
     * @param readerTypeName the type name of the reader
     * @param objects the serialized TypeObject of an encoded identifier, or {@code null} when it is
     *     not at hand
     */
    static Verdict decide(byte[] writerInformation, String writerTypeName, byte[] readerInformation,
            String readerTypeName, Function<byte[], byte[]> objects) {
        XTypes.TypeInformation writer = information(writerInformation);
        XTypes.TypeInformation reader = information(readerInformation);
        byte[] writerComplete = identifier(writer == null ? null : writer.complete, XTypes.EK_COMPLETE);
        byte[] readerComplete = identifier(reader == null ? null : reader.complete, XTypes.EK_COMPLETE);
        if (writerComplete != null && Arrays.equals(writerComplete, readerComplete)) {
            return Verdict.CONSISTENT;
        }
        byte[] writerMinimal = identifier(writer == null ? null : writer.minimal, XTypes.EK_MINIMAL);
        byte[] readerMinimal = identifier(reader == null ? null : reader.minimal, XTypes.EK_MINIMAL);
        if (writerMinimal != null && Arrays.equals(writerMinimal, readerMinimal)) {
            return Verdict.CONSISTENT;
        }
        if (writerComplete == null || readerComplete == null) {
            // Step 2: a side without a TypeObject to judge. The names decide.
            return writerTypeName.equals(readerTypeName) ? Verdict.CONSISTENT
                    : Verdict.inconsistent("the type names differ and one side announced no COMPLETE "
                            + "TypeIdentifier (XTypes 1.3 7.6.3.4.2, Step 2)");
        }
        DynamicType writerType;
        DynamicType readerType;
        try {
            writerType = CompleteTypes.of(writerComplete, objects);
            readerType = CompleteTypes.of(readerComplete, objects);
        } catch (CompleteTypes.Missing missing) {
            return Verdict.unresolved(missing.identifier());
        } catch (RuntimeException undescribed) {
            return Verdict.inconsistent("a TypeObject is not judged: " + undescribed.getMessage());
        }
        return TypeAssignability.assignableFrom(readerType, writerType) ? Verdict.CONSISTENT
                : Verdict.inconsistent("the reader type is not assignable from the writer type "
                        + "(XTypes 1.3 7.2.4.4)");
    }

    /** The TypeInformation of a PID 0x0075 payload (XCDR2 little-endian), or null for none. */
    private static XTypes.TypeInformation information(byte[] bytes) {
        if (bytes == null || bytes.length == 0) {
            return null;
        }
        try {
            Reader reader = TypeLookupService.reader(bytes, Endianness.LITTLE);
            return ZdCodec.zd_dec_DDS_XTypes_TypeInformation(reader, TryConstruct.DISCARD);
        } catch (RuntimeException unreadable) {
            // A TypeInformation that does not decode is no TypeInformation: the endpoint is
            // judged as one that announced none.
            return null;
        }
    }

    /** The encoded identifier of {@code with} when it is a hash of {@code kind}, else null. */
    private static byte[] identifier(XTypes.TypeIdentifierWithDependencies with, byte kind) {
        if (with == null || with.typeid_with_size == null || with.typeid_with_size.type_id == null
                || with.typeid_with_size.type_id._d != kind) {
            return null;
        }
        return CompleteTypes.encodeIdentifier(with.typeid_with_size.type_id);
    }
}
