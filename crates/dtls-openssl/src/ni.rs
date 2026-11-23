// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Names of public keys after RFC 6920: the SHA-256 hash of the
//! SubjectPublicKeyInfo (§2), possibly truncated, as an `ni` URI (§3), a
//! human-speakable `nih` URI (§7) or in the binary format (§6). RFC 7252
//! §9.1.3.2.1 identifies a raw public key this way and asks for at least
//! `sha-256-120`.

use openssl::hash::{MessageDigest, hash};

/// The hash algorithms of the Named Information Hash Algorithm Registry
/// (RFC 6920 §9.4, Figure 11): suite id, name, length in bits.
const SUITES: [(u8, &str, usize); 6] = [
    (1, "sha-256", 256),
    (2, "sha-256-128", 128),
    (3, "sha-256-120", 120),
    (4, "sha-256-96", 96),
    (5, "sha-256-64", 64),
    (6, "sha-256-32", 32),
];

/// A name of a public key: the (truncated) SHA-256 hash of its
/// SubjectPublicKeyInfo. Two names are equal when algorithm, length and
/// value are (RFC 6920 §2: "regardless of encoding").
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyName {
    suite: u8,
    value: Vec<u8>,
}

/// Why a text is no name of RFC 6920.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    /// Neither `ni:`, `nih:` nor hex of the binary format.
    Form,
    /// A hash algorithm the registry does not name.
    Algorithm(String),
    /// A value of the wrong length or with a character outside its
    /// encoding.
    Value,
    /// The check digit of a `nih` URI does not match.
    CheckDigit,
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Form => write!(f, "neither ni:, nih: nor the hex of a binary name"),
            Self::Algorithm(a) => write!(f, "unknown hash algorithm `{a}`"),
            Self::Value => write!(f, "the hash value does not fit the algorithm"),
            Self::CheckDigit => write!(f, "the check digit does not match"),
        }
    }
}

impl std::error::Error for NameError {}

fn suite_by_name(name: &str) -> Option<(u8, usize)> {
    // §7: "the alg field MAY contain the ID value as an ASCII-encoded
    // decimal number instead of the hash name string".
    SUITES
        .iter()
        .find(|(id, n, _)| *n == name || id.to_string() == name)
        .map(|(id, _, bits)| (*id, *bits))
}

fn suite_by_id(id: u8) -> Option<(&'static str, usize)> {
    SUITES
        .iter()
        .find(|(i, _, _)| *i == id)
        .map(|(_, n, bits)| (*n, *bits))
}

impl KeyName {
    /// The name of the public key `spki_der` (DER SubjectPublicKeyInfo)
    /// with the algorithm `suite` (1 = sha-256, 3 = sha-256-120, ...).
    ///
    /// # Errors
    /// An id outside the registry.
    pub fn of_key(spki_der: &[u8], suite: u8) -> Result<Self, NameError> {
        let (_, bits) =
            suite_by_id(suite).ok_or_else(|| NameError::Algorithm(suite.to_string()))?;
        let digest = hash(MessageDigest::sha256(), spki_der).map_err(|_| NameError::Value)?;
        // §2: the leftmost N bits.
        Ok(Self {
            suite,
            value: digest[..bits / 8].to_vec(),
        })
    }

    /// `true` when `spki_der` is the key this name names.
    #[must_use]
    pub fn names(&self, spki_der: &[u8]) -> bool {
        Self::of_key(spki_der, self.suite).is_ok_and(|n| n == *self)
    }

