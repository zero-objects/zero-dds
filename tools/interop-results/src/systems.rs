// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The columns: who they are, what drives them, and what each one
//! cannot be asked by construction.
//!
//! A structural limit belongs here and not in a cell: `rmw_cyclonedds_cpp`
//! writes the host's byte order and takes no argument for it, so half
//! its column does not exist, and that is a property of the
//! implementation rather than a result of any run. Recorded once, cited
//! by every cell it explains.

/// What kind of thing a column is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One of this project's three language columns.
    Own,
    /// A DDS vendor's own generated serialiser.
    Dds,
    /// A CORBA ORB. XCDR1 *is* CORBA CDR — CORBA 3.3 part 2 §9.3.
    Orb,
    /// A ROS 2 type support, which is neither its vendor nor a DDS
    /// implementation: it generates its own serialisation.
    Ros,
}

/// One column.
#[derive(Debug, Clone, Copy)]
pub struct System {
    /// The directory name under a case, and the key in every source
    /// TSV. Stable: a rename would move every file in the tree.
    pub id: &'static str,
    /// The heading the tables print.
    pub label: &'static str,
    /// Which family the column belongs to.
    pub kind: Kind,
    /// The product and version the recorded rows were taken from.
    pub version: &'static str,
    /// The command that regenerates this column's source TSV.
    pub producer: &'static str,
    /// Whether a machine with no licensed toolchain can reach it.
    pub reachable_without_licence: bool,
    /// Encoding version 2 at all. The ORBs and both ROS type supports
    /// have none: Fast CDR 1.0.24 has none and CORBA CDR predates
    /// XTypes.
    pub has_version_2: bool,
    /// The byte orders this column can write. Two of the three ORBs
    /// write one each and say so themselves.
    pub writes_orders: &'static [&'static str],
    /// What the column is, in one line, for `SYSTEMS.md`.
    pub note: &'static str,
    /// Whether a case and its `-unannotated` twin are the **same
    /// type** to this column, so that the same sample must give the
    /// same octets.
    ///
    /// True where the front end drops the annotations rather than
    /// honouring them: every ROS message is a `@final` XCDR1 struct
    /// whatever its `.idl` says, and CORBA IDL has no extensibility at
    /// all. False for the DDS vendors, where the extensibility the
    /// twins vary is exactly what the pair exists to measure, and for
    /// this project's own columns, which honour it.
    ///
    /// `crate::reproducible` is what reads it, and what it buys: a
    /// recorded octet no re-run reproduces is otherwise invisible to
    /// `--verify`, which compares the tree against these sources and
    /// never against a fresh sweep.
    pub twins_are_one_type: bool,
    /// How many write cells this column carried bytes for when its
    /// source was last recorded.
    ///
    /// A floor and not an equality: a case added to the corpus that
    /// this column refuses leaves it where it is, and one it takes
    /// raises it. What it catches is the fall — a sweep that reports
    /// success and returns nothing. That happened: the three CORBA
    /// sweeps ran to completion, exited 0 and produced no rows, because
    /// a variable was assigned and never exported, and the run was read
    /// as a matrix in which three ORBs agreed with everyone. An exit
    /// code cannot tell those apart. A count can.
    pub bytes_floor: usize,
}

