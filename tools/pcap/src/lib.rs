// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-pcap` library — parsing primitives.
//!
//! Crate `zerodds-pcap`. Safety classification: **COMFORT**.
//! Offline pcap-/RTPS-Decoder; pure parsing, kein Runtime-Pfad.
//!
//! Wir implementieren einen leichten libpcap-File-Reader (LE und BE
//! Magic-Words) und einen "best-effort" RTPS-Locator: jeder packet-
//! payload wird nach dem `RTPS`-Magic durchsucht; ab dem Treffer
//! versuchen wir [`zerodds_rtps::datagram::decode_datagram`].
//!
//! Damit funktioniert das Tool für **alle gängigen pcap-Captures**
//! (Ethernet/IP/UDP/RTPS) ohne dass wir L2/L3/L4-Parsing
//! implementieren müssen — die UDP-Payload startet immer mit `RTPS`.

#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

/// Pcap libpcap-Format Magic, Little-Endian Variante.
pub const PCAP_MAGIC_LE: u32 = 0xA1B2_C3D4;
/// Pcap libpcap-Format Magic, Big-Endian Variante.
pub const PCAP_MAGIC_BE: u32 = 0xD4C3_B2A1;
/// Pcap libpcap-Format Magic, nanosecond-precision LE.
pub const PCAP_MAGIC_NS_LE: u32 = 0xA1B2_3C4D;
/// RTPS-Magic (`'R','T','P','S'`).
pub const RTPS_MAGIC: &[u8; 4] = b"RTPS";

/// Pcap-File-Header (24 Bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcapFileHeader {
    /// Magic-Word (siehe [`PCAP_MAGIC_LE`]).
    pub magic: u32,
    /// Major-Version. Standard: 2.
    pub version_major: u16,
    /// Minor-Version. Standard: 4.
    pub version_minor: u16,
    /// Snapshot-Length (DoS-Cap pro packet).
    pub snaplen: u32,
    /// Datalink-Layer-Type (1 = Ethernet, 113 = LinuxSLL, 0 = NULL).
    pub linktype: u32,
}

/// Per-Record Pcap-Header (16 Bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcapRecordHeader {
    /// Timestamp-Sekunden.
    pub ts_sec: u32,
    /// Timestamp-Microsekunden (oder ns je nach Magic).
    pub ts_usec: u32,
    /// Captured-Length (≤ snaplen).
    pub incl_len: u32,
    /// Original-Length im Wire.
    pub orig_len: u32,
}

/// Parse-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PcapError {
    /// Datei zu kurz für Header.
    TooShort,
    /// Magic-Word nicht erkannt.
    BadMagic(u32),
    /// `incl_len` größer als verbleibende Bytes.
    TruncatedRecord,
}

impl std::fmt::Display for PcapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "pcap file too short for header"),
            Self::BadMagic(m) => write!(f, "unknown pcap magic 0x{m:08x}"),
            Self::TruncatedRecord => write!(f, "pcap record exceeds remaining bytes"),
        }
    }
}

impl std::error::Error for PcapError {}

/// Liest den Pcap-File-Header. Liefert (`header`, `is_big_endian`).
///
/// # Errors
/// [`PcapError::TooShort`] oder [`PcapError::BadMagic`].
pub fn parse_file_header(bytes: &[u8]) -> Result<(PcapFileHeader, bool), PcapError> {
    if bytes.len() < 24 {
        return Err(PcapError::TooShort);
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let big_endian = match magic {
        PCAP_MAGIC_LE | PCAP_MAGIC_NS_LE => false,
        PCAP_MAGIC_BE => true,
        m => return Err(PcapError::BadMagic(m)),
    };
    let read_u16 = |off: usize| -> u16 {
        if big_endian {
            u16::from_be_bytes([bytes[off], bytes[off + 1]])
        } else {
            u16::from_le_bytes([bytes[off], bytes[off + 1]])
        }
    };
    let read_u32 = |off: usize| -> u32 {
        if big_endian {
            u32::from_be_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
        } else {
            u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
        }
    };
    Ok((
        PcapFileHeader {
            magic,
            version_major: read_u16(4),
            version_minor: read_u16(6),
            snaplen: read_u32(16),
            linktype: read_u32(20),
        },
        big_endian,
    ))
}

/// Iterator über pcap-Records. Liefert pro Aufruf
/// `Some((header, payload_slice))`.
pub struct PcapIter<'a> {
    bytes: &'a [u8],
    pos: usize,
    big_endian: bool,
}