    /// The hash algorithm, as the registry names it.
    #[must_use]
    pub fn algorithm(&self) -> &'static str {
        suite_by_id(self.suite).map_or("sha-256", |(n, _)| n)
    }

    /// Parses an `ni` URI (§3, the authority and query ignored as §2 says),
    /// a `nih` URI (§7, the check digit checked when present) or the hex
    /// of the binary format (§6, "-" and spaces allowed).
    ///
    /// # Errors
    /// See [`NameError`].
    pub fn parse(text: &str) -> Result<Self, NameError> {
        let text = text.trim();
        if let Some(rest) = text.strip_prefix("ni://") {
            let path = rest
                .split_once('/')
                .map(|(_, p)| p)
                .ok_or(NameError::Form)?;
            let path = path.split('?').next().unwrap_or(path);
            let (alg, value) = path.split_once(';').ok_or(NameError::Form)?;
            let (suite, bits) =
                suite_by_name(alg).ok_or_else(|| NameError::Algorithm(alg.to_string()))?;
            let value = base64url_decode(value).ok_or(NameError::Value)?;
            return Self::checked(suite, bits, value);
        }
        if let Some(rest) = text.strip_prefix("nih:") {
            let mut fields = rest.split(';');
            let alg = fields.next().ok_or(NameError::Form)?;
            let sepval = fields.next().ok_or(NameError::Form)?;
            let check = fields.next();
            if fields.next().is_some() {
                return Err(NameError::Form);
            }
            let (suite, bits) =
                suite_by_name(alg).ok_or_else(|| NameError::Algorithm(alg.to_string()))?;
            let hex: String = sepval.chars().filter(|c| *c != '-').collect();
            if hex.chars().any(|c| !matches!(c, '0'..='9' | 'a'..='f')) {
                return Err(NameError::Value);
            }
            if let Some(check) = check
                && (check.len() != 1 || luhn16(&hex) != check.chars().next())
            {
                return Err(NameError::CheckDigit);
            }
            let value = hex_decode(&hex).ok_or(NameError::Value)?;
            return Self::checked(suite, bits, value);
        }
        let hex: String = text
            .chars()
            .filter(|c| *c != '-' && !c.is_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        let bytes = hex_decode(&hex).ok_or(NameError::Form)?;
        let (header, value) = bytes.split_first().ok_or(NameError::Form)?;
        // §6: the two reserved bits are ignored on receipt.
        let suite = header & 0x3f;
        let (_, bits) =
            suite_by_id(suite).ok_or_else(|| NameError::Algorithm(suite.to_string()))?;
        Self::checked(suite, bits, value.to_vec())
    }

    fn checked(suite: u8, bits: usize, value: Vec<u8>) -> Result<Self, NameError> {
        if value.len() * 8 != bits {
            return Err(NameError::Value);
        }
        Ok(Self { suite, value })
    }

    /// The `ni` URI without authority (§3).
    #[must_use]
    pub fn to_ni_uri(&self) -> String {
        format!(
            "ni:///{};{}",
            self.algorithm(),
            base64url_encode(&self.value)
        )
    }

    /// The `nih` URI with check digit, in groups of four (§7).
    #[must_use]
    pub fn to_nih_uri(&self) -> String {
        let hex = hex_encode(&self.value);
        let groups: Vec<&str> = hex
            .as_bytes()
            .chunks(4)
            .map(|c| std::str::from_utf8(c).unwrap_or(""))
            .collect();
        format!(
            "nih:{};{};{}",
            self.algorithm(),
            groups.join("-"),
            luhn16(&hex).unwrap_or('0')
        )
    }

    /// The binary format (§6): the suite id, then the value.
    #[must_use]
    pub fn to_binary(&self) -> Vec<u8> {
        let mut out = vec![self.suite & 0x3f];
        out.extend_from_slice(&self.value);
        out
    }
}

impl std::fmt::Display for KeyName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_ni_uri())
    }
}

/// Luhn mod N with N = 16 (ISO/IEC 7812, RFC 6920 §7) over lowercase hex.
fn luhn16(hex: &str) -> Option<char> {
    let mut factor = 2u32;
    let mut sum = 0u32;
    for c in hex.chars().rev() {
        let digit = c.to_digit(16)?;
        let addend = factor * digit;
        factor = if factor == 2 { 1 } else { 2 };
        sum += addend / 16 + addend % 16;
    }
    let check = (16 - sum % 16) % 16;
    std::char::from_digit(check, 16)
}

const B64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64url_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(char::from(B64URL[((n >> (18 - 6 * i)) & 0x3f) as usize]));
        }
    }
    out
}

fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    // §3: "with no "=" padding characters".
    let mut bits = 0u32;
    let mut count = 0u32;
    let mut out = Vec::new();
    for c in text.bytes() {
        let v = B64URL.iter().position(|b| *b == c)? as u32;
        bits = (bits << 6) | v;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    (bits == 0).then_some(out)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// The SubjectPublicKeyInfo of RFC 6920 §8.2, Figure 9.
    const SPKI_HEX: &str = "30820122300d06092a864886f70d01010105000382010f003082010a0282010100a25f83da9bd9f17a3a3667bafd5a940ecf16d55a553a5ed403b1658e6dcfa3b7dba4e7cc0f52c67d351dc468c2bd7b9ddbe40ad710cdf95320ee0dd7566e5b7aae2c5f830a193c725896d686e80ee694eb5cf2903ef3a88a8856b6cd3638762297b16b3c9c07f34f9708a1bc29389b81062b7460387a932f39be1234096e0b5710b7a37bf2c6eed6c1e5ecaec59c8314f46b58e2def2ffc97707e3f34c97cf1a289e38a1b3934175a1a4763f4d78d744d61ae3cee25dc5784cb531222ec74b8c6f56785ca1c4c01dcae5b944d7e9909cbceeb0a2b1dcda6da00ff6ad1e2c12a2a766603e36d49141c2f2e769392c9dd2dfb5a34495487c876489ddbf0501eedd0203010001";

    fn spki() -> Vec<u8> {
        hex_decode(SPKI_HEX).unwrap()
    }

    #[test]
    fn rfc6920_8_2_the_names_of_the_example_key() {
        let full = KeyName::of_key(&spki(), 1).unwrap();
        assert_eq!(
            full.to_ni_uri(),
            "ni:///sha-256;UyaQV-Ev4rdLoHyJJWCi11OHfrYv9E1aGQAlMO2X_-Q"
        );
        let short = KeyName::of_key(&spki(), 3).unwrap();
        assert_eq!(
            hex_encode(&short.to_binary()),
            "0353269057e12fe2b74ba07c892560a2"
        );
        assert_eq!(
            short.to_nih_uri(),
            "nih:sha-256-120;5326-9057-e12f-e2b7-4ba0-7c89-2560-a2;f"
        );
        let tiny = KeyName::of_key(&spki(), 6).unwrap();
        assert_eq!(luhn16(&hex_encode(&tiny.value)), Some('b'));
    }

    #[test]
    fn rfc6920_2_the_encodings_of_one_name_are_equal() {
        let short = KeyName::of_key(&spki(), 3).unwrap();
        for text in [
            "nih:sha-256-120;5326-9057-e12f-e2b7-4ba0-7c89-2560-a2;f",
            "nih:3;532690-57e12f-e2b74b-a07c89-2560a2;f",
            "0353 2690 57e1 2fe2 b74b a07c 8925 60a2",
            "ni://example.com/sha-256-120;UyaQV-Ev4rdLoHyJJWCi?ct=x",
        ] {
            assert_eq!(KeyName::parse(text).unwrap(), short, "{text}");
        }
        assert!(short.names(&spki()));
        assert_eq!(
            KeyName::parse("nih:sha-256-32;53269057;b").unwrap(),
            KeyName::of_key(&spki(), 6).unwrap()
        );
        assert_eq!(
            KeyName::parse("nih:sha-256-120;5326-9057-e12f-e2b7-4ba0-7c89-2560-a2;0"),
            Err(NameError::CheckDigit)
        );
        assert!(matches!(
            KeyName::parse("ni:///md5;AAAA"),
            Err(NameError::Algorithm(_))
        ));
        assert_eq!(
            KeyName::parse("ni:///sha-256-120;UyaQ"),
            Err(NameError::Value)
        );
        assert!(!short.names(b"another key"));
    }

    #[test]
    fn rfc6920_8_1_hello_world() {
        let d = hash(MessageDigest::sha256(), b"Hello World!").unwrap();
        assert_eq!(
            base64url_encode(&d),
            "f4OxZX_x_FO5LcGBSKHWXfwtSx-j1ncoSt3SABJtkGk"
        );
        assert_eq!(
            base64url_decode("f4OxZX_x_FO5LcGBSKHWXfwtSx-j1ncoSt3SABJtkGk").unwrap(),
            d.to_vec()
        );
    }
}
