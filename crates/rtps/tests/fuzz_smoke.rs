//! Stable-Rust Fuzz-Smoke-Tests: pseudo-random Byte-Streams in alle
//! Wire-Decoder + den FragmentAssembler. **Kein echtes coverage-guided
//! Fuzzing**, aber ein Panic-Smoke: der Decoder darf auf *keinen* Input
//! panic/abort — nur `Ok(..)` oder `Err(WireError::..)` sind erlaubt.
//!
//! Fuer echtes cargo-fuzz-Targets siehe `crates/rtps/fuzz/` (benoetigt
//! nightly + `cargo install cargo-fuzz`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_rtps::datagram::decode_datagram;
use zerodds_rtps::fragment_assembler::FragmentAssembler;
use zerodds_rtps::submessages::{
    AckNackSubmessage, DataFragSubmessage, DataSubmessage, FragmentNumberSet, GapSubmessage,
    HeartbeatFragSubmessage, HeartbeatSubmessage, NackFragSubmessage, SequenceNumberSet,
};

mod common;
use common::XorShift32;

/// Generiert einen pseudo-random Byte-Buffer aus dem Seed.
fn random_bytes(rng: &mut XorShift32, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let w = rng.next_u32().to_le_bytes();
        out.extend_from_slice(&w);
    }
    out.truncate(len);
    out
}

/// Wrapper: nimmt Closure, die auf zufaelligem Input operieren soll,
/// und treibt sie mit N Iterationen in variablen Groessen. Erlaubt
/// beliebige Ergebnisse — der Test schlaegt **nur** bei Panic fehl.
fn fuzz<F: FnMut(&[u8])>(seed: u32, iterations: usize, mut f: F) {
    let mut rng = XorShift32::new(seed);
    for i in 0..iterations {
        // Laengen: 0, 1, 4, 16, 64, 256, 1024, 4096 — deckt typische
        // Buffer-Grenzen ab.
        let len = match i % 8 {
            0 => 0,
            1 => 1,
            2 => 4,
            3 => 16,
            4 => 64,
            5 => 256,
            6 => 1024,
            _ => 4096,
        };
        let bytes = random_bytes(&mut rng, len);
        f(&bytes);
    }
}

#[test]
fn fuzz_decode_datagram_does_not_panic() {
    fuzz(0x1234_5678, 2000, |buf| {
        // Darf nur Ok(_) oder Err(WireError::_) zurueckgeben — kein Panic.
        let _ = decode_datagram(buf);
    });
}

#[test]
fn fuzz_sequence_number_set_read_from_does_not_panic() {
    fuzz(0xCAFE_BABE, 2000, |buf| {
        let _ = SequenceNumberSet::read_from(buf, 0, true);
        let _ = SequenceNumberSet::read_from(buf, 0, false);
    });
}

#[test]
fn fuzz_fragment_number_set_read_from_does_not_panic() {
    fuzz(0xDEAD_BEEF, 2000, |buf| {
        let _ = FragmentNumberSet::read_from(buf, 0, true);
        let _ = FragmentNumberSet::read_from(buf, 0, false);
    });
}

#[test]
fn fuzz_data_read_body_does_not_panic() {
    fuzz(0xFACE_B00C, 1000, |buf| {
        let _ = DataSubmessage::read_body(buf, true);
    });
}

#[test]
fn fuzz_data_frag_read_body_does_not_panic() {
    fuzz(0x1337_F00D, 1000, |buf| {
        // Alle Flag-Kombinationen — Fuzzer soll keine triggert nur
        // eine bestimmte Branch.
        for flags in 0..16_u8 {
            let q = flags & 1 != 0;
            let h = flags & 2 != 0;
            let k = flags & 4 != 0;
            let n = flags & 8 != 0;
            let _ = DataFragSubmessage::read_body(buf, true, q, h, k, n);
        }
    });
}

#[test]
fn fuzz_heartbeat_read_body_does_not_panic() {
    fuzz(0xBEEF_CAFE, 1000, |buf| {
        for (f, l) in [(true, true), (true, false), (false, true), (false, false)] {
            let _ = HeartbeatSubmessage::read_body(buf, true, f, l, false);
            let _ = HeartbeatSubmessage::read_body(buf, true, f, l, true);
        }
    });
}

