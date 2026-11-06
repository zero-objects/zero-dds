//! XTypes 1.3 §7.6.8: a derived type that owns its fields reads its
//! KeyHash and key holder from a body by the plan, the key members read and
//! every other member stepped over, and they are those of the whole
//! sample.

#![allow(clippy::unwrap_used, missing_docs)]

use zerodds_codec::{DdsSerialize, DdsType, Encapsulation};

macro_rules! keyed {
    ($outer:ident, $outer_name:literal, $inner:ident, $inner_name:literal, $ext:literal) => {
        #[derive(DdsType, Debug, Clone, PartialEq)]
        #[dds(name = $inner_name, extensibility = $ext, nested)]
        struct $inner {
            note: String,
            #[dds(key)]
            a: u32,
            tags: Vec<String>,
        }

        #[derive(DdsType, Debug, Clone, PartialEq)]
        #[dds(name = $outer_name, extensibility = $ext)]
        struct $outer {
            text: String,
            #[dds(key)]
            id: i64,
            blob: Vec<u8>,
            #[dds(key)]
            name: String,
            maybe: Option<u32>,
            #[dds(key)]
            inner: $inner,
            pair: [u16; 3],
            #[dds(key)]
            flag: bool,
            value: f64,
        }

        impl $outer {
            fn sample(id: i64, filler: usize, maybe: Option<u32>) -> Self {
                Self {
                    text: "x".repeat(filler),
                    id,
                    blob: vec![0xA5; filler],
                    name: String::from("sensor-7"),
                    maybe,
                    inner: $inner {
                        note: "n".repeat(filler / 3),
                        a: 0x0102_0304,
                        tags: vec![String::from("t"); filler % 5],
                    },
                    pair: [1, 2, 3],
                    flag: true,
                    value: 2.5,
                }
            }
        }
    };
}

keyed!(
    FinalKeyed,
    "k::FinalKeyed",
    FinalInner,
    "k::FinalInner",
    "final"
);
keyed!(
    AppendableKeyed,
    "k::AppendableKeyed",
    AppendableInner,
    "k::AppendableInner",
    "appendable"
);
keyed!(
    MutableKeyed,
    "k::MutableKeyed",
    MutableInner,
    "k::MutableInner",
    "mutable"
);

/// Whether the plan read takes the body; a PL_CDR1 parameter list
/// (`@mutable` under XCDR1) is read whole.
fn check<T: DdsSerialize + zerodds_dcps::DdsType>(value: &T, mutable: bool) {
    let hash = zerodds_codec::key_hash(value).unwrap().unwrap();
    let holder = zerodds_codec::key_holder(value).unwrap();
    for encapsulation in Encapsulation::ALL {
        let mut body = Vec::new();
        zerodds_codec::encode(value, encapsulation, &mut body).unwrap();
        let big_endian = matches!(
            encapsulation,
            Encapsulation::Xcdr2Be | Encapsulation::Xcdr1Be
        );
        let read = T::key_material_from_body(&body, encapsulation.is_xcdr1(), big_endian);
        if mutable && encapsulation.is_xcdr1() {
            assert!(read.is_none(), "{encapsulation:?}");
        } else {
            assert_eq!(read, Some(Ok((hash, holder.clone()))), "{encapsulation:?}");
        }
        // A body cut short gives no key: refused, or read whole.
        body.truncate(body.len() / 2);
        let cut = T::key_material_from_body(&body, encapsulation.is_xcdr1(), big_endian);
        assert!(
            matches!(cut, None | Some(Err(_))),
            "{encapsulation:?}: {cut:?}"
        );
    }
}

#[test]
fn xtypes_7_6_8_the_key_read_by_the_plan_is_the_key_of_the_whole_sample() {
    for (id, filler, maybe) in [(7, 0, None), (-3, 1, Some(9)), (1 << 40, 1000, Some(0))] {
        check(&FinalKeyed::sample(id, filler, maybe), false);
        check(&AppendableKeyed::sample(id, filler, maybe), false);
        check(&MutableKeyed::sample(id, filler, maybe), true);
    }
    // The members stepped over do not change the key.
    let a = zerodds_codec::key_hash(&FinalKeyed::sample(7, 0, None)).unwrap();
    let b = zerodds_codec::key_hash(&FinalKeyed::sample(7, 1000, Some(1))).unwrap();
    assert_eq!(a, b);
}

/// How long reading the key by the plan takes against decoding the whole
/// sample and keying the value, for a sample of about two kilobytes. Run
/// on its own, in a release build:
/// `cargo test --release -p zerodds-codec --test key_material_by_plan -- --ignored --nocapture`.
#[test]
#[ignore = "a timing, not a check"]
#[allow(clippy::print_stdout)]
fn key_by_plan_timing() {
    let value = FinalKeyed::sample(42, 1000, Some(1));
    let mut body = Vec::new();
    zerodds_codec::encode(&value, Encapsulation::Xcdr2Le, &mut body).unwrap();
    let rounds = 50_000u32;
    let started = std::time::Instant::now();
    for _ in 0..rounds {
        let decoded: FinalKeyed = zerodds_codec::decode(&body, Encapsulation::Xcdr2Le).unwrap();
        let hash = zerodds_codec::key_hash(&decoded).unwrap();
        let holder = zerodds_codec::key_holder(&decoded).unwrap();
        assert!(hash.is_some() && !holder.is_empty());
    }
    let whole = started.elapsed() / rounds;
    let started = std::time::Instant::now();
    for _ in 0..rounds {
        let read =
            <FinalKeyed as zerodds_dcps::DdsType>::key_material_from_body(&body, false, false);
        assert!(matches!(read, Some(Ok(_))));
    }
    let by_plan = started.elapsed() / rounds;
    std::println!(
        "{} octets: whole sample decoded and keyed {whole:?}; key by the plan {by_plan:?} (per sample)",
        body.len()
    );
}
