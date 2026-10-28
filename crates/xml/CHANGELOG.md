# Changelog — `zerodds-xml`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- The nesting scan before `roxmltree` is `zerodds_foundation::xml_depth::check_nesting`; the copy
  the parser carried is gone. Same limit (`MAX_TREE_DEPTH`) and the same `LimitExceeded` message.

### Added
- The XSD Type Representation follows the IDL-to-XSD mapping (CORBA to WSDL/SOAP Interworking 1.2.1, 4.1.7)
  that §7.3.3 refers to: the element of a sequence or an array is `item` (`item1`, `item2`, ... for further
  dimensions, `element` is still read), an array declared as a field is the type
  `<enclosing>.<field>_ArrayOf<element>`, and the dimensions nest as the mapping shows for `long matrix[5][3]`
  (the first dimension is the innermost type; intermediate types `ArrayOf<element>`, suffix `_n` for a name taken
  by other dimensions). An alias of a complex type is a restriction that states the content again, a string
  member is `nillable`, the `maxLength` of a bounded string is `fixed`, the branches of a union are
  `minOccurs="0"`. The SOAP-encoding twins (`_SE_`, `soapenc:Array`) are skipped on reading.
- `schemas::XTYPES_TYPES_XSD`, `XTYPES_TYPES_NONAMESPACE_XSD` and `XTYPES_TYPES_NAMESPACE`: the XSD of Annex A
  of DDS-XTypes 1.3 (`http://www.omg.org/dds`). An element in that namespace is read by its schema:
  `extensibility="appendable"` is the spelling, `append` an error.
- `nonBasicTypeName` beside a `type` other than `nonBasic` is an error; `name`, `baseType`, `nonBasicTypeName`
  are checked against `identifierName` (`is_identifier_name`), `id` against 0..=0x0FFFFFFF, `bitBound` against
  1..=32 (enum) and 1..=64 (bitmask), a flag's `position` is required and 0..=63.
- `xsd_schema::parse_xsd_schema` reads the XSD Type Representation of §7.3.3: annotation types and
  applications (`appinfo`), the built-in annotations, root elements, unions, bitmasks, sequences, arrays and
  maps; `xsd_writer::type_library_to_xsd` writes a `TypeLibrary` as such a schema. `AnnotationMemberDecl` has
  `default_value`. `xs:byte` is `int8` (it was `octet`).
- `qos_parser::parse_qos_library_element_with`, `qos_parser::parse_entity_qos_with` and `xtypes_parser::parse_types_element_with`: the element readers with `ReadOptions`, returning `Parsed` with the diagnostics. The `*_public` and `parse_types_element` functions read with the default options.
- `serialize_data_representation` and `parse_data_representation`: the XML
  Data Representation of DDS-XTypes 1.3 §7.4.4. The root element is named
  after the type, and the Valid variant (§7.4.4.1) declares the implied
  namespace `ddstype://www.omg.org/<module path>` (`implied_namespace`) as
  the default namespace, the Well-formed variant (§7.4.4.2) none. The reader
  refuses a root of another name and a namespace of another module path and
  says which variant it read. `serialize_sample` keeps the DDS-XML 1.0
  `<sample type_ref>` form.

### Fixed
- A document that opens thousands of elements overflowed the stack inside
  the XML parser before the depth cap was checked, and aborted the process.
  `parse_xml_tree`, which every loader calls, now scans the source
  iteratively for nesting deeper than `MAX_TREE_DEPTH` (skipping comments,
  CDATA, processing instructions, the DOCTYPE and quoted values) before the
  parser runs, and checks the parsed tree again for what entity expansion
  added.
- Forms other DDS products write, and that the DDS-XML 1.0 schemas do not
  define, are refused with `XmlError::VendorForm`, which names the path, the
  form and the specification's form: `<topic_filter>` as a child element,
  `qos_profile_ref` on `topic`, `data_writer` and `data_reader`,
  `topic_filter` on `topic`, the container names `domainparticipant_qos` and
  `participant_qos`, a duration written as text, the IDL names of primitives
  in `type` (`long`, `octet`, ...), a type name in `type` without `nonBasic`,
  `nonbasic`, the `discriminator` attribute of `union`, `<bit_value>`,
  `<bitset>`, `base_type`, `must_understand`, `forwardDcl`, an upper-case or
  absent `kind` of a forward declaration, `extensibility="appendable"`.
  `ReadOptions::vendor_forms = VendorForms::Accept` (or
  `ReadOptions::accept_vendor_forms()`) reads them as before and adds a
  `Diagnostic` of kind `DiagnosticKind::VendorForm` for each use; it is
  independent of `UnknownElements`.
