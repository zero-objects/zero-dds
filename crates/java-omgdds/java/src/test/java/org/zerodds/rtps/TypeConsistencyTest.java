// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;

import java.util.HashMap;
import java.util.Map;
import java.util.function.Function;
import org.junit.jupiter.api.Test;
import org.zerodds.cdr.HexBytes;
import org.zerodds.cdr.Md5;
import org.zerodds.typelookup.CompleteTypes;
import org.zerodds.typelookup.TypeLookupService;
import org.zerodds.typelookup.generated.ZdCodec;
import org.zerodds.typelookup.generated.ZdTypes.DDS.XTypes;

/**
 * The type part of the match of an endpoint that pairs by TypeInformation (DDS-XTypes 1.3
 * §7.6.3.4.2), on COMPLETE objects built here: equal identifiers, assignability of the reader's
 * type from the writer's (§7.2.4.4), an object not at hand, and the type names where a side
 * announced no TypeInformation. The cells of DDS-RPC 1.0 §7.4.3 and §7.7.2.5 in small.
 */
class TypeConsistencyTest {

    private static final int MUTABLE = XTypes.TypeFlag.IS_MUTABLE;
    private static final int APPENDABLE = XTypes.TypeFlag.IS_APPENDABLE;

    private final Map<String, byte[]> objects = new HashMap<String, byte[]>();
    private final Function<byte[], byte[]> lookup = id -> objects.get(HexBytes.encode(id));

    private static XTypes.TypeIdentifier plain(byte kind) {
        XTypes.TypeIdentifier id = new XTypes.TypeIdentifier();
        id._d = kind;
        return id;
    }

    private static XTypes.TypeIdentifier string() {
        XTypes.TypeIdentifier id = plain(XTypes.TI_STRING8_SMALL);
        id.string_sdefn = new XTypes.StringSTypeDefn();
        id.string_sdefn.bound = 0;
        return id;
    }

    private static XTypes.CompleteStructMember member(long memberId, String name, XTypes.TypeIdentifier type) {
        XTypes.CompleteStructMember m = new XTypes.CompleteStructMember();
        m.common = new XTypes.CommonStructMember();
        m.common.member_id = memberId;
        m.common.member_flags = 0;
        m.common.member_type_id = type;
        m.detail = new XTypes.CompleteMemberDetail();
        m.detail.name = name;
        return m;
    }

    /** A COMPLETE struct object, kept for the lookup; returns its identifier. */
    private XTypes.TypeIdentifier struct(String name, int flags, XTypes.CompleteStructMember... members) {
        XTypes.CompleteStructType s = new XTypes.CompleteStructType();
        s.struct_flags = flags;
        s.header = new XTypes.CompleteStructHeader();
        s.header.base_type = plain(XTypes.TK_NONE);
        s.header.detail = new XTypes.CompleteTypeDetail();
        s.header.detail.type_name = name;
        s.member_seq = members;
        XTypes.TypeObject object = new XTypes.TypeObject();
        object._d = XTypes.EK_COMPLETE;
        object.complete = new XTypes.CompleteTypeObject();
        object.complete._d = XTypes.TK_STRUCTURE;
        object.complete.struct_type = s;
        byte[] serialized = CompleteTypes.encodeObject(object);
        byte[] md5 = Md5.hash(serialized);
        XTypes.TypeIdentifier id = plain(XTypes.EK_COMPLETE);
        id.equivalence_hash = java.util.Arrays.copyOf(md5, 14);
        objects.put(HexBytes.encode(CompleteTypes.encodeIdentifier(id)), serialized);
        return id;
    }

    private static XTypes.TypeIdentifierWithDependencies with(XTypes.TypeIdentifier id) {
        XTypes.TypeIdentifierWithDependencies w = new XTypes.TypeIdentifierWithDependencies();
        w.typeid_with_size = new XTypes.TypeIdentifierWithSize();
        w.typeid_with_size.type_id = id;
        w.typeid_with_size.typeobject_serialized_size = 0;
        w.dependent_typeid_count = 0;
        w.dependent_typeids = new XTypes.TypeIdentifierWithSize[0];
        return w;
    }

    /** The PID 0x0075 payload naming {@code complete} and no MINIMAL identifier. */
    private static byte[] information(XTypes.TypeIdentifier complete) {
        XTypes.TypeInformation info = new XTypes.TypeInformation();
        info.minimal = with(plain(XTypes.TK_NONE));
        info.complete = with(complete);
        org.zerodds.wire.Writer w = TypeLookupService.writer();
        ZdCodec.zd_enc_DDS_XTypes_TypeInformation(w, info);
        return w.finish();
    }

    private TypeConsistency.Verdict decide(XTypes.TypeIdentifier writer, XTypes.TypeIdentifier reader) {
        return TypeConsistency.decide(information(writer), "W", information(reader), "R", lookup);
    }

    @Test
    void equalIdentifiersAreTheSameTypeWithoutAnObject() {
        XTypes.TypeIdentifier pose = struct("Pose", MUTABLE, member(1, "x", plain(XTypes.TK_INT32)));
        objects.clear();
        assertEquals(TypeConsistency.Kind.CONSISTENT, decide(pose, pose).kind);
    }