impl<'a> PcapIter<'a> {
    /// Konstruiert den Iterator. Erwartet, dass [`parse_file_header`]
    /// bereits erfolgreich war.
    #[must_use]
    pub fn new(bytes: &'a [u8], big_endian: bool) -> Self {
        Self {
            bytes,
            pos: 24,
            big_endian,
        }
    }

    /// Liefert nächsten Record oder `None`.
    ///
    /// # Errors
    /// [`PcapError::TruncatedRecord`] wenn `incl_len` über die Datei hinausragt.
    pub fn next_record(&mut self) -> Result<Option<(PcapRecordHeader, &'a [u8])>, PcapError> {
        if self.pos + 16 > self.bytes.len() {
            return Ok(None);
        }
        let read_u32 = |b: &[u8], off: usize, be: bool| -> u32 {
            if be {
                u32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
            } else {
                u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
            }
        };
        let h = PcapRecordHeader {
            ts_sec: read_u32(self.bytes, self.pos, self.big_endian),
            ts_usec: read_u32(self.bytes, self.pos + 4, self.big_endian),
            incl_len: read_u32(self.bytes, self.pos + 8, self.big_endian),
            orig_len: read_u32(self.bytes, self.pos + 12, self.big_endian),
        };
        let data_start = self.pos + 16;
        let data_end = data_start
            .checked_add(h.incl_len as usize)
            .ok_or(PcapError::TruncatedRecord)?;
        if data_end > self.bytes.len() {
            return Err(PcapError::TruncatedRecord);
        }
        self.pos = data_end;
        Ok(Some((h, &self.bytes[data_start..data_end])))
    }
}

/// Sucht den ersten `RTPS`-Magic im `payload` und liefert den
/// resultierenden Offset (oder `None`).
///
/// Damit überspringen wir L2/L3/L4-Header (Ethernet/IP/UDP) ohne sie
/// explizit zu parsen.
#[must_use]
pub fn find_rtps_offset(payload: &[u8]) -> Option<usize> {
    payload.windows(4).position(|w| w == RTPS_MAGIC)
}

/// Sub-command des Pcap-CLIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `parse <FILE>` — drucke RTPS-Submessages pro Frame.
    Parse(FileArgs),
    /// `stats <FILE>` — drucke nur Aggregat-Counts pro Submessage-Typ.
    Stats(FileArgs),
}

/// Argumente für `parse` und `stats`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileArgs {
    /// Pfad zum pcap-File.
    pub file: String,
}