- The Minimal TypeObject of a typedef of a sequence, an array or a map is an
  alias type whose related type is the plain collection identifier (XTypes
  1.3 §7.3.4.6, §7.3.4.8), as the Complete one is. It was a Minimal sequence
  or array type, which names no type of its own.
- QoS `kind` and `access_scope` values accepted 18 short spellings
  (`RELIABLE`, `KEEP_LAST`, `VOLATILE`, …) beside the DCPS IDL enumeration
  literals. §7.1.4 Tab.7.1 makes the IDL literals the only legal values and
  §7.2.1.1 spells them out; the short forms are refused now, in the reader
  and in the schema (whose `default=` attributes carry the IDL literals
  too). The `extensibility` attribute of `<struct>`/`<union>` likewise takes
  only XTypes 1.3 §7.3.3's lowercase `final`/`appendable`/`mutable`.
- A participant's `<register_type>` and `<topic>` children were read as
  references into the domain (`ref` or `name` attribute), while §7.3.5.4.2
  plus the Domain building block's complex types — which the
  DomainParticipant schema reuses — type them as definitions. They are
  parsed with the same required attributes as in a `<domain>`
  (`type_ref`/`register_type_ref`), join the ones the referenced domain
  makes available, and override a domain entry of the same name; a
  participant that names only `domain_id` resolves writers at its own
  topics.
- `parse_bool` accepted `TRUE`/`FALSE`. §7.1.4 Tab.7.1 makes the four
  spellings `1`/`true`/`0`/`false` case-sensitive; the exported helper now
  follows that, like `parse_bool_strict`.
- The XML declaration's `encoding` was discarded (roxmltree takes a
  `&str`), so a document declaring e.g. ISO-8859-1 was read as UTF-8.
  §7.1.1 requires UTF-8; another declaration is refused. A `data:` URI
  body that is not UTF-8 was silently turned into U+FFFD; it is now
  rejected.
- `domain_id` was capped at 232 on `<domain>` (a DDS convention not in
  DDS-XML) and not on `<domain_participant>`. §7.1.5 Tab.7.2 types it as
  `unsigned long`; the cap is gone, so both accept the field's range.
- The strict-mode namespace check was a substring search, so the
  namespace string in a comment or attribute value satisfied it
  (§7.3.1). It now reads the namespace of the parsed root element.
- A well-formed document with a DTD was rejected (`allow_dtd: false`),
  and a byte-level depth pre-check could count `<` inside CDATA or
  comments as tags. §7.1.1 asks for well-formedness; DTDs are accepted
  (external entities are still neither fetched nor resolved), and the
  depth is measured on the parsed tree. Because the parser expands an
  internal entity without a size limit, a value over
  `MAX_ENTITY_VALUE_BYTES` (256), an entity that refers to another
  entity and more than `MAX_ENTITY_DECLARATIONS` (64) declarations are
  refused with `LimitExceeded`.
- Octet sequences in `user_data`/`topic_data`/`group_data` were read as
  Base64 from `<value>`. §7.2.4.2 distinguishes the two representations
  by element name: `<value>` is the comma-separated decimal/hex list and
  `<valueB64>` is Base64. Both are now read, and both together are
  rejected.
- `<partition>` was read from repeated `<name>` children (and a vendor
  `<name_list>`). Per §7.2.4.1 the sequence's `complexType` holds
  `<element>` children, so `<partition><name><element>g1</element></name>`
  is the form read; the repeated spelling is rejected.
- An inline QoS container's `base_name` attribute (§7.3.5.4.4) was
  ignored. `<topic_qos base_name="…">` and the writer/reader/publisher/
  subscriber/participant containers now inherit the named profile's
  matching container before applying their own policies.
- Sixteen QoS structure fields were required when the spec makes them
  optional (`minOccurs="0"`, §7.2.3 / §7.2.6) and the DDS defines a
  default. A present-but-empty policy such as `<deadline/>` or a
  `<duration>` without `<sec>` is now read at its DDS default instead of
  being rejected.
