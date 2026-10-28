# TypeLookup protocol source

These two unmodified IDL files come from Eclipse Cyclone DDS 11.0.1,
commit `e54e991f75a3e67f8e628da3171122e36ea5b872`, paths
`src/core/ddsi/idl/ddsi_xt_typelookup.idl` and `ddsi_xt_typeinfo.idl`.
They were read from the existing test-host vendor checkout, not fetched externally.
The upstream LICENSE and NOTICE accompany them unchanged.

`scripts/generate-typelookup.sh` runs the shared ZeroDDS IDLC on the complete
schema and places the generated Java classes in their package. Package placement
is mechanical. There is no separate handwritten wire codec or reduced schema.
The runtime uses both service operations and the same registry metadata emitted
by IDLC. The builtin endpoints use the shared reliable RTPS history, ordered
receive windows, fragmentation and fragment repair. All four service endpoint
bits are advertised. Java-to-Java service and deliberately lost-fragment repair
are verified separately from vendor byte compatibility.

`ZdCodec` holds the codecs in both encodings: XCDR2 (CDR2_LE) at its top
level and XCDR1 (CDR_LE) in the nested `ZdCodec.Xcdr1`, which the generator
writes for the RPC request and reply types. CDR2_LE is the default of the
TypeLookup endpoints (DDS-XTypes 1.4 Beta 1 §7.6.3.3.3); CDR_LE is the setting
`wire.type_lookup_encoding=xcdr1` (`TypeLookupService.Encoding`, DDSI-RTPS 2.5
§10.4 Table 10.2, DDS-XTypes 1.3 Table 60). The envelopes `TypeLookup_Request` and
`TypeLookup_Reply` are FINAL (DDSXTY14-30), and `TypeLookupService` writes and
reads them as their header followed by their call or return, each with the
generated codec of the encoding. Both files come from the current
`zerodds-idlc` through `scripts/generate-typelookup.sh`. Against the codecs of
`31574259c75` the differences are the generator's own since then: an octet is a
Java `byte` (CORBA 3.3 part 3 §1.5.2) where it was an `int`, the octet constant
`EK_MINIMAL` is `(byte) 0xF1`, an enumeration is a class after §1.5.9
(`RemoteExceptionCode`), the classes have `equals` and `hashCode`, and the RPC
envelopes have no codec of their own in `ZdCodec`.
