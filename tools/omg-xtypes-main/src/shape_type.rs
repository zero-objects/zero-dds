// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The suite's 25 shape types, generated at build time from
//! `interop/shapes/omg-dds-xtypes.idl`, and the runtime dispatch over
//! them.
//!
//! ## Why the types are generated and not written here
//!
//! `tools/omg-shape-main` took its single `ShapeType` from a
//! hand-written Rust struct until commit `98b4800bdc3`. That is the
//! wrong side of the measurement: a defect in the generator then cannot
//! fail the suite, and a defect in the hand form is charged to us. Both
//! were real there — the hand-written type left
//! `DdsType::type_object()` at its default `None` (a topic type name
//! announced with no type behind it, no `PID_TYPE_INFORMATION` in SEDP,
//! XTypes 1.3 §7.6.3.2) and did not override `encode_be`.
//!
//! For `dds-xtypes` the argument is not an analogy. This suite measures
//! nothing but the type: its 625 publisher/subscriber pairs ask what a
//! `TypeObject` says about an aggregate and whether two of them are
//! assignable (XTypes 1.3 §7.2.4.4.4.4). A hand-written type would put
//! a hand-written answer on both sides of that question, and the
//! measurement would be of nothing.
//!
//! ## The dispatch
//!
//! The suite chooses the type per run (`-type Shape2Mutable`) and pairs
//! two runs of different types on one topic, so one process has to be
//! able to be any of the 25. `srcCxx/ShapeType_variants.h` solves that
//! with `ShapeTypeVariants::create_writer`/`create_reader`: a
//! `WriterBase`/`ReaderBase` abstract base, a template `Writer<T,…>` /
//! `Reader<T,…>` per type, and a 25-branch `strcmp` chain that picks
//! one. [`create_writer`] and [`create_reader`] below are that same
//! shape — a trait object over a generic implementation, and one
//! `match` with 25 arms.
//!
//! The 25 arms are written once, by `variant_dispatch`, from a single
//! list of type names. That list is the only place a variant is named:
//! [`VARIANT_NAMES`], [`is_variant`], [`print_variants`],
//! [`create_writer`] and [`create_reader`] all come out of it, so a name
//! cannot be in one and missing from another.

// The suite's data lines go to stdout, as they do in
// `ShapeType_variants.h`; that is this module's output and not a stray
// debug print.
#![allow(clippy::print_stdout)]

// `missing_docs` is this crate being a library: the generator documents
// what it considers the surface and not the codec's inherent functions,
// and the workspace warns on every undocumented public item.
#[allow(missing_docs)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/omg-dds-xtypes.rs"));
}

use std::io::Write as _;
use std::sync::Arc;

use zerodds_dcps::psm_constants::status as status_bits;
use zerodds_dcps::qos::{DataReaderQos, DataWriterQos, PublisherQos, SubscriberQos};
use zerodds_dcps::{
    DataReader, DataWriter, DdsType, DomainParticipant, Publisher, Subscriber, Topic, TopicQos,
};

use crate::listeners::{ShapeReaderListener, ShapeTopicListener};

/// One of the 25 types the suite drives, seen through the two things
/// the suite asks of it.
///
/// `fill_data` and `print_data` are the two operations
/// `srcCxx/ShapeType_variants.h` needs per type — `ShapeNFiller::fill_data`
/// and the vendor's `TSupport::print_data`. Everything else the client
/// does with a sample goes through `DdsType`, which the generator wrote.
pub trait ShapeVariant: DdsType + Clone + Send + Sync + 'static {
    /// The suite's own sample values, `ShapeType_variants.h`
    /// `ShapeNFiller::fill_data`: `x = count % 250`,
    /// `y = 2*count % 250`, `shapesize = 30`, and per family
    /// `z = 3*count % 250` resp. `angle = (5*count) % 360`.
    fn fill_data(color: &str, count: i32) -> Self;

    /// The vendor `print_data` layout the suite's README shows: one
    /// member per line, three leading spaces, a leading newline before
    /// each line and a closing newline after the last.
    fn print_data(&self, out: &mut String);
}