    @Test
    void aMemberAppendedToAMutableStructureIsAssignableBothWays() {
        XTypes.TypeIdentifier old = struct("Pose", MUTABLE, member(1, "x", plain(XTypes.TK_INT32)));
        XTypes.TypeIdentifier appended = struct("Pose", MUTABLE, member(1, "x", plain(XTypes.TK_INT32)),
                member(2, "y", plain(XTypes.TK_INT32)));
        assertEquals(TypeConsistency.Kind.CONSISTENT, decide(appended, old).kind);
        assertEquals(TypeConsistency.Kind.CONSISTENT, decide(old, appended).kind);
    }

    @Test
    void aParameterOfAnotherTypeIsNotAssignable() {
        XTypes.TypeIdentifier byLong = struct("Svc_add_In", MUTABLE, member(7, "a", plain(XTypes.TK_INT32)));
        XTypes.TypeIdentifier byString = struct("Svc_add_In", MUTABLE, member(7, "a", string()));
        assertEquals(TypeConsistency.Kind.INCONSISTENT, decide(byLong, byString).kind);
        assertEquals(TypeConsistency.Kind.INCONSISTENT, decide(byString, byLong).kind);
    }

    @Test
    void aReturnTypeWidenedToLongLongIsNotAssignable() {
        XTypes.TypeIdentifier byLong = struct("Svc_scale_Out", MUTABLE,
                member(3, "return_", plain(XTypes.TK_INT32)));
        XTypes.TypeIdentifier byLongLong = struct("Svc_scale_Out", MUTABLE,
                member(3, "return_", plain(XTypes.TK_INT64)));
        assertEquals(TypeConsistency.Kind.INCONSISTENT, decide(byLongLong, byLong).kind);
    }

    @Test
    void aRequestOfTheBaseInterfaceIsAssignableToTheDerivedOne() {
        // The operations of an interface are the members of its @choice request type, the
        // parameters of each an empty or filled _In structure (DDS-RPC 1.0 7.5.1.2).
        XTypes.TypeIdentifier baseIn = struct("Base2_ident_In", MUTABLE);
        XTypes.TypeIdentifier derivedIn = struct("Derived2_ident_In", MUTABLE);
        XTypes.TypeIdentifier moreIn = struct("Derived2_more_In", MUTABLE);
        XTypes.TypeIdentifier base = struct("Base2_Request", MUTABLE, member(0x11, "ident", baseIn));
        XTypes.TypeIdentifier derived = struct("Derived2_Request", MUTABLE, member(0x11, "ident", derivedIn),
                member(0x22, "more", moreIn));
        assertEquals(TypeConsistency.Kind.CONSISTENT, decide(base, derived).kind);
        assertEquals(TypeConsistency.Kind.CONSISTENT, decide(derived, base).kind);
    }

    @Test
    void anEmptyStructureIsNotAssignableFromOneWithMembers() {
        XTypes.TypeIdentifier empty = struct("E", APPENDABLE);
        XTypes.TypeIdentifier filled = struct("E", APPENDABLE, member(0, "x", plain(XTypes.TK_INT32)));
        assertEquals(TypeConsistency.Kind.INCONSISTENT, decide(filled, empty).kind);
    }

    @Test
    void anInt8IsNotAnOctet() {
        XTypes.TypeIdentifier int8 = struct("B", MUTABLE, member(1, "b", plain(XTypes.TK_INT8)));
        XTypes.TypeIdentifier octet = struct("B", MUTABLE, member(1, "b", plain(XTypes.TK_BYTE)));
        XTypes.TypeIdentifier uint8 = struct("B", MUTABLE, member(1, "b", plain(XTypes.TK_UINT8)));
        assertEquals(TypeConsistency.Kind.INCONSISTENT, decide(int8, octet).kind);
        assertEquals(TypeConsistency.Kind.INCONSISTENT, decide(uint8, int8).kind);
    }

    @Test
    void anObjectNotAtHandIsTheOneToAskFor() {
        XTypes.TypeIdentifier inner = struct("Inner", MUTABLE, member(1, "x", plain(XTypes.TK_INT32)));
        XTypes.TypeIdentifier writer = struct("Outer", MUTABLE, member(5, "inner", inner));
        XTypes.TypeIdentifier reader = struct("Outer", MUTABLE, member(5, "inner", inner),
                member(6, "more", plain(XTypes.TK_INT32)));
        byte[] innerId = CompleteTypes.encodeIdentifier(inner);
        objects.remove(HexBytes.encode(innerId));
        TypeConsistency.Verdict verdict = decide(writer, reader);
        assertEquals(TypeConsistency.Kind.UNRESOLVED, verdict.kind);
        assertArrayEquals(innerId, verdict.missing);
    }

    @Test
    void withoutTypeInformationTheTypeNamesDecide() {
        XTypes.TypeIdentifier pose = struct("Pose", MUTABLE, member(1, "x", plain(XTypes.TK_INT32)));
        assertEquals(TypeConsistency.Kind.CONSISTENT,
                TypeConsistency.decide(new byte[0], "Pose", information(pose), "Pose", lookup).kind);
        assertEquals(TypeConsistency.Kind.INCONSISTENT,
                TypeConsistency.decide(information(pose), "Pose", new byte[0], "Other", lookup).kind);
    }
}
