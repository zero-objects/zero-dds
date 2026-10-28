// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The call sequence one sample becomes, before any language sees it.
//!
//! Every entry is one exported operation of the binding under test. The
//! list carries values and framing boundaries and nothing else: no
//! offset, no padding, no length word, no byte order. Those are what
//! the binding decides, and a generator that decided any of them would
//! be measuring itself.

/// Which spelling of an integer argument the generated program hands
/// the binding.
///
/// Every one of the fifteen bindings names its integer writers for an
/// *unsigned* wire width — `put_u8` through `put_u64` — and none of them
/// takes an IDL type. So the conversion from a signed IDL value to the
/// octets is the caller's in all fifteen, and there are two things a
/// caller can hand over:
///
/// * [`Form::Bits`] — the two's complement pattern in the write's own
///   width, which is the same argument in every binding and keeps a
///   corpus sweep on the wire question.
/// * [`Form::Natural`] — the IDL value itself, which is what a caller of
///   that binding actually writes: `-12345` for a `short` and
///   `16045690984833335998` for an `unsigned long long`.
///
/// Running the corpus in both and subtracting is the whole of
/// `tools/wireprobe/endpoint-number-surface`. A binding that answers the
/// same way in both takes the value a caller has; one that does not
/// makes the caller carry the conversion, and whether it says so is the
/// measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// The two's complement pattern of the write's width. The default,
    /// and the spelling every renderer emitted before `--numbers`
    /// existed, so a sweep that does not ask for the other one produces
    /// the same source text and therefore the same octets.
    Bits,
    /// The IDL value.
    Natural,
}

impl Form {
    /// Parse the `--numbers` value.
    ///
    /// # Errors
    /// The name, when it is neither.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "bits" => Ok(Self::Bits),
            "natural" => Ok(Self::Natural),
            other => Err(format!("--numbers: `{other}` is not bits or natural")),
        }
    }
}

/// One integer argument, in both spellings.
///
/// Both are carried rather than one derived from the other, because
/// neither derivation is available where a renderer stands: the pattern
/// alone has lost the sign the IDL type gave it, and the value alone
/// does not say which width's complement it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Int {
    /// The octets, as an unsigned integer of the write's width.
    pub bits: u64,
    /// The IDL value the sample carried.
    pub natural: i128,
}

impl Int {
    /// One argument, from the pattern and the value it came from.
    #[must_use]
    pub const fn new(bits: u64, natural: i128) -> Self {
        Self { bits, natural }
    }

    /// The argument as a decimal, in the form asked for.
    ///
    /// A decimal in both, because that is what every renderer already
    /// emitted for [`Form::Bits`]; a renderer that needs another
    /// spelling — Kotlin's signed `Long`, Julia's `Float32` suffix —
    /// reads the fields instead.
    #[must_use]
    pub fn text(self, form: Form) -> String {
        match form {
            Form::Bits => self.bits.to_string(),
            Form::Natural => self.natural.to_string(),
        }
    }

    /// The pattern as the octet it is.
    #[must_use]
    pub const fn octet(self) -> u8 {
        self.bits as u8
    }
}

/// A `float` argument, in the two spellings the renderers can spell.
///
/// Two, and not one, because a renderer may either hand the binding the
/// sample's own decimal and let the target language narrow it, or hand
/// it the `binary32` this generator already narrowed and write the
/// shortest decimal that reads back as that.
///
/// * [`Real::text`] is the decimal exactly as the corpus wrote it. All
///   fifteen hand-written endpoint renderers pass this to `put_f32`, so
///   the narrowing is the target language's. That is the point where
///   Lua, OCaml, Elixir, Node and Python — none of which has a 32-bit
///   float — can lose a value without saying so.
/// * [`Real::value`] is the sample's number already narrowed to the
///   `binary32` an IDL `float` holds. Ten of the fifteen wrote the
///   shortest decimal of this before they were brought onto the corpus
///   decimal; the `java` renderer, which is not one of the fifteen,
///   still writes this.
#[derive(Debug, Clone, PartialEq)]
pub struct Real {
    /// The sample's number, narrowed to `binary32`.
    pub value: f32,
    /// The decimal the sample carried, unparsed.
    pub text: String,
}

/// The length code a `@mutable` member's header carries. XTypes 1.3
/// §7.4.3.4.2 Table 34.
///
/// Carried through to the renderer rather than resolved here, because
/// what a code means is the binding's answer: LC4 reserves a NEXTINT of
/// its own, LC0..LC3 put the length in the code, and LC5..LC7 reuse the
/// member's own leading word and must **not** reserve a second one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lc(pub u8);

impl Lc {
    /// The name the generated source spells it with, in the numbering
    /// every one of the fifteen bindings exports.
    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }
}