/// `   color: "BLUE"` — a string member, quoted, as `print_data` writes it.
fn member_string(out: &mut String, name: &str, value: &str) {
    out.push_str("\n   ");
    out.push_str(name);
    out.push_str(": \"");
    out.push_str(value);
    out.push('"');
}

/// `   x: 42` — a `long` member.
fn member_long(out: &mut String, name: &str, value: i32) {
    out.push_str("\n   ");
    out.push_str(name);
    out.push_str(": ");
    out.push_str(&value.to_string());
}

/// `   angle: 0.000000` — a `float` member, C `%f`, which is six
/// fractional digits.
fn member_float(out: &mut String, name: &str, value: f32) {
    out.push_str("\n   ");
    out.push_str(name);
    out.push_str(&format!(": {value:.6}"));
}

/// `Shape1…`: `color, x, y, shapesize`.
macro_rules! shape1_variants {
    ($($t:ident),* $(,)?) => { $(
        impl ShapeVariant for generated::$t {
            fn fill_data(color: &str, count: i32) -> Self {
                Self {
                    color: color.to_owned(),
                    x: count % 250,
                    y: (2 * count) % 250,
                    shapesize: 30,
                }
            }
            fn print_data(&self, out: &mut String) {
                member_string(out, "color", &self.color);
                member_long(out, "x", self.x);
                member_long(out, "y", self.y);
                member_long(out, "shapesize", self.shapesize);
                out.push('\n');
            }
        }
    )* };
}

/// `Shape2…`: `color, x, y, shapesize, angle`.
macro_rules! shape2_variants {
    ($($t:ident),* $(,)?) => { $(
        impl ShapeVariant for generated::$t {
            fn fill_data(color: &str, count: i32) -> Self {
                Self {
                    color: color.to_owned(),
                    x: count % 250,
                    y: (2 * count) % 250,
                    shapesize: 30,
                    #[allow(clippy::cast_precision_loss)]
                    angle: ((5 * count) % 360) as f32,
                }
            }
            fn print_data(&self, out: &mut String) {
                member_string(out, "color", &self.color);
                member_long(out, "x", self.x);
                member_long(out, "y", self.y);
                member_long(out, "shapesize", self.shapesize);
                member_float(out, "angle", self.angle);
                out.push('\n');
            }
        }
    )* };
}

/// `Shape5…`: `color, x, y, shapesize, angle`, by inheritance from Shape1. The base is the first
/// member `parent` of the generated struct (XTypes 1.3 §7.5.1.4.1.3), so the four members of
/// `Shape1…` are filled and printed through it; `ShapeType_variants.h` gives both families the
/// same filler values, and the bytes are those of the flat `Shape2…`.
macro_rules! shape5_variants {
    ($(($t:ident, $base:ident)),* $(,)?) => { $(
        impl ShapeVariant for generated::$t {
            fn fill_data(color: &str, count: i32) -> Self {
                Self {
                    parent: <generated::$base as ShapeVariant>::fill_data(color, count),
                    #[allow(clippy::cast_precision_loss)]
                    angle: ((5 * count) % 360) as f32,
                }
            }
            fn print_data(&self, out: &mut String) {
                member_string(out, "color", &self.parent.color);
                member_long(out, "x", self.parent.x);
                member_long(out, "y", self.parent.y);
                member_long(out, "shapesize", self.parent.shapesize);
                member_float(out, "angle", self.angle);
                out.push('\n');
            }
        }
    )* };
}

/// `Shape3…`: `color, x, y, z, shapesize` — the inserted member sits in
/// the middle, which is the whole reason this family exists.
macro_rules! shape3_variants {
    ($($t:ident),* $(,)?) => { $(
        impl ShapeVariant for generated::$t {
            fn fill_data(color: &str, count: i32) -> Self {
                Self {
                    color: color.to_owned(),
                    x: count % 250,
                    y: (2 * count) % 250,
                    z: (3 * count) % 250,
                    shapesize: 30,
                }
            }
            fn print_data(&self, out: &mut String) {
                member_string(out, "color", &self.color);
                member_long(out, "x", self.x);
                member_long(out, "y", self.y);
                member_long(out, "z", self.z);
                member_long(out, "shapesize", self.shapesize);
                out.push('\n');
            }
        }
    )* };
}