/// Parse-Fehler beim CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Kein subcommand.
    Missing,
    /// Unbekanntes subcommand.
    Unknown(String),
    /// FILE-arg fehlt.
    MissingFile,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "no sub-command given"),
            Self::Unknown(s) => write!(f, "unknown sub-command: {s}"),
            Self::MissingFile => write!(f, "missing FILE argument"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parst `args` (typisch `env::args().skip(1)`) zu einem [`Command`].
///
/// # Errors
/// Siehe [`ParseError`].
pub fn parse_args(args: &[String]) -> Result<Command, ParseError> {
    let sub = args.first().ok_or(ParseError::Missing)?;
    match sub.as_str() {
        "parse" => {
            let file = args.get(1).ok_or(ParseError::MissingFile)?.clone();
            Ok(Command::Parse(FileArgs { file }))
        }
        "stats" => {
            let file = args.get(1).ok_or(ParseError::MissingFile)?.clone();
            Ok(Command::Stats(FileArgs { file }))
        }
        other => Err(ParseError::Unknown(other.to_string())),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    fn make_pcap_le(records: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        // File header: magic LE, vmajor=2, vminor=4, thiszone=0, sigfigs=0, snaplen=65535, linktype=1
        out.extend_from_slice(&PCAP_MAGIC_LE.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // thiszone
        out.extend_from_slice(&0u32.to_le_bytes()); // sigfigs
        out.extend_from_slice(&65535u32.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        for rec in records {
            out.extend_from_slice(&0u32.to_le_bytes()); // ts_sec
            out.extend_from_slice(&0u32.to_le_bytes()); // ts_usec
            out.extend_from_slice(&(rec.len() as u32).to_le_bytes()); // incl_len
            out.extend_from_slice(&(rec.len() as u32).to_le_bytes()); // orig_len
            out.extend_from_slice(rec);
        }
        out
    }

    #[test]
    fn parse_file_header_le() {
        let pcap = make_pcap_le(&[]);
        let (h, be) = parse_file_header(&pcap).unwrap();
        assert_eq!(h.magic, PCAP_MAGIC_LE);
        assert_eq!(h.version_major, 2);
        assert_eq!(h.linktype, 1);
        assert!(!be);
    }

    #[test]
    fn parse_file_header_too_short() {
        assert!(matches!(
            parse_file_header(&[1, 2, 3]),
            Err(PcapError::TooShort)
        ));
    }

    #[test]
    fn parse_file_header_bad_magic() {
        let mut bytes = vec![0u8; 24];
        bytes[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        assert!(matches!(
            parse_file_header(&bytes),
            Err(PcapError::BadMagic(_))
        ));
    }

    #[test]
    fn iter_returns_records() {
        let pcap = make_pcap_le(&[b"hello-record-1", b"hello-record-2"]);
        let (_h, be) = parse_file_header(&pcap).unwrap();
        let mut iter = PcapIter::new(&pcap, be);
        let (_h1, payload1) = iter.next_record().unwrap().unwrap();
        assert_eq!(payload1, b"hello-record-1");
        let (_h2, payload2) = iter.next_record().unwrap().unwrap();
        assert_eq!(payload2, b"hello-record-2");
        assert!(iter.next_record().unwrap().is_none());
    }

    #[test]
    fn find_rtps_offset_no_match() {
        assert!(find_rtps_offset(b"plain text without magic").is_none());
    }

    #[test]
    fn find_rtps_offset_finds_after_header() {
        let mut payload = vec![0u8; 42]; // simulate L2+L3+L4 header
        payload.extend_from_slice(b"RTPS\x02\x05\x01\x10");
        let off = find_rtps_offset(&payload).unwrap();
        assert_eq!(off, 42);
    }

    #[test]
    fn parse_args_parse_subcommand() {
        let cmd = parse_args(&s(&["parse", "x.pcap"])).unwrap();
        assert_eq!(
            cmd,
            Command::Parse(FileArgs {
                file: "x.pcap".into()
            })
        );
    }

    #[test]
    fn parse_args_stats_subcommand() {
        let cmd = parse_args(&s(&["stats", "y.pcap"])).unwrap();
        assert_eq!(
            cmd,
            Command::Stats(FileArgs {
                file: "y.pcap".into()
            })
        );
    }

    #[test]
    fn parse_args_missing_file() {
        assert!(matches!(
            parse_args(&s(&["parse"])),
            Err(ParseError::MissingFile)
        ));
    }

    #[test]
    fn parse_args_unknown() {
        assert!(matches!(
            parse_args(&s(&["foo"])),
            Err(ParseError::Unknown(_))
        ));
    }
}
