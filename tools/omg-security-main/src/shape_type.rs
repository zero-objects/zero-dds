// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The suite's `ShapeType`, generated at build time from
//! `interop/shapes/omg-dds-security.idl`.
//!
//! It is generated, not written by hand, and it was generated from the
//! first commit. `tools/omg-shape-main` records what the hand form
//! costs: a suite run has to measure what this project generates, and
//! with a hand-written type a defect in the generator cannot fail the
//! suite while a defect in the hand form is charged to us. Both were
//! real there — a `type_object()` left at its default `None` (a topic
//! type name announced with no type behind it, no
//! `PID_TYPE_INFORMATION` in SEDP, XTypes 1.3 §7.6.3.2) and an
//! `encode_be` that was never overridden, so a big-endian writer
//! stamped `D_CDR2_BE` onto a little-endian body.
//!
//! The three tests at the bottom are the guards: two of them are the
//! `omg-shape-main` pair rebuilt for this type, the third measures what
//! the front end does with the suite's `//@key` comment.

// `dead_code` is this binary's: the generated text is a type's whole
// surface — `key_hash`, the IDL's constants — and a binary that uses
// part of it leaves the rest unused. The clippy lints the generator used
// to trip here are gone from its output.
#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/omg-dds-security.rs"));
}

pub use generated::ShapeType;

impl ShapeType {
    /// One shape sample. Four members, which is the whole type — the
    /// `dds-security` form has no `additional_payload_size`.
    #[must_use]
    pub fn new(color: impl Into<String>, x: i32, y: i32, shapesize: i32) -> Self {
        Self {
            color: color.into(),
            x,
            y,
            shapesize,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::ShapeType;
    use zerodds_dcps::DdsType;

    /// The type this client measures with announces itself.
    ///
    /// The regression guard for the reason the type is generated at
    /// all. `DdsType::type_object()` defaults to `None`, and the
    /// hand-written `ShapeType` of the *other* suite
    /// (`crates/dcps/src/interop.rs`) takes that default: an endpoint
    /// built on it registers no `TypeObject` with the participant's
    /// TypeLookup server and its SEDP announcement carries no
    /// `PID_TYPE_INFORMATION` (XTypes 1.3 §7.6.3.2) — a topic type name
    /// with no type behind it. Anyone who points this client at a
    /// hand-written type fails here.
    #[test]
    fn the_measured_type_announces_a_type_object() {
        assert!(
            <ShapeType as DdsType>::type_object().is_some(),
            "the type this client measures with announces no TypeObject; \
             it is a hand-written one, not the generated one"
        );
    }

    /// Big-endian is its own encoding, not little-endian with a
    /// different stamp.
    ///
    /// The other half of the same regression: a type that does not
    /// override `encode_be` lets the `DdsType` default delegate to the
    /// little-endian body while the writer stamps `D_CDR2_BE` (RTPS 2.5
    /// §10.5 — the representation identifier covers the whole payload).
    /// A peer then reads the DHEADER of a 20-octet sample as a number
    /// in the hundreds of millions, and both sides stay quiet about it.
    #[test]
    fn the_two_byte_orders_do_not_produce_the_same_octets() {
        let sample = ShapeType::new("BLACK", 42, 77, 30);
        let mut le = Vec::new();
        DdsType::encode(&sample, &mut le).expect("little-endian encode");
        let mut be = Vec::new();
        DdsType::encode_be(&sample, &mut be).expect("big-endian encode");
        assert_ne!(
            le, be,
            "encode_be returned the little-endian octets; the big-endian \
             body would go out under a D_CDR2_BE stamp"
        );
        assert_eq!(le.len(), be.len(), "the two orders write the same length");
    }

    /// The suite writes its key as `//@key`, and the two halves of
    /// "keyed" have to agree whatever the front end makes of that line.
    ///
    /// `interop/shapes/omg-dds-security.idl` is the suite's file
    /// verbatim: `string<COLOR_MAX_LENGTH> color; //@key` — the
    /// comment-suffix form that XTypes 1.3 Table 24 declares equivalent
    /// to the `@key` annotation ("alternately and equivalently … by
    /// suffixing it with an annotation type name using slash-slash-at").
    /// Measured on the tree this crate was written against, the form
    /// passes `parse` and `lower` without a diagnostic and `color` comes
    /// out with `facets.is_key() == false`; the front-end work to read
    /// it is under way. `README.md` records what the keyless state costs
    /// on the wire.
    ///
    /// So this test does not pin the defect — pinning it would turn the
    /// fix into a failure. It pins the invariant that holds on both
    /// sides of the fix: `HAS_KEY` and `key_hash()` are one decision,
    /// and where the type is keyed it is `color` that decides the hash
    /// and not the position. A half-landed fix that flips `HAS_KEY`
    /// without a key holder behind it — the shape that merges every
    /// instance into one and says nothing — fails here.
    ///
    /// The front end has since learned the `//@key` suffix, so `HAS_KEY`
    /// is now `true` and the generated type carries an **inherent**
    /// `key_hash() -> Result<[u8; 16], WireError>` that shadows the
    /// trait's `Option`-returning one. Every call below therefore names
    /// the trait method: `HAS_KEY` and `DdsType::key_hash` are the pair
    /// this test is about, and the inherent method would answer a
    /// different question (whether the holder encoded) with a different
    /// type.
    #[test]
    fn the_key_decision_is_one_decision_and_it_is_the_colour() {
        let sample = ShapeType::new("BLACK", 42, 77, 30);
        assert_eq!(
            <ShapeType as DdsType>::HAS_KEY,
            DdsType::key_hash(&sample).is_some(),
            "HAS_KEY and key_hash() disagree: one of them says this type has \
             instances and the other says it does not"
        );

        if !<ShapeType as DdsType>::HAS_KEY {
            // The measured pre-fix state. A keyless type has one
            // instance and no KeyHash, so there is nothing further to
            // hold against anything.
            return;
        }

        let same_colour_elsewhere = ShapeType::new("BLACK", 1, 2, 3);
        let other_colour = ShapeType::new("RED", 42, 77, 30);
        assert_eq!(
            DdsType::key_hash(&sample),
            DdsType::key_hash(&same_colour_elsewhere),
            "x, y and shapesize are not key members (XTypes 1.3 §7.6.8): \
             moving a shape must not create a second instance"
        );
        assert_ne!(
            DdsType::key_hash(&sample),
            DdsType::key_hash(&other_colour),
            "two colours are two instances; one KeyHash for both merges them \
             and the merge is silent"
        );
    }
}