/// `Shape4…`: `color, shapesize, x, y` — Shape1's members, reordered.
macro_rules! shape4_variants {
    ($($t:ident),* $(,)?) => { $(
        impl ShapeVariant for generated::$t {
            fn fill_data(color: &str, count: i32) -> Self {
                Self {
                    color: color.to_owned(),
                    shapesize: 30,
                    x: count % 250,
                    y: (2 * count) % 250,
                }
            }
            fn print_data(&self, out: &mut String) {
                member_string(out, "color", &self.color);
                member_long(out, "shapesize", self.shapesize);
                member_long(out, "x", self.x);
                member_long(out, "y", self.y);
                out.push('\n');
            }
        }
    )* };
}

shape1_variants! {
    Shape1Default, Shape1Final, Shape1Extensible, Shape1Mutable, Shape1MutableExplicitID,
}
shape2_variants! {
    Shape2Default, Shape2Final, Shape2Extensible, Shape2Mutable, Shape2MutableExplicitID,
}
shape5_variants! {
    (Shape5Default, Shape1Default),
    (Shape5Final, Shape1Final),
    (Shape5Extensible, Shape1Extensible),
    (Shape5Mutable, Shape1Mutable),
    (Shape5MutableExplicitID, Shape1MutableExplicitID),
}
shape3_variants! {
    Shape3Default, Shape3Final, Shape3Extensible, Shape3Mutable, Shape3MutableExplicitID,
}
shape4_variants! {
    Shape4Default, Shape4Final, Shape4Extensible, Shape4Mutable, Shape4MutableExplicitID,
}

// ============================================================================
// The dispatch — `ShapeTypeVariants` of `srcCxx/ShapeType_variants.h`
// ============================================================================

/// The publish side of one chosen type.
///
/// `srcCxx/ShapeType_variants.h` `WriterBase`. The trait exists so the
/// 25 monomorphisations of `TypedWriter` can be held in one place; it
/// carries nothing a type parameter would have to be visible for.
pub trait WriterBase {
    /// Fills a sample per the suite's filler and writes it.
    ///
    /// # Errors
    /// Whatever `DataWriter::write` reports.
    fn write_data(&self, color: &str, count: i32) -> zerodds_dcps::Result<()>;
    /// The topic name, for the suite's own output lines.
    fn topic_name(&self) -> &str;
    /// The registered type name — `DdsType::TYPE_NAME` of the chosen type.
    fn type_name(&self) -> &'static str;
    /// Runs the listener callbacks that are due.
    fn drive_listeners(&self);
    /// Reads `InconsistentTopicStatus`, which is what fires the topic
    /// listener (DDS 1.4 §2.2.4.2.4).
    fn poll_inconsistent_topic(&self);
}

/// The subscribe side of one chosen type.
///
/// `srcCxx/ShapeType_variants.h` `ReaderBase`.
pub trait ReaderBase {
    /// Takes everything available and prints each valid sample the way
    /// the suite's reader does. Returns how many were printed.
    fn take_data(&self) -> usize;
    /// The topic name, for the suite's own output lines.
    fn topic_name(&self) -> &str;
    /// The registered type name — `DdsType::TYPE_NAME` of the chosen type.
    fn type_name(&self) -> &'static str;
    /// Runs the listener callbacks that are due.
    fn drive_listeners(&self);
    /// Reads `InconsistentTopicStatus`, which is what fires the topic
    /// listener (DDS 1.4 §2.2.4.2.4).
    fn poll_inconsistent_topic(&self);
}

/// One type's publish side. `Writer<T, TFiller, TSupport, TDataWriter>`
/// of `ShapeType_variants.h`, whose four parameters collapse to one here
/// because the generated `DdsType` carries the type support and the
/// filler is a trait method.
struct TypedWriter<T: ShapeVariant> {
    topic_name: String,
    topic: Topic<T>,
    writer: DataWriter<T>,
    // The publisher owns the writer's group; dropping it would take the
    // writer with it.
    _publisher: Publisher,
}

/// One type's subscribe side. `Reader<T, TSupport, TDataReader>` of
/// `ShapeType_variants.h`.
struct TypedReader<T: ShapeVariant> {
    topic_name: String,
    topic: Topic<T>,
    reader: DataReader<T>,
    _subscriber: Subscriber,
}