#[test]
fn fuzz_acknack_read_body_does_not_panic() {
    fuzz(0x5EED_1234, 1000, |buf| {
        for final_flag in [true, false] {
            let _ = AckNackSubmessage::read_body(buf, true, final_flag);
        }
    });
}

#[test]
fn fuzz_gap_read_body_does_not_panic() {
    fuzz(0xABCD_EF01, 1000, |buf| {
        let _ = GapSubmessage::read_body(buf, true, false, false);
        // Auch mit gesetzten Flags fuzzen, um den Trailer-Pfad zu treffen.
        let _ = GapSubmessage::read_body(buf, true, true, true);
    });
}

#[test]
fn fuzz_heartbeat_frag_read_body_does_not_panic() {
    fuzz(0x0BAD_F00D, 1000, |buf| {
        let _ = HeartbeatFragSubmessage::read_body(buf, true);
    });
}

#[test]
fn fuzz_nack_frag_read_body_does_not_panic() {
    fuzz(0x600D_1DEA, 1000, |buf| {
        let _ = NackFragSubmessage::read_body(buf, true);
    });
}

/// DataFrag-Fixture-Helfer fuer Fuzz-Tests.
fn df_arbitrary(
    sn: i64,
    starting: u32,
    count: u16,
    frag_size: u16,
    sample_size: u32,
    payload: Vec<u8>,
) -> DataFragSubmessage {
    use zerodds_rtps::wire_types::{EntityId, FragmentNumber, SequenceNumber};
    DataFragSubmessage {
        extra_flags: 0,
        reader_id: EntityId::user_reader_with_key([1, 2, 3]),
        writer_id: EntityId::user_writer_with_key([4, 5, 6]),
        writer_sn: SequenceNumber(sn),
        fragment_starting_num: FragmentNumber(starting),
        fragments_in_submessage: count,
        fragment_size: frag_size,
        sample_size,
        serialized_payload: payload.into(),
        inline_qos_flag: false,
        hash_key_flag: false,
        key_flag: false,
        non_standard_flag: false,
    }
}

#[test]
fn fuzz_assembler_insert_random_does_not_panic() {
    let mut rng = XorShift32::new(0xDEAD_BABE);
    let mut a = FragmentAssembler::default();
    for _ in 0..5000 {
        let payload_len = (rng.next_u32() % 1000) as usize;
        let payload = random_bytes(&mut rng, payload_len);
        let df = df_arbitrary(
            (rng.next_u32() as i64) % 1000,
            rng.next_u32() % 100,
            (rng.next_u32() % 8) as u16,
            (rng.next_u32() % 2000) as u16,
            rng.next_u32() % 10_000,
            payload,
        );
        let _ = a.insert(&df);
    }
    assert!(a.len() <= 64, "assembler overran max_pending_sns cap");
}

