//! Gemeinsame Helpers fuer Integration-Tests in `crates/rtps/tests/`.
//!
//! Wird per `mod common;` eingebunden. Test-Dateien sollten keine
//! Duplikate von Generatoren/Fixtures enthalten.

#![allow(dead_code)] // jede Test-Datei nutzt nur einen Ausschnitt

use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

/// Kanonische Test-GUIDs. Writer- und Reader-Seite muessen in E2E-Tests
/// dieselben Werte verwenden, damit `handle_acknack(src_guid, ...)`
/// korrekt dispatcht.
pub const TEST_WRITER_KEY: [u8; 3] = [0x10, 0x20, 0x30];
pub const TEST_READER_KEY: [u8; 3] = [0xA0, 0xB0, 0xC0];

#[must_use]
pub fn test_writer_guid() -> Guid {
    Guid::new(
        GuidPrefix::from_bytes([1; 12]),
        EntityId::user_writer_with_key(TEST_WRITER_KEY),
    )
}

#[must_use]
pub fn test_reader_guid() -> Guid {
    Guid::new(
        GuidPrefix::from_bytes([2; 12]),
        EntityId::user_reader_with_key(TEST_READER_KEY),
    )
}

/// Deterministischer, aber nicht-trivialer Sample-Inhalt fuer
/// byte-genaue Reassembly-Pruefung.
///
/// Formel: `byte[i] = (sn * K + i) & 0xFF`, wobei
/// `K = 0x9E3779B1` (Knuth-Multiplikativ-Hash, Golden-Ratio-Fraktion).
/// Diese Wahl:
/// - streut die Bytes gleichmaessig (kein konstanter oder linearer
///   Wert, der Reassembly-Bugs maskieren wuerde),
/// - ist deterministisch (kein RNG-State; selbe sn ergibt identische
///   Sequenz) — wichtig fuer Test-Reproduzierbarkeit,
/// - ist o(len) berechenbar ohne Tabellen.
///
/// Aendert sich diese Funktion, muessen alle Fragment-E2E-Tests
/// entsprechend angepasst werden (sie vergleichen `pattern_for(i, n)`
/// 1:1 mit dem reassemblierten Reader-Sample).
#[must_use]
pub fn pattern_for(sn: usize, len: usize) -> Vec<u8> {
    let seed = (sn as u32).wrapping_mul(0x9E37_79B1);
    (0..len)
        .map(|i| (seed.wrapping_add(i as u32) & 0xFF) as u8)
        .collect()
}

/// Einfacher xorshift32-RNG — deterministisch, reproducible.
#[derive(Debug, Clone)]
pub struct XorShift32(pub u32);

impl XorShift32 {
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}