impl<T: ShapeVariant> WriterBase for TypedWriter<T> {
    fn write_data(&self, color: &str, count: i32) -> zerodds_dcps::Result<()> {
        let sample = T::fill_data(color, count);
        let mut line = format!(
            "Writing Topic \"{}\", type \"{}\", count {count}, data:",
            self.topic_name,
            T::TYPE_NAME
        );
        sample.print_data(&mut line);
        print!("{line}");
        // The suite is read by a human at a terminal and by whatever the
        // reviewer pipes it into; a pipe is block-buffered, so each record
        // is flushed as it is written rather than in bursts.
        let _ = std::io::stdout().flush();
        self.writer.write(&sample)
    }

    fn topic_name(&self) -> &str {
        &self.topic_name
    }

    fn type_name(&self) -> &'static str {
        T::TYPE_NAME
    }

    fn drive_listeners(&self) {
        self.writer.drive_listeners();
    }

    /// Reading the status is what dispatches `on_inconsistent_topic`; the
    /// runtime adds the conflicts discovery found to the `Topic`'s own.
    fn poll_inconsistent_topic(&self) {
        let _ = self.topic.inconsistent_topic_status();
    }
}

impl<T: ShapeVariant> ReaderBase for TypedReader<T> {
    fn take_data(&self) -> usize {
        let Ok(samples) = self.reader.take_with_info() else {
            return 0;
        };
        let mut printed = 0;
        for s in &samples {
            if !s.info.valid_data {
                continue;
            }
            let mut line = format!("\nRead data for Topic {}", self.topic_name);
            s.data.print_data(&mut line);
            print!("{line}");
            printed += 1;
        }
        if printed > 0 {
            let _ = std::io::stdout().flush();
        }
        printed
    }

    fn topic_name(&self) -> &str {
        &self.topic_name
    }

    fn type_name(&self) -> &'static str {
        T::TYPE_NAME
    }

    fn drive_listeners(&self) {
        self.reader.drive_listeners();
    }

    /// Reading the status is what dispatches `on_inconsistent_topic`; the
    /// runtime adds the conflicts discovery found to the `Topic`'s own.
    fn poll_inconsistent_topic(&self) {
        let _ = self.topic.inconsistent_topic_status();
    }
}

fn build_writer<T: ShapeVariant>(
    participant: &DomainParticipant,
    topic_name: &str,
) -> zerodds_dcps::Result<Box<dyn WriterBase>> {
    let publisher = participant.create_publisher(PublisherQos::default());
    let topic = participant.create_topic::<T>(topic_name, TopicQos::default())?;
    topic.set_listener(
        Some(Arc::new(ShapeTopicListener::new(
            topic_name.to_owned(),
            T::TYPE_NAME,
        ))),
        status_bits::ANY,
    );
    let writer = publisher.create_datawriter::<T>(&topic, DataWriterQos::default())?;
    Ok(Box::new(TypedWriter {
        topic_name: topic_name.to_owned(),
        topic,
        writer,
        _publisher: publisher,
    }))
}

fn build_reader<T: ShapeVariant>(
    participant: &DomainParticipant,
    topic_name: &str,
) -> zerodds_dcps::Result<Box<dyn ReaderBase>> {
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let topic = participant.create_topic::<T>(topic_name, TopicQos::default())?;
    topic.set_listener(
        Some(Arc::new(ShapeTopicListener::new(
            topic_name.to_owned(),
            T::TYPE_NAME,
        ))),
        status_bits::ANY,
    );
    let reader = subscriber.create_datareader::<T>(&topic, DataReaderQos::default())?;
    reader.set_listener(
        Some(Arc::new(ShapeReaderListener::new(
            topic_name.to_owned(),
            T::TYPE_NAME,
        ))),
        status_bits::ANY,
    );
    Ok(Box::new(TypedReader {
        topic_name: topic_name.to_owned(),
        topic,
        reader,
        _subscriber: subscriber,
    }))
}