#[test]
fn fuzz_assembler_edge_values_do_not_panic() {
    // Gezielte Edge-Werte, die ein Pure-Random-Fuzzer selten trifft.
    // Jeder Case muss via DoS-Caps oder Validierung abgefangen werden —
    // nie Panic, nie OOM-Allocation.
    let mut a = FragmentAssembler::default();

    // sample_size = u32::MAX → SampleTooLarge (no alloc)
    let _ = a.insert(&df_arbitrary(1, 1, 1, 1000, u32::MAX, vec![]));
    // sample_size = 0, fragment_size > 0 → total_fragments=0, is_complete()=false
    let _ = a.insert(&df_arbitrary(2, 1, 1, 100, 0, vec![]));
    // fragment_size = 0 → FragmentSizeInvalid
    let _ = a.insert(&df_arbitrary(3, 1, 1, 0, 100, vec![]));
    // fragment_size = u16::MAX, sample_size = u16::MAX-1 → genau 1 Fragment
    let _ = a.insert(&df_arbitrary(
        4,
        1,
        1,
        u16::MAX,
        u32::from(u16::MAX) - 1,
        vec![0; u16::MAX as usize - 1],
    ));
    // fragment_starting_num = u32::MAX → FragmentIndexOutOfRange
    let _ = a.insert(&df_arbitrary(5, u32::MAX, 1, 4, 12, vec![1, 2, 3, 4]));
    // fragment_starting_num = 0 → FragmentIndexZero
    let _ = a.insert(&df_arbitrary(6, 0, 1, 4, 4, vec![1, 2, 3, 4]));
    // fragments_in_submessage = 0 → FragmentsInSubmessageInvalid
    let _ = a.insert(&df_arbitrary(7, 1, 0, 4, 4, vec![1, 2, 3, 4]));
    // fragments_in_submessage = u16::MAX → starting + count Overflow
    let _ = a.insert(&df_arbitrary(8, 1, u16::MAX, 4, 100, vec![]));
    // Payload leer, aber erwartet voll → PayloadSizeMismatch
    let _ = a.insert(&df_arbitrary(9, 1, 1, 4, 4, vec![]));
    // Payload zu lang → PayloadSizeMismatch
    let _ = a.insert(&df_arbitrary(10, 1, 1, 4, 4, vec![0; 100]));
    // writer_sn = 0 (SN-Space beginnt spec-maessig bei 1, aber wir
    // pruefen nur ≤ delivered_up_to — Assembler sieht writer_sn nie direkt)
    let _ = a.insert(&df_arbitrary(0, 1, 1, 4, 4, vec![1, 2, 3, 4]));
    // writer_sn negativ (illegal per Spec, aber i64-Wire-Feld erlaubt's)
    let _ = a.insert(&df_arbitrary(-1, 1, 1, 4, 4, vec![1, 2, 3, 4]));
    // Inkonsistente Folge-Fragmente (schon-vorhandene-sn + anderer
    // sample_size) → InconsistentWithBuffered
    let _ = a.insert(&df_arbitrary(20, 1, 1, 4, 8, vec![1, 2, 3, 4]));
    let _ = a.insert(&df_arbitrary(20, 2, 1, 4, 99, vec![5, 6, 7, 8]));

    // Nach all dem darf der Assembler weiter sinnvoll arbeiten
    let res = a.insert(&df_arbitrary(100, 1, 1, 4, 4, vec![1, 2, 3, 4]));
    assert!(res.is_some(), "assembler still works after edge inputs");
}

#[test]
fn fuzz_assembler_slow_drip_attack_evicts_oldest() {
    // Slow-drip: Angreifer schickt je 1 Fragment fuer viele SNs.
    // max_pending_sns=64 → ab 65. SN muss evicten, und zwar **LRU:
    // die kleinste/aelteste SN**, nicht die neueste.
    use zerodds_rtps::wire_types::{FragmentNumber, SequenceNumber};
    let mut a = FragmentAssembler::default();
    for sn in 1..=200i64 {
        let _ = a.insert(&df_arbitrary(sn, 1, 1, 4, 12, vec![1, 2, 3, 4]));
    }
    assert_eq!(a.len(), 64, "max_pending_sns cap enforced");
    assert!(
        a.drop_count() >= 200 - 64,
        "dropped >= 136 buffers via LRU eviction"
    );

    // D4-Schaerfung: pruefen, dass die **letzten 64 SNs** drin sind
    // (137..=200), und alle davor rausgeworfen wurden. Wenn die LRU-
    // Richtung umgedreht wuerde (neueste statt aelteste evicten),
    // wuerde dieser Test fehlschlagen.
    let active: Vec<_> = a.incomplete_sns().collect();
    assert_eq!(active.len(), 64);
    // Sortiert nach SN (BTreeMap-Iteration) → ordinal exakt 137..=200.
    for (i, sn) in active.iter().enumerate() {
        assert_eq!(
            *sn,
            SequenceNumber(137 + i as i64),
            "eviction should drop oldest SNs (1..=136), keep newest (137..=200); \
             got SN {} at position {}",
            sn.0,
            i
        );
    }
    // Zusaetzliche Absicherung: eine alte SN (sn=10) ist weg →
    // missing_fragments liefert leeres Set, nicht etwa die vorherigen
    // Luecken.
    let missing_of_old = a.missing_fragments(SequenceNumber(10));
    assert_eq!(
        missing_of_old.num_bits, 0,
        "evicted SN 10 must have empty missing-set (it's gone, not just unfinished)"
    );
    // Eine aktuelle SN hat eine Luecke (wir haben nur Fragment 1 von 3
    // geschickt, also fehlen 2 und 3).
    let missing_of_new = a.missing_fragments(SequenceNumber(200));
    let missing_vec: Vec<_> = missing_of_new.iter_set().collect();
    assert_eq!(
        missing_vec,
        vec![FragmentNumber(2), FragmentNumber(3)],
        "SN 200 still in-flight with Fragments 2+3 missing"
    );
}