/// The columns, in the order the tables print them.
pub const SYSTEMS: [System; 18] = [
    System {
        id: "cyclone",
        label: "Cyclone",
        kind: Kind::Dds,
        version: "Eclipse Cyclone DDS 11.0.1",
        producer: "tools/interop-results/record.sh --vendors cyclone",
        reachable_without_licence: true,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "`idlc` generates the type support; the probe calls `dds_stream_write_sample`.",
        twins_are_one_type: false,
        bytes_floor: 872,
    },
    System {
        id: "rti",
        label: "RTI",
        kind: Kind::Dds,
        version: "RTI Connext DDS 7.7.0, rtiddsgen 4.7.0",
        producer: "tools/interop-results/record.sh --vendors rti",
        reachable_without_licence: false,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "Licensed and machine-bound. `rtiddsgen -language C` plus \
               `<T>Plugin_serialize_to_cdr_buffer_ex`.",
        twins_are_one_type: false,
        bytes_floor: 868,
    },
    System {
        id: "fastdds",
        label: "Fast DDS",
        kind: Kind::Dds,
        version: "eProsima Fast DDS 3.6.1 / Fast CDR 2.3.5",
        producer: "tools/interop-results/record.sh --vendors fastdds",
        reachable_without_licence: true,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "`fastddsgen` plus Fast CDR's `Cdr` in `XCDRv1` / `XCDRv2`.",
        twins_are_one_type: false,
        bytes_floor: 1112,
    },
    System {
        id: "opendds",
        label: "OpenDDS",
        kind: Kind::Dds,
        version: "OpenDDS 3.34.0 / ACE 6.5.24",
        producer: "tools/interop-results/record.sh --vendors opendds",
        reachable_without_licence: true,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "The only IDL front end of the five whose `map` compiles, and the only DDS \
               implementation here with CORBA/TAO lineage: `opendds_idl --idl-version 4` plus \
               `tao_idl -Sa -St` produces the classic C++ mapping, so `wireprobe-opendds` \
               takes the same CORBA C++ fill the omniORB and TAO probes take. \
               `OpenDDS::DCPS::Serializer`, whose `Encoding` carries both the version and the \
               byte order, so one build answers all four combinations. Its 504 refusals are \
               the CORBA C++ fill's and are worded as the probe's, not the vendor's: \
               OpenDDS's own front end parses `map`, and `wireprobe-fill`'s CORBA C++ output \
               has no form for one.",
        twins_are_one_type: false,
        bytes_floor: 836,
    },
    System {
        id: "hdds",
        label: "HDDS",
        kind: Kind::Dds,
        version: "HDDS, hddsgen 1.2.0",
        producer: "tools/interop-results/record.sh --vendors hdds",
        reachable_without_licence: true,
        has_version_2: true,
        // Measured, not configured: `hddsgen gen rust` emits
        // `encode_xcdr1_le`, `encode_xcdr2_le` and their `_at` variants,
        // and no `_be` of any kind, so the big-endian half of this
        // column does not exist.
        writes_orders: &["le"],
        note: "The second pure-Rust DDS implementation. `hddsgen gen rust` generates the \
               type and its own `encode_*` methods; `wireprobe-hdds` compiles that output \
               unedited against the declaration shim in `tools/wireprobe/hdds-shim/`, because \
               the published `hdds` crate does not carry the trait surface the generated code \
               implements. Structural limits, each measured: no big-endian encoder, no \
               parameter-list encoder — one framing per encoding version, whatever the type's \
               extensibility says — and a front end that accepts all corpus cases it parses \
               while its serialiser frames `@appendable` and `@mutable` types as `@final`, \
               with no DHEADER and no EMHEADER. `Dialect::Hdds` carries those departures, \
               opt-in, and reproduces every recorded row.",
        twins_are_one_type: false,
        // Measured from the five-vendor run: the LE-only writer answered
        // 320 cells with octets. The entry was written before that run
        // finished and said 0, which `committed_tree.rs` rightly took
        // for a column that had stopped producing.
        bytes_floor: 320,
    },
    System {
        id: "opensplice",
        label: "OpenSplice",
        kind: Kind::Dds,
        version: "OpenSplice Community Edition 6.9 (OSPL_V6_9_210323OSS_RELEASE)",
        producer: "tools/interop-results/record.sh --vendors opensplice",
        reachable_without_licence: true,
        // Measured, not configured: OpenSplice 6.9 has no XTypes and
        // `CdrTypeSupport` writes one representation, XCDR1 in the
        // host's byte order. The host is little endian.
        has_version_2: false,
        writes_orders: &["le"],
        note: "`idlpp -S -l cpp` (standalone C++, the CORBA C++ mapping) plus \
               `DDS::OpenSplice::CdrTypeSupport::serialize`. Its IDL is OMG IDL 3.5: no \
               annotation parses, keys are `#pragma keylist`, so `wireprobe-opensplice` \
               rewrites a case only where every construct means the same XCDR1 bytes in that \
               IDL and refuses the rest by name — `@mutable`, `@optional`, `@external`, \
               `@must_understand`, `@value`, an enum `@bit_bound` below 32, `bitmask`, \
               `bitset`, `map`, and a union as the topic type. `idlpp` itself refuses \
               `wchar`, `wstring`, `fixed`, struct inheritance, empty structs, `octet` and \
               `int8`/`uint8` discriminators and struct-typed key members. Four cases are \
               accepted by `idlpp` and its own generated C++ does not compile: the three \
               recursive types (forward-declared structs are defined twice), a type named \
               `static`, and an `unsigned long long` case label above the signed range.",
        // Not `true`, although the front end drops the annotations the
        // same way omniORB's, TAO's, JacORB's and both ROS type
        // supports' do. `tools/interop-results/tests/reproducible.rs`
        // measured 12 twin pairs whose octets disagree only at the
        // positions `padding_positions.rs`'s layout criterion already
        // calls fill — `union-discriminator-boolean`'s discriminator
        // is followed by `ad3fdc` in one recording and `bd38b3` in the
        // other, both between the same two members. Every disagreement
        // reads back as its own sample, so none is a value; the
        // recording's alignment gaps are simply not a function of the
        // sample here, which `crate::reproducible`'s allowance forgives
        // for the octets and this field would otherwise claim is not
        // possible at all. Unlike the write buffer
        // `omniorb-template.cpp` and `tao-template.cpp` fixed with a
        // fixed fill marker, this probe hands its sample straight to
        // `DDS::OpenSplice::CdrTypeSupport::serialize`, which owns and
        // allocates its own output buffer — there is no buffer here to
        // pre-mark before the call. `true` once a fix on that side (or
        // a re-record that measures the gap gone) is confirmed.
        twins_are_one_type: false,
        bytes_floor: 191,
    },
    System {
        id: "omniorb",
        label: "omniORB",
        kind: Kind::Orb,
        version: "omniORB 4.3.3",
        producer: "tools/interop-results/record.sh --orbs",
        reachable_without_licence: false,
        has_version_2: false,
        writes_orders: &["le", "be"],
        note: "`omniidl -bcxx` plus `cdrMemoryStream`. The only one of the three that writes \
               both byte orders, which is where the factor of two between this column and the \
               other two comes from and not from anything it accepts that they do not. It \
               refuses ten cases: the eight `union-discriminator-{octet,int8,uint8,wchar}` \
               (J9), where it answers `Syntax error in union declaration` and then \
               `Cannot create an instance of union 'Choice' inside its own definition`, which \
               is a remark about its own recovery and not about the case; and the two the fill \
               refuses before any ORB sees them.",
        twins_are_one_type: true,
        bytes_floor: 416,
    },
    System {
        id: "tao",
        label: "TAO",
        kind: Kind::Orb,
        version: "TAO 2.5.24",
        producer: "tools/interop-results/record.sh --orbs",
        reachable_without_licence: false,
        has_version_2: false,
        writes_orders: &["le"],
        note: "`tao_idl -as` plus `TAO_OutputCDR`. Writes little endian only: \
               `ACE_OutputCDR::write_4` swaps only when libACE was built with \
               `ACE_ENABLE_SWAP_ON_WRITE`, and this build was not. It refuses omniORB's ten \
               and ten more, all `fixed<P,S>` — and it refuses them **at exit status 0**: \
               `Error - tao_idl: \"gen.idl\", line L: fixed data types are not supported`, \
               294 lines over the ten cases, on a run that reports success. The probe catches \
               it because `genC.cpp` is not there afterwards, which is the only reason this \
               column is not ten cases of empty bytes. A severity in this front end's output \
               is not a claim its exit status backs.",
        twins_are_one_type: true,
        bytes_floor: 198,
    },
    System {
        id: "jacorb",
        label: "JacORB",
        kind: Kind::Orb,
        version: "JacORB 3.9",
        producer: "tools/interop-results/record.sh --orbs",
        reachable_without_licence: false,
        has_version_2: false,
        writes_orders: &["be"],
        note: "`org.jacorb.idl.parser` plus `CDROutputStream`. Writes big endian only: \
               `org.jacorb.orb.CDROutputStream` has no `setLittleEndian` — only \
               `CDRInputStream` does. It refuses 24 cases, which is the most of the three, and \
               14 of them are two defects of its own rather than J9's discriminator question. \
               **A union case label outside the signed range of its Java holder**: \
               `Illegal case label <3735928559> for integral switch type int` for an \
               `unsigned long` discriminator, the same for `unsigned long long`, `long long` \
               treated as 32-bit, and `unsigned short` reaching `javac` as \
               `Choice.java:33: error: incompatible types: possible lossy conversion from int \
               to short` in JacORB's **own** generated code — eight cases. **A `default:` \
               branch it can only give discriminator 0**: the generated \
               `fraction(int _discriminator, double _x)` throws `BAD_OPERATION` for any other, \
               so a sample selecting the default with an unlisted label has no JacORB form — \
               six cases, `array-1d-union`, `sequence-union` and `union-with-default`.",
        twins_are_one_type: true,
        bytes_floor: 194,
    },
    System {
        id: "ros-fastrtps",
        label: "ros-frtps",
        kind: Kind::Ros,
        version: "rosidl_typesupport_fastrtps_c 2.2.2 (ROS 2 Humble)",
        producer: "tools/interop-results/record.sh --ros (fastrtps)",
        reachable_without_licence: false,
        has_version_2: false,
        writes_orders: &["le", "be"],
        note: "Not Fast DDS: it generates its own serialisation rather than calling \
               `fastddsgen`'s output. Extensibility, `@key` and `@optional` are parsed and \
               dropped — every ROS message is a `@final` XCDR1 struct whatever its `.idl` says.",
        twins_are_one_type: true,
        bytes_floor: 224,
    },
    System {
        id: "ros-cyclone",
        label: "ros-cyc",
        kind: Kind::Ros,
        version: "rmw_cyclonedds_cpp 1.3.4 (ROS 2 Humble)",
        producer: "tools/interop-results/record.sh --ros (cyclone)",
        reachable_without_licence: false,
        has_version_2: false,
        writes_orders: &["le", "be"],
        note: "Not Cyclone: it walks the introspection type support rather than using \
               Cyclone's `idlc`. It writes the **host's** byte order and takes no argument \
               for it, so on a little-endian host the big-endian half of its column does not \
               exist.",
        twins_are_one_type: true,
        bytes_floor: 121,
    },
    System {
        id: "zerodds-rust",
        label: "ZD-Rust",
        kind: Kind::Own,
        version: "this tree",
        producer: "tools/interop-results/regenerate.sh (in process)",
        reachable_without_licence: true,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "`zerodds_wire_probe::encode`, under `Dialect::Specification` — the \
               clause-by-clause reading of XTypes 1.3, which is the default this project \
               ships. It was `Dialect::Cyclone`; `docs/specs/zerodds-interop-suite-1.0.md` \
               §7 carries the reversal.",
        twins_are_one_type: false,
        bytes_floor: 3330,
    },
    System {
        id: "zerodds-c",
        label: "ZD-C",
        kind: Kind::Own,
        version: "this tree, runtime/c",
        producer: "tools/wireprobe/three-column-gate --full",
        reachable_without_licence: true,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "An independent implementation of the same plan, measured by \
               `wireprobe-c-matrix` in the three-column gate's `full` tier.",
        twins_are_one_type: false,
        bytes_floor: 3330,
    },
    System {
        id: "zerodds-cpp",
        label: "ZD-C++",
        kind: Kind::Own,
        version: "this tree, runtime/cpp",
        producer: "tools/interop-results/record.sh --own cpp",
        reachable_without_licence: true,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "The fifth independent implementation, and the write \
               direction only: `CodecTarget::default_into` refuses for \
               C++, so no reader is generated. \
               `tools/wireprobe/wireprobe-cpp-corpus` holds it against \
               the Rust column.",
        twins_are_one_type: false,
        bytes_floor: 3330,
    },
    System {
        id: "zerodds-csharp",
        label: "ZD-C#",
        kind: Kind::Own,
        version: "this tree, runtime/csharp",
        producer: "tools/interop-results/record.sh --own csharp",
        reachable_without_licence: true,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "The sixth independent implementation, and the only own \
               column besides Rust, C and Python that generates a reader \
               as well as a writer: `default_csharp.rs` supplies Table 9 \
               per type. What this column carries here is the **write** \
               grid, which is what `csharp-record.sh` produces alongside \
               the octets. The read direction is measured and is not in \
               the ledger: \
               `tools/wireprobe/wireprobe-csharp-corpus --read` reads \
               every case's recorded Rust octets back and compares the \
               sample — 372 same, 0 different, 2 refused, 2 unwritten — \
               under `plain-cdr2 le` alone, because a read grid over all \
               ten slots is another 3 780 runs and no run of it is \
               recorded.",
        twins_are_one_type: false,
        // The measured count, not a copy of a sibling's: 3 740 of the
        // 3 760 slots carry octets and the twenty that do not are two
        // cases the wire has no form for. Held against the C, C++ and
        // Java sources row for row over every shared slot — 3 740, 3 450
        // and 3 450 — with none different, which is what verifies the
        // nine configurations the corpus sweep does not compare.
        bytes_floor: 3740,
    },
    System {
        id: "zerodds-java",
        label: "ZD-Java",
        kind: Kind::Own,
        version: "this tree, runtime/java",
        producer: "tools/interop-results/record.sh --own java",
        reachable_without_licence: true,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "The fourth independent implementation, and the write \
               direction only: `CodecTarget::default_into` refuses for \
               Java, so no reader is generated. \
               `tools/wireprobe/wireprobe-java-corpus` holds it against \
               the Rust column.",
        twins_are_one_type: false,
        bytes_floor: 3330,
    },
    System {
        id: "zerodds-python",
        label: "ZD-Py",
        kind: Kind::Own,
        version: "this tree, Python 3.14 stdlib only",
        producer: "tools/wireprobe/three-column-gate --full",
        reachable_without_licence: true,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "The third independent implementation, measured by \
               `tools/wireprobe/corpus-python.py --matrix` in the same run.",
        twins_are_one_type: false,
        bytes_floor: 3330,
    },
    System {
        id: "zerodds-typescript",
        label: "ZD-TS",
        kind: Kind::Own,
        version: "this tree, runtime/typescript",
        producer: "tools/interop-results/record.sh --own typescript",
        reachable_without_licence: true,
        has_version_2: true,
        writes_orders: &["le", "be"],
        note: "The seventh independent implementation, and the **write** \
               grid only. A reader is generated — `default_typescript.rs` \
               supplies Table 9 per type — and is measured by \
               `runtime/typescript/run-tests.sh` against each case's own \
               sample; what is not built is a probe that reads a payload \
               on standard input, so no read row is recorded rather than \
               one being guessed at. \
               `tools/wireprobe/wireprobe-typescript-corpus` holds the \
               write direction against the Rust column under \
               `plain-cdr2 le`, and this grid is the other nine slots.",
        twins_are_one_type: false,
        // The measured count: 373 of the 376 cases carry octets in all
        // ten slots. The three that do not are two samples the Rust
        // writer refuses as well — a `char` above U+00FF and a `wchar`
        // above the BMP — and one name TypeScript has no form for,
        // `docs/language-limits.md`'s captured qualified reference.
        bytes_floor: 3730,
    },
];

/// One system by id.
#[must_use]
pub fn system(id: &str) -> Option<&'static System> {
    SYSTEMS.iter().find(|system| system.id == id)
}