/// Writes everything that is per-variant out of one list of the 25
/// names.
///
/// `ShapeType_variants.h` writes the same list five times over — twice
/// in `print_type_variants`/`check_type_variant` as a
/// `"Shape%d%s"`-formatted string, and twice more as a `strcmp` chain in
/// `create_reader`/`create_writer`. Nothing there holds the four copies
/// together. Here the four come from one list, so a variant that is in
/// the IDL and not in this list fails to compile in `VARIANT_NAMES`
/// (which is length-checked below) rather than going missing from one of
/// the four.
macro_rules! variant_dispatch {
    ($($t:ident),* $(,)?) => {
        /// The 25 type names the suite accepts for `-type`, in the order
        /// `ShapeTypeVariants::print_type_variants` prints them: version
        /// outermost, extensibility kind innermost.
        pub const VARIANT_NAMES: [&str; 25] = [$(stringify!($t)),*];

        /// `ShapeTypeVariants::create_writer`. `None` = the name is not
        /// one of the 25.
        ///
        /// # Errors
        /// Whatever topic/publisher/writer creation reports.
        pub fn create_writer(
            type_name: &str,
            participant: &DomainParticipant,
            topic_name: &str,
        ) -> Option<zerodds_dcps::Result<Box<dyn WriterBase>>> {
            match type_name {
                $(n if n == stringify!($t) => {
                    Some(build_writer::<generated::$t>(participant, topic_name))
                })*
                _ => None,
            }
        }

        /// `ShapeTypeVariants::create_reader`. `None` = the name is not
        /// one of the 25.
        ///
        /// # Errors
        /// Whatever topic/subscriber/reader creation reports.
        pub fn create_reader(
            type_name: &str,
            participant: &DomainParticipant,
            topic_name: &str,
        ) -> Option<zerodds_dcps::Result<Box<dyn ReaderBase>>> {
            match type_name {
                $(n if n == stringify!($t) => {
                    Some(build_reader::<generated::$t>(participant, topic_name))
                })*
                _ => None,
            }
        }

        /// The extensibility each variant was generated with, in the
        /// order of [`VARIANT_NAMES`]. Read from the generated
        /// `DdsType::EXTENSIBILITY`, so it is what the codec and the
        /// announced `TypeObject` agree on and not a second claim beside
        /// them.
        #[must_use]
        pub fn variant_extensibilities() -> [zerodds_dcps::Extensibility; 25] {
            [$(<generated::$t as DdsType>::EXTENSIBILITY),*]
        }

        /// Every variant's `DdsType::type_object()`, in the order of
        /// [`VARIANT_NAMES`].
        #[must_use]
        pub fn variant_type_objects() -> [Option<zerodds_types::type_object::TypeObject>; 25] {
            [$(<generated::$t as DdsType>::type_object()),*]
        }

        /// Every variant's `DdsType::TYPE_IDENTIFIER`, in the order of
        /// [`VARIANT_NAMES`].
        #[must_use]
        pub fn variant_type_identifiers() -> [zerodds_types::TypeIdentifier; 25] {
            [$(<generated::$t as DdsType>::TYPE_IDENTIFIER),*]
        }

        /// Whether every variant declares a key. All 25 do — `color` is
        /// `@key` in each of them.
        #[must_use]
        pub fn variant_keyed_flags() -> [bool; 25] {
            [$(<generated::$t as DdsType>::HAS_KEY),*]
        }
    };
}

variant_dispatch! {
    Shape1Default, Shape1Final, Shape1Extensible, Shape1Mutable, Shape1MutableExplicitID,
    Shape2Default, Shape2Final, Shape2Extensible, Shape2Mutable, Shape2MutableExplicitID,
    Shape3Default, Shape3Final, Shape3Extensible, Shape3Mutable, Shape3MutableExplicitID,
    Shape4Default, Shape4Final, Shape4Extensible, Shape4Mutable, Shape4MutableExplicitID,
    Shape5Default, Shape5Final, Shape5Extensible, Shape5Mutable, Shape5MutableExplicitID,
}

/// `ShapeTypeVariants::check_type_variant`.
#[must_use]
pub fn is_variant(type_name: &str) -> bool {
    VARIANT_NAMES.contains(&type_name)
}