/// One call into the binding's writer.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// One octet. Carries `octet`, `uint8`, `int8` and the eight-bit
    /// holders — XTypes 1.3 §7.4.1.1.1 Table 30 gives them all the same
    /// one-octet form.
    U8(Int),
    /// Two octets.
    U16(Int),
    /// Four octets.
    U32(Int),
    /// Eight octets.
    U64(Int),
    /// A `boolean`. One octet, 0 or 1 — kept apart from [`Op::U8`]
    /// because every binding now exports a writer of its own for it and
    /// calling that one is what puts it under test.
    Bool(bool),
    /// An IDL `char`: the one octet Table 30 gives it.
    ///
    /// Apart from [`Op::U8`] because the operation is not the same one.
    /// `put_char` refuses a code point above U+00FF and `put_u8` has no
    /// opinion about what its octet means; the two write the same octet
    /// for every value either accepts, and only one of them is under
    /// test when a `char` is what the model declared.
    Char(Int),
    /// An IDL `wchar`: one two-octet code unit.
    Wchar(Int),
    /// An IDL `wstring`, as its UTF-16 code units.
    ///
    /// The units and not a string: the binding writes the length word —
    /// which counts octets and not units — and the units themselves, and
    /// the conversion from text to units is the sample's, exactly as
    /// [`Op::Str`] hands over octets and not a string.
    Wstr(Vec<u16>),
    /// Binary32, in the two spellings the fifteen renderers split on.
    F32(Real),
    /// Binary64.
    F64(f64),
    /// A `bitset`, as the anchor word its fields pack into and the
    /// width of that anchor. XTypes 1.3 §7.3.1.4.
    ///
    /// The packing is the type's — field *k* at the running sum of the
    /// widths before it, padding fields included — and is resolved by
    /// the walk from the model, the same way an `enum`'s holder value
    /// and a `bitmask`'s bit positions already are. What the binding
    /// answers is the pair the type does not: which write the anchor
    /// goes out through and what that write aligns to, which is the one
    /// thing that differs between the two encodings.
    Bitset {
        /// The packed holder.
        bits: u64,
        /// Its width in octets: one, two, four or eight.
        anchor: u8,
    },
    /// A `fixed<P,S>`, as the decimal the sample carried.
    ///
    /// Unparsed. The padding to `digits`, the nibble order, the sign
    /// nibble and the leading zero are all §9.3.2.8's and therefore the
    /// binding's; handing over a digit string would be this generator
    /// answering them.
    Fixed {
        /// The decimal, as the sample wrote it.
        value: String,
        /// `P`.
        digits: u8,
        /// `S`.
        scale: u8,
    },
    /// A narrow string, as its octets without the terminator: the
    /// binding writes the length word, the octets and the NUL.
    Str(Vec<u8>),
    /// A collection of one-octet elements, as its octets: the binding
    /// writes the element count and the octets.
    SeqU8(Vec<u8>),
    /// The count of a collection whose elements follow one by one.
    ///
    /// A separate variant from [`Op::U32`] so a renderer and a reader
    /// of the generated source can tell a length word from a value; the
    /// call it becomes is the same `put_u32`.
    ///
    /// It carries an [`Int`] like the value writes do, and both of its
    /// spellings are the same number: a length is a count of elements
    /// and never a two's complement of anything, so [`Form::Natural`]
    /// changes nothing here. That is the statement, not an oversight —
    /// a sweep in the other form must move the *values* and leave every
    /// length word where it was.
    Count(Int),
    /// Open a DHEADER. XTypes 1.3 §7.4.3.5.
    DHeaderBegin,
    /// Close the innermost open DHEADER.
    DHeaderEnd,
    /// Open a PL_CDR2 member header. XTypes 1.3 §7.4.3.4.2.
    EmHeaderBegin {
        /// The member id.
        id: u32,
        /// Whether the reader must fail rather than skip.
        must_understand: bool,
        /// Which of the eight length codes frames the member.
        lc: Lc,
    },
    /// Close the innermost open member header.
    ///
    /// It carries the length code back, because the call it becomes
    /// does: a binding that was handed one code to open with and
    /// another to close with has to refuse, and it cannot if the close
    /// does not say.
    EmHeaderEnd {
        /// The code the matching [`Op::EmHeaderBegin`] opened with.
        lc: Lc,
    },
    /// Open a PL_CDR1 parameter header, short form. XTypes 1.3
    /// §7.4.1.2.
    PidBegin {
        /// The parameter id.
        pid: u16,
        /// Whether bit 14 of the id is set.
        must_understand: bool,
    },
    /// Open a PL_CDR1 parameter header, twelve-octet form.
    PidExtendedBegin {
        /// The member id, which is 28 bits here and not fourteen.
        member_id: u32,
        /// Whether bit 30 of the eMemberHeader is set. Not the marker's
        /// bit 14, which this framing sets either way.
        must_understand: bool,
    },
    /// Close the innermost open parameter header.
    PidEnd,
    /// The PID_LIST_END sentinel that closes a parameter list.
    PidListEnd,
}

/// Which XCDR version the run is in.
///
/// It reaches the renderer because it reaches the binding's
/// constructor: XTypes 1.3 §7.4.1 puts MAXALIGN at eight under XCDR1
/// and four under XCDR2, and a writer that did not take it would be
/// writing one of the two and saying nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// PLAIN_CDR and PL_CDR.
    Xcdr1,
    /// PLAIN_CDR2, DELIMITED_CDR and PL_CDR2.
    Xcdr2,
}

/// One case, one encoding, one byte order, as calls.
#[derive(Debug, Clone)]
pub struct Program {
    /// The calls, in order.
    pub ops: Vec<Op>,
    /// Which XCDR version the writer is constructed for.
    pub version: Version,
}

/// Why a case has no form in one binding's write surface.
///
/// Not an error: it is the extent of the binding, and the sweep counts
/// it apart from a divergence. The text names the construct and the
/// operation that is missing, so a reader never has to guess which of
/// the two it is looking at.
#[derive(Debug, Clone)]
pub struct OutOfSurface(pub String);

impl core::fmt::Display for OutOfSurface {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}