- `Duration_t` only accepted `DURATION_INFINITY`/`DURATION_INFINITE_SEC`
  and capped `nanosec` at 999 999 999. §7.2.2 names
  `DURATION_ZERO_*`/`TIME_INVALID_*` as legal values and §7.2.2.1 lets
  `nanosec` be any decimal; the numeric infinite duration
  (`2147483647`/`2147483647`) now reads as infinity too.
- Data Samples did not check the declared field lengths: an array
  accepted any item count, a bounded sequence accepted more than
  `sequenceMaxLength`, and a multi-dimensional array was read as one
  flat level. §7.3.7.3.2 fixes the array item count (nested per
  dimension) and bounds the sequence; both are enforced now.
- `register_type` accepted the registration name as the type name when
  `type_ref` was absent, and never checked that the referenced type
  exists. §7.3.4.4.1 makes `type_ref` mandatory and requires the type to
  have been defined; it is now required, and resolving a topic checks it
  against the `<types>` blocks.
- Data Samples read `long`/`short`/`long long` as decimal only, accepted
  any enum literal, and demanded a `<discriminator>` in a union. §7.1.4
  Tab.7.1 / §7.2.2 allow the hex spelling and the named constants, §7.1.4
  bounds an enum to its IDL literals, and §7.3.7.4.2 shows a union without
  a discriminator. All three are read now; the union serializer writes the
  active case member instead of a generic `<value>`.
- Unknown or misplaced children were skipped in the Domains,
  DomainParticipants and Applications readers, so a `<data_reader>` under
  a `<publisher>` or a misspelt container vanished. §7.3.5.4.1 nests each
  entity under its parent; a child that is not one is reported.
- A DataWriter without QoS of its own took over its Publisher's
  `<publisher_qos>`, a DataReader its Subscriber's. §7.3.5.4.4: inline QoS
  applies only to the entity that owns it. Both now stay at the DDS
  defaults when the entity names no QoS.
- Policies that are not fields of a QoS container (DDS 1.4 §2.2.3, mapped
  1:1 by §7.2) were read and silently dropped on materialization, e.g.
  `<datareader_qos><ownership_strength>`. They are refused now.
- A domain inherited only the `domain_id` of its base. §7.3.4.4.2
  inherits the topics and registered types as well;
  `DomainLibrary::effective_domain` merges the whole chain.
- Element text was trimmed and only the first text run was kept, so
  `<name><!-- c -->A</name>` read as empty and `x<!-- c -->y` as `x`. The
  full concatenated text is kept (§7.1.4 Tab.7.1 `string`), and a
  structure field that occurs twice is reported instead of silently
  taking the first.
- The Types reader turned `type="nonBasic"` into a type literally named
  `nonBasic` and dropped map attributes. It reads `nonBasicTypeName`,
  `mapKeyType`/`mapKeyNonBasicTypeName`, `mapMaxLength` and
  `mapKeyStringMaxLength`, and `-1` bounds as unbounded
  (XTypes 1.3 §7.3.2.4).
- **Every shipped XSD rejected every conforming DDS-XML document.** All
  seven chameleon schemas carried `elementFormDefault="unqualified"`,
  so a document that puts its elements in
  `http://www.omg.org/spec/DDS-XML` — as the specification's own five
  example documents do — failed on its first child element. The
  schemas are `pub` (`zerodds_xml::schemas`), so this was output.
- `<domain>` required a `domain_id`. §7.3.4.4.2 writes an inheriting
  domain without one (`<domain name="MyDomain" base_name="BaseDomain">`)
  and the schema marks the attribute optional. `DomainEntry::domain_id`
  is now `Option<u32>`; `DomainLibrary::effective_domain_id` follows the
  `base_name` chain.
- `topic_filter` was read only as a `<topic_filter>` child element, a
  vendor spelling. §7.3.2.4.3 puts it as an attribute on the QoS
  container (`<datawriter_qos topic_filter="A*">`); that form fell
  through the lax branch and the filter was lost. The attribute is now
  the container's own filter; the vendor `<topic_filter>` child element
  remains the profile's default for containers without an attribute.
- A `<qos_profile>` that repeats a QoS container kept the last one and
  dropped the rest without a word. §7.3.2.4.3 allows several
  DataWriter/DataReader/Topic containers per profile, each with its own
  `topic_filter`; all are kept now, and the topic name selects between
  them (the first explicit filter that matches, else the filterless
  container). A repeated Publisher/Subscriber/DomainParticipant
  container has no such selection and is refused by name.