/// `ShapeTypeVariants::print_type_variants`: five per line, one line per
/// version, each name followed by `", "` including the last on the line.
pub fn print_variants(out: &mut String) {
    for (i, name) in VARIANT_NAMES.iter().enumerate() {
        out.push_str(name);
        out.push_str(", ");
        if i % 5 == 4 {
            out.push('\n');
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_dcps::Extensibility;

    /// The suite drives 25 types and the client has to be able to be
    /// each of them.
    ///
    /// `srcCxx/ShapeType.idl` declares `Shape1`…`Shape5` in five
    /// extensibility forms. A generator that dropped one — an
    /// `@mutable` it cannot emit, an inheritance it cannot flatten —
    /// would leave `-type` with a name it rejects, and the pair that
    /// name belongs to would go unmeasured rather than failing.
    #[test]
    fn all_twenty_five_variants_dispatch() {
        assert_eq!(VARIANT_NAMES.len(), 25);
        for name in VARIANT_NAMES {
            assert!(is_variant(name), "{name} is not accepted by -type");
        }
        // Every family and every kind is present, spelled the way the
        // suite spells it.
        for version in 1..=5 {
            for kind in [
                "Default",
                "Final",
                "Extensible",
                "Mutable",
                "MutableExplicitID",
            ] {
                let expected = format!("Shape{version}{kind}");
                assert!(
                    is_variant(&expected),
                    "the suite's -type {expected} is not one of ours"
                );
            }
        }
    }

    /// The type this client measures with announces itself.
    ///
    /// This is the regression guard for the reason the types are
    /// generated at all. `DdsType::type_object()` defaults to `None`,
    /// and a hand-written type takes that default: an endpoint built on
    /// it registers no `TypeObject` with the participant's TypeLookup
    /// server and its SEDP announcement carries no
    /// `PID_TYPE_INFORMATION` (XTypes 1.3 §7.6.3.2) — a topic type name
    /// with no type behind it. For a suite that measures assignability
    /// that is not a detail: with no `TypeObject` there is nothing for
    /// §7.2.4.4.4.4 to compare, and every pair would match or fail for
    /// the wrong reason. Anyone who points this client at a hand form
    /// fails here, 25 times.
    #[test]
    fn every_measured_type_announces_a_type_object() {
        for (name, obj) in VARIANT_NAMES.iter().zip(variant_type_objects()) {
            assert!(
                obj.is_some(),
                "{name} announces no TypeObject; it is a hand-written type, \
                 not the generated one"
            );
        }
        for (name, id) in VARIANT_NAMES.iter().zip(variant_type_identifiers()) {
            assert_ne!(
                id,
                zerodds_types::TypeIdentifier::None,
                "{name} announces no TypeIdentifier"
            );
        }
    }

    /// The five extensibility declarations of the IDL reach the
    /// generated types.
    ///
    /// XTypes 1.3 Tab. 59 gives each of the three kinds a different
    /// XCDR2 framing — PLAIN_CDR2 for `@final`, DELIMITED_CDR for
    /// `@appendable`, PL_CDR2 for `@mutable` — and §7.2.4.4.4.4 a
    /// different assignability rule. A generator that collapsed
    /// `@mutable` to `@appendable` (which is what a front end that
    /// ignores the annotation does) would leave 10 of the 25 types
    /// measuring the wrong thing while still producing samples.
    ///
    /// `…Default` is APPENDABLE because `build.rs` says so and
    /// XTypes 1.3 §7.3.1.2.1.9 makes that the default extensibility;
    /// the suite's own compatibility matrix treats Default and
    /// Extensible as the same throughout.
    #[test]
    fn the_five_extensibility_forms_survive_generation() {
        let exts = variant_extensibilities();
        for (name, ext) in VARIANT_NAMES.iter().zip(exts) {
            let expected = if name.ends_with("Final") {
                Extensibility::Final
            } else if name.ends_with("Mutable") || name.ends_with("MutableExplicitID") {
                Extensibility::Mutable
            } else {
                // `…Default` and `…Extensible`.
                Extensibility::Appendable
            };
            assert_eq!(ext, expected, "{name} was generated as {ext:?}");
        }
        assert_eq!(
            exts.iter()
                .filter(|e| **e == Extensibility::Mutable)
                .count(),
            10,
            "the ten @mutable types of the suite"
        );
        assert_eq!(
            exts.iter().filter(|e| **e == Extensibility::Final).count(),
            5,
            "the five @final types of the suite"
        );
    }

    /// `color` is `@key` in all 25 declarations.
    ///
    /// It decides the instance, and XTypes 1.3 §7.2.4.4.4.4 has the key
    /// members enter assignability. A generated type that lost the
    /// annotation would publish one instance for every colour and
    /// announce a type whose key set is empty.
    #[test]
    fn every_variant_keeps_its_key_member() {
        for (name, keyed) in VARIANT_NAMES.iter().zip(variant_keyed_flags()) {
            assert!(keyed, "{name} lost its @key color");
        }
    }

    /// `STR_LEN_MAX` is 32 in this suite and 128 in `dds-rtps`.
    ///
    /// The bound is part of the wire type (XTypes 1.3 §7.3.1.2.1.4 puts
    /// it in the `StringSTypeDefn`), so it reaches assignability: with
    /// `ignore_string_bounds = false` a `string<128>` writer is not
    /// assignable to a `string<32>` reader. Copying `dds-rtps`'s 128
    /// into this IDL would answer a different question than the suite
    /// asks, silently.
    #[test]
    fn the_string_bound_is_the_one_this_suite_declares() {
        assert_eq!(generated::STR_LEN_MAX, 32);
    }

    /// Big-endian is its own encoding, not little-endian with a
    /// different stamp.
    ///
    /// The hand-written `ShapeType` of `tools/omg-shape-main` did not
    /// override `encode_be`, so the `DdsType` default delegated to the
    /// little-endian body while the writer stamped `D_CDR2_BE` (RTPS 2.5
    /// §10.5 — the representation identifier covers the whole payload).
    /// Checked here for one type of each extensibility kind, because the
    /// three framings are three different code paths in the generator.
    #[test]
    fn the_two_byte_orders_do_not_produce_the_same_octets() {
        fn check<T: ShapeVariant>(name: &str) {
            let sample = T::fill_data("RED", 7);
            let mut le = Vec::new();
            DdsType::encode(&sample, &mut le).expect("little-endian encode");
            let mut be = Vec::new();
            DdsType::encode_be(&sample, &mut be).expect("big-endian encode");
            assert_ne!(
                le, be,
                "{name}: encode_be returned the little-endian octets; the \
                 big-endian body would go out under a D_CDR2_BE stamp"
            );
            assert_eq!(
                le.len(),
                be.len(),
                "{name}: the two orders write the same length"
            );
        }
        check::<generated::Shape1Final>("Shape1Final");
        check::<generated::Shape1Extensible>("Shape1Extensible");
        check::<generated::Shape1Mutable>("Shape1Mutable");
        check::<generated::Shape3MutableExplicitID>("Shape3MutableExplicitID");
    }

    /// A sample survives its own codec, in every one of the 25 types.
    ///
    /// The `@mutable` codec is the one this checks that nothing else
    /// does: it writes an EMHEADER per member (XTypes 1.3 §7.4.3.4.1)
    /// and is the only path where `@id(30)` reaches the wire. A round
    /// trip that loses a member is silent — the reader gets the member's
    /// default and reports success.
    #[test]
    fn every_variant_round_trips_its_own_sample() {
        fn check<T: ShapeVariant + PartialEq + core::fmt::Debug>(name: &str) {
            let sample = T::fill_data("GREEN", 13);
            let mut buf = Vec::new();
            DdsType::encode(&sample, &mut buf).expect("encode");
            let back = <T as DdsType>::decode(&buf).expect("decode");
            assert_eq!(back, sample, "{name} did not survive its own codec");
        }
        macro_rules! round_trip {
            ($($t:ident),* $(,)?) => { $( check::<generated::$t>(stringify!($t)); )* };
        }
        round_trip! {
            Shape1Default, Shape1Final, Shape1Extensible, Shape1Mutable, Shape1MutableExplicitID,
            Shape2Default, Shape2Final, Shape2Extensible, Shape2Mutable, Shape2MutableExplicitID,
            Shape3Default, Shape3Final, Shape3Extensible, Shape3Mutable, Shape3MutableExplicitID,
            Shape4Default, Shape4Final, Shape4Extensible, Shape4Mutable, Shape4MutableExplicitID,
            Shape5Default, Shape5Final, Shape5Extensible, Shape5Mutable, Shape5MutableExplicitID,
        }
    }

    /// The filler is the suite's, value for value.
    ///
    /// `ShapeType_variants.h`: `x = count % 250`, `y = 2*count % 250`,
    /// `shapesize = 30`, `z = 3*count % 250`, `angle = (5*count) % 360`.
    /// A client whose samples differ from the suite's is still
    /// measurable for assignability, but a reviewer comparing two
    /// vendors' logs side by side would see two different data sets.
    #[test]
    fn the_filler_values_are_the_suites() {
        let s1 = generated::Shape1Default::fill_data("BLUE", 100);
        assert_eq!((s1.x, s1.y, s1.shapesize), (100, 200, 30));
        let s2 = generated::Shape2Default::fill_data("BLUE", 100);
        assert_eq!(s2.angle, 140.0_f32, "(5*100) % 360");
        let s3 = generated::Shape3Default::fill_data("BLUE", 100);
        assert_eq!(s3.z, 50, "(3*100) % 250");
        let s4 = generated::Shape4Default::fill_data("BLUE", 130);
        assert_eq!(
            (s4.x, s4.y, s4.shapesize),
            (130, 10, 30),
            "2*130 % 250 = 10"
        );
        let s5 = generated::Shape5Default::fill_data("BLUE", 100);
        assert_eq!(
            (s5.parent.x, s5.parent.y, s5.parent.shapesize, s5.angle),
            (s2.x, s2.y, s2.shapesize, s2.angle),
            "Shape5 fills like Shape2 — same members, reached through `parent`"
        );
    }

    /// `print_data` reproduces the vendor layout the suite's README
    /// shows, down to the leading newline and the three spaces.
    ///
    /// A reviewer reads two logs against each other; a differently
    /// indented line is a diff that says nothing about the measurement.
    #[test]
    fn print_data_matches_the_layout_the_suite_documents() {
        let mut out = String::new();
        generated::Shape1Extensible::fill_data("BLUE", 1).print_data(&mut out);
        assert_eq!(
            out,
            "\n   color: \"BLUE\"\n   x: 1\n   y: 2\n   shapesize: 30\n"
        );
        let mut out = String::new();
        generated::Shape2Extensible::fill_data("BLUE", 0).print_data(&mut out);
        assert_eq!(
            out, "\n   color: \"BLUE\"\n   x: 0\n   y: 0\n   shapesize: 30\n   angle: 0.000000\n",
            "a float prints as C's %f — six fractional digits"
        );
        let mut out = String::new();
        generated::Shape4Extensible::fill_data("BLUE", 1).print_data(&mut out);
        assert_eq!(
            out, "\n   color: \"BLUE\"\n   shapesize: 30\n   x: 1\n   y: 2\n",
            "Shape4 prints in its own declaration order — shapesize before x"
        );
    }

    /// `-help-types` prints what `ShapeTypeVariants::print_type_variants`
    /// prints: one line per version, five names per line, every name
    /// followed by a comma and a space.
    #[test]
    fn help_types_lists_five_lines_of_five() {
        let mut out = String::new();
        print_variants(&mut out);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 5);
        for (i, line) in lines.iter().enumerate() {
            let names: Vec<&str> = line.split(", ").filter(|s| !s.is_empty()).collect();
            assert_eq!(names.len(), 5, "line {i}: {line}");
            for name in names {
                assert!(name.starts_with(&format!("Shape{}", i + 1)), "{name}");
            }
        }
    }

    /// A name that is not one of the 25 gets no writer and no reader.
    ///
    /// The suite's own client answers an unknown `-type` with
    /// `Error: Type "X" is not a valid type` and the list; the dispatch
    /// has to say `None` for that to be reachable rather than creating
    /// something.
    #[test]
    fn an_unknown_type_name_dispatches_to_nothing() {
        assert!(!is_variant("ShapeType"));
        assert!(!is_variant("Shape6Default"));
        assert!(!is_variant("shape1default"));
        assert!(!is_variant(""));
    }
}