- Schema content models corrected against the specification:
  `domain_participant` carries `register_type`, `topic`, `publisher`,
  `subscriber`, `domain_participant_qos` and the attributes `base_name`,
  `domain_ref`, `domain_id`; `topic/@register_type_ref` is required
  (§7.3.4.4.1); `register_type/@type_ref` is required (§7.3.4.4.1);
  `application`
  holds full participant definitions; QoS containers may repeat and
  appear in any order; the profile's participant container is named
  `domain_participant_qos`.
- A QoS profile, domain or participant could inherit from a `base_name`
  defined later in the document. §7.3.2.4.2, §7.3.4.4.2, §7.3.5.4.3 and
  §7.3.6.4.2 require the base to have been defined before; forward
  references are refused now.
- `domain_ref` resolved only as `library::name`, and a participant
  without `domain_ref` could not be resolved even when it named a
  `domain_id`. §7.3.5.4.2's examples write `domain_ref="MyDomain"`
  unqualified and allow the Domain to be named numerically; both read
  now (an unqualified name searches the domain libraries in document
  order).
- The schema files were named differently from the specification's
  normative files (`types`, `domains`, `domain_participants`,
  `applications` instead of `type`, `domain`, `domainparticipant`,
  `application`; §7.1.2, §7.3.2.3, §7.3.4.3, §7.3.5.3, §7.3.6.3), and
  the QoS chameleon typed every entity QoS as `xs:any`, so no policy,
  field or enumeration was checked. The files carry the specification's
  names now; the QoS containers are typed per DDS 1.4 §2.2.3 field
  lists with the §7.2.1 enumerations and the §7.2.3 defaults; the
  Domains/Participants schemas gain `qos_profile_ref` and `topic_filter`
  on `<topic>` (and `qos_profile_ref` on `<data_writer>`/
  `<data_reader>`), lose the `registered_name` child of `<topic>` and
  the `content_filter` element the reader refuses.
- `sequenceMaxLength="-1"` (XTypes 1.3 §7.3.2.4.2, the unbounded
  sequence) was refused as a name that is no constant. It reads as
  `Some(0)` in members and typedefs, so the member stays a sequence, and
  the sample reader and the TypeObject bridge treat it as unbounded.
- Map attributes were read on struct members only, and the TypeObject
  bridge lowered a map member to its value type. A member with
  `mapKeyType` becomes a `PlainMapSmall`/`PlainMapLarge`; `typedef` reads
  the same attributes (`TypedefType::map_key`, `map_max_length`,
  `map_key_string_max_length`) and lowers to an alias of the map.
  `mapKeyNonBasicTypeName` on a basic key and `mapKeyStringMaxLength` on
  a non-string key are refused (XTypes 1.3 §7.3.2.4.3). A member naming
  `sequenceMaxLength` and `arrayDimensions` is an array of sequences
  (§7.3.2.4.4); the sequence was dropped.
- Data Samples: an element of a structure that is no member and a member
  given twice were skipped, the second one dropped. Both are errors now
  (§7.2.3). The members of the `baseType` chain were never read; a derived
  structure's sample takes them (XTypes 1.3 §7.3.2.5). An `unsigned long`
  member takes the §7.2.2.1 constants of that type
  (`DURATION_INFINITE_NSEC`, `DURATION_ZERO_NSEC`, `TIME_INVALID_NSEC`),
  and an unbounded sequence takes any number of items.
- A second inline QoS container in one entity (`<datawriter_qos>` twice
  in a `<data_writer>`, likewise for the topic, publisher, subscriber and
  participant) replaced the first without a word. It is an error now
  (§7.3.5.4.4).
- The Types reader required `discriminator="…"` as an attribute of
  `<union>` and `<bit_value>` for bitmask flags. XTypes 1.3 (§7.3.2.5.2,
  §7.3.2.7.2, Annex A) writes the discriminator as a
  `<discriminator type=".." nonBasicTypeName=".."/>` child and the flags
  as `<flag>`; both forms are read now, the older ones as well.
- A `<qos_library>` read its `<qos_profile>` children first and the
  single-QoS shortcuts (§7.3.2.4.4) after them, so the document order that
  §7.3.2.4.2 "defined before" depends on was lost: a profile inheriting a
  shortcut written before it was refused, one inheriting a shortcut
  written after it was accepted. Profiles and shortcuts keep document
  order now.
- `base_name` on a QoS container inside a `<qos_profile>` was read and
  dropped. §7.3.5.4.4 gives it to the QoS of an inline entity; in a
  profile it is refused (the profile inherits through its own
  `base_name`). The shortcut form's `base_name` is the profile's.
- The vendor `<topic_filter>` element (profile and `<topic>`) took the
  layout whitespace around the pattern as part of it; it is trimmed.
- Base64 (`<valueB64>`) accepted `=` padding before the last group and
  three `=` in a group (`xs:base64Binary`).
- A structure element (a policy, a `Duration_t`, the `<name>` of a
  partition) that held text beside or instead of its child elements read
  the text as nothing: `<partition><name>g1</name></partition>` was a
  partition without names. It is an error now (§7.2.3, §7.2.4.1); the
  sentinel text of a `Duration_t` stays its own form.
- An empty `<reliability/>` was BEST_EFFORT in every container. §7.2.3
  gives an omitted field the default of the DDS, and §2.2.3.14 makes it
  RELIABLE for a DataWriter and BEST_EFFORT for a DataReader and a Topic;
  the reader follows the container.
- `dds-xml_type_definitions_nonamespace.xsd` was `xs:any` with
  `processContents="lax"` (a `<types>` block of anything validated). It
  is the XML Type Representation schema of DDS-XTypes 1.3 Annex A now,
  written out from the text of that annex (the OMG's file is not in the
  tree; four attribute start tags the print leaves open are closed).
  `tests/schema_validation.rs` gains two negative rows for it.
- `char8`, `char16`, `int8` and `uint8` (XTypes 1.3 Annex A
  `allTypeKind`) were read as the names of types that do not exist.
  `char8`/`char16` are `char`/`wchar`; `int8` and `uint8` are new
  `PrimitiveType::Int8`/`UInt8`, with their own range in Data Samples and
  their own primitive kinds in the TypeObject bridge.
- `dds-xml_{qos,type,domain,domainparticipant,application,dds_system}_definitions{,_nonamespace}.xsd`
  are the normative files of the specification
  (`http://www.omg.org/spec/DDS-XML/20180501/`) byte for byte, where they
  had been reconstructions from the text. They include each other by
  absolute URL; validate offline with an XML catalog (`rewriteURI`), as
  the tests do. The five example documents of the specification validate
  against them. The Data Samples files and `dds-xml_common.xsd` stay
  ours (§7.3.7.3.1: no generic schema exists for samples).
- The readers follow those schemas where they had followed the text
  alone: `initial_samples`/`initial_instances` of `<resource_limits>`
  (carried in `EntityQos::initial_resource_limits`), `format` and
  `<registered_name>` of `<register_type>`, `<registered_name>` of
  `<topic>`, `<content_filter>`, `<status_condition>`, `<read_condition>`
  and `<query_condition>` of `<data_reader>` (`DataReaderEntry`,
  `FilterDef`, `ReadConditionDef`), a QoS container without `name` directly
  in a `<qos_library>`, every QoS container repeated in a profile (same
  filter: merged), `base_name` on a container inside a profile (resolved
  with the profile), numeric elements that are empty (`<sec></sec>`, the
  value of an omitted field), `<value>` of `user_data`/`topic_data`/
  `group_data` as `xs:base64Binary`.
- Unknown elements and attributes (an RTI file's `is_default_qos`,
  `<publish_mode>`, …), and policies in a container that does not have
  them, are an error that names the path, as before. `ReadOptions` with
  `UnknownElements::ReportAndSkip` (`parse_qos_libraries_with`,
  `parse_domain_libraries_with`, `parse_domain_participant_libraries_with`,
  `parse_application_libraries_with`, `parse_dds_xml_with`) leaves them
  out and returns each as a `Diagnostic` with its path
  (`dds/qos_library[L]/qos_profile[P]/datawriter_qos/publish_mode`).
  `XmlElement::path` carries that path.
- The Types reader follows the grammar of the normative schema
  (`dds-xml_type_definitions_nonamespace.xsd`): `<annotate>` and
  `<verbatim>` children, `<annotation>` declarations, `<const>` inside a
  `<struct>`, the attributes `nested`, `autoid`, `external`,
  `tryConstruct`, `nonSerialized`, `defaultLiteral`, `extensibility="append"`
  (`appendable` still read), the `extensibility` of a `<union>` (it was
  dropped), `<include2>`, `forward_dcl` kinds. All of it is read into the
  model (`Annotation`, `Verbatim`, `AnnotationTypeDef`, `TryConstruct`,
  `AutoId`, new fields on `StructType`, `StructMember`, `UnionType`,
  `EnumType`, `EnumLiteral`, `BitmaskType`, `BitValue`, `TypedefType`,
  `ModuleEntry`, `ConstEntry`). An element or attribute outside the grammar
  is refused with its path (or reported under `ReportAndSkip`:
  `parse_type_libraries_with`); it was skipped without a word.
- Bound attributes (`stringMaxLength`, `sequenceMaxLength`,
  `mapMaxLength`, `mapKeyStringMaxLength`), `arrayDimensions`, the `value`
  of an `<enumerator>` and a `<caseDiscriminator>` are constant
  expressions (XTypes 1.3 §7.3.2.4; IDL `const_exp`): literals (decimal,
  `0x`, octal), names of `<const>`s with IDL scoping (also those of a
  module or struct), `LENGTH_UNLIMITED` and the other constants of
  §7.2.2.1, `+ - * / % << >> & ^ | ~` and parentheses. `-1` is unbounded,
  any other negative bound is an error.
- The TypeObject bridge builds in dependency order for
  `bridge_library` and `bridge_library_complete`: a reference carries the
  identifier its target has in the result at any depth (the pre-pass hashed
  a type before its own references were resolved); a name that is no type
  is `UnresolvedReference` (it was a zero hash); a cycle among the types is
  `UnsupportedXsdConstruct`; `<const>`, `<forward_dcl>` and `<annotation>`
  entries no longer make the library fail.
- A `typedef` of a sequence, an array or a map has a Complete TypeObject
  now: an alias whose related type is the plain collection identifier
  (XTypes 1.3 §7.3.4.6); it was refused. `external`, `tryConstruct`,
  `nested`, `autoid="hash"`, `defaultLiteral` and the extensibility of a
  union reach the TypeObject flags. Annotations, `<verbatim>` and
  `nonSerialized="true"` have no place in a TypeObject and are refused by
  name.

### Added
- `ReadOptions::vendor_forms`, `VendorForms`, `ReadOptions::accept_vendor_forms()`,
  `ReadOptions::with_vendor_forms()`, `DiagnosticKind::VendorForm` and
  `XmlError::VendorForm`.
- `QosProfileRegistry::from_xml_with` / `from_file_with` return the registry
  with the diagnostics of a `ReadOptions::report_and_skip()` read, and
  `from_libraries` builds one from libraries already read. A `<types>` block
  inside `<dds>` now follows the same options as the libraries.
- Doc comments cite the sections the specification has: the primitive
  constants and the schema simple types sit in §7.2.2.1, the building blocks
  in §7.3.1, the DDS system block set in §8.1; QoS policy references use the
  DDS 1.4 §2.2.3 numbering; references to internal planning documents and
  to a "Cluster" scheme are gone.
- `tests/schema_validation.rs` — DDS-XML documents measured by `xmllint`
  against the XSDs, in both directions: what this crate writes must
  validate, and what the schema rejects must be rejected here too. The
  documents this crate reads against its own schema are listed with the
  reason for each. Runs against the OMG's own schemas as well when
  `ZERODDS_OMG_DDSXML_DIR` is set.
- `bridge_library_complete_with_known(lib, known)`: like
  `bridge_library_complete`, for a `<types>` library whose types refer to
  types that are not part of it but already known, as the DDS-XRCE 1.0
  §7.7.3.3.2 allows (`known` maps names to `CompleteTypeObject`s; the
  references become the `EquivalenceHashComplete` identifiers those
  objects have). A name that resolves to nothing is an
  `UnresolvedReference` naming it, identifiers are computed in
  dependency order so that references hold across more than one level,
  names are looked up like IDL scoped names, and a cycle among the types
  of the library is refused.
- `QosProfileRegistry::writer_qos_for_inline` and `reader_qos_for_inline`
  materialize an inline QoS container that names a `base_name`
  (§7.3.5.4.4): the profile's container for the topic is the base and the
  inline policies override it.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization as a Layer-3 schema crate.

### Spec-Referenzen
- OMG DDS-XML 1.0 — Parser + QoS-Profile-Loader + Building-Block-Foundation.

### RC1-Audit
- License-Header (SPDX-Apache-2.0) auf alle src-Files.
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Phase-X markers cleaned up (§1.13).

### Properties
- 302 tests green; clippy clean.
- Pure-Rust, std-only (build-time tool, no embedded use case).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
