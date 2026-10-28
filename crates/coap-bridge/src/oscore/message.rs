//! OSCORE message protection (RFC 8613 §8): protect/unprotect a whole CoAP
//! message. The plaintext is the inner code + class-E options + payload (§5.3);
//! it is AEAD-encrypted (via [`super::aead`]) and carried as the payload of an
//! outer message that holds the class-U options + the OSCORE option (No. 9).
//!
//! Option classes per §4.1.2: most options are class E (encrypted+integrity);
//! a fixed set is class U (unprotected, kept in the outer message); class I
//! (integrity-only) options are uncommon and are passed through the AAD.
//!
//! `no_std + alloc`.

extern crate alloc;

use alloc::vec::Vec;

use crate::message::{CoapCode, CoapMessage};
use crate::option::{CoapOption, OptionNumber, OptionValue};

use super::aead::{aes_ccm_decrypt, aes_ccm_encrypt, oscore_aad, oscore_nonce};
use super::aead::{protect_request, unprotect_request};
use super::wire::OscoreOption;
use super::{OscoreError, SecurityContext};

/// The CoAP OSCORE option number (RFC 8613 §2).
pub const OSCORE_OPTION: OptionNumber = 9;

/// Class-U options (RFC 8613 §4.1.2 Table 1): kept in the outer message rather
/// than encrypted. Everything else is treated as class E.
fn is_class_u(num: OptionNumber) -> bool {
    matches!(
        num,
        3    // Uri-Host
        | 7  // Uri-Port
        | 9  // OSCORE
        | 35 // Proxy-Uri
        | 39 // Proxy-Scheme
        | 14 // Max-Age
        | 6  // Observe (outer copy)
        | 23 // Block2
        | 27 // Block1
        | 28 // Size2
        | 60 // Size1
        | 16 // Hop-Limit
        | 258 // No-Response
    )
}

/// CoAP option-list encoding (RFC 7252 §3.1), delta-coded from option number 0.
fn encode_options(opts: &[CoapOption]) -> Vec<u8> {
    let mut sorted: Vec<&CoapOption> = opts.iter().collect();
    sorted.sort_by_key(|o| o.number);
    let mut out = Vec::new();
    let mut prev = 0u16;
    for opt in sorted {
        let delta = u32::from(opt.number - prev);
        prev = opt.number;
        let val = opt.value.to_wire_bytes();
        let (dn, dx) = ext(delta);
        let (ln, lx) = ext(val.len() as u32);
        out.push((dn << 4) | (ln & 0x0F));
        out.extend_from_slice(&dx);
        out.extend_from_slice(&lx);
        out.extend_from_slice(&val);
    }
    out
}

/// Decode a CoAP option list (+ optional `0xFF` payload) — the inverse of
/// [`encode_options`]. Option values are kept opaque (raw bytes), which
/// round-trips losslessly for protect/unprotect.
fn decode_options(bytes: &[u8]) -> Result<(Vec<CoapOption>, Vec<u8>), OscoreError> {
    let mut opts = Vec::new();
    let mut prev = 0u16;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0xFF {
            return Ok((opts, bytes[i + 1..].to_vec()));
        }
        i += 1;
        let dn = (b >> 4) & 0x0F;
        let ln = b & 0x0F;
        let delta = read_ext(dn, bytes, &mut i)?;
        let len = read_ext(ln, bytes, &mut i)? as usize;
        if i + len > bytes.len() {
            return Err(OscoreError);
        }
        let num = prev
            .checked_add(u16::try_from(delta).map_err(|_| OscoreError)?)
            .ok_or(OscoreError)?;
        prev = num;
        opts.push(CoapOption::new(
            num,
            OptionValue::Opaque(bytes[i..i + len].to_vec()),
        ));
        i += len;
    }
    Ok((opts, Vec::new()))
}

fn ext(v: u32) -> (u8, Vec<u8>) {
    if v < 13 {
        (v as u8, Vec::new())
    } else if v < 269 {
        (13, alloc::vec![(v - 13) as u8])
    } else {
        (14, ((v - 269) as u16).to_be_bytes().to_vec())
    }
}

fn read_ext(nibble: u8, bytes: &[u8], i: &mut usize) -> Result<u32, OscoreError> {
    match nibble {
        0..=12 => Ok(u32::from(nibble)),
        13 => {
            let b = *bytes.get(*i).ok_or(OscoreError)?;
            *i += 1;
            Ok(u32::from(b) + 13)
        }
        14 => {
            if *i + 2 > bytes.len() {
                return Err(OscoreError);
            }
            let v = u16::from_be_bytes([bytes[*i], bytes[*i + 1]]);
            *i += 2;
            Ok(u32::from(v) + 269)
        }
        _ => Err(OscoreError), // 15 reserved
    }
}

/// Build the OSCORE plaintext (RFC 8613 §5.3): inner code byte, class-E options,
/// and (if any) `0xFF` + payload.
fn build_plaintext(inner: &CoapMessage) -> Vec<u8> {
    let mut pt = Vec::new();
    pt.push((inner.code.class << 5) | inner.code.detail);
    let class_e: Vec<CoapOption> = inner
        .options
        .iter()
        .filter(|o| !is_class_u(o.number))
        .cloned()
        .collect();
    pt.extend_from_slice(&encode_options(&class_e));
    if !inner.payload.is_empty() {
        pt.push(0xFF);
        pt.extend_from_slice(&inner.payload);
    }
    pt
}

/// Protect a request (RFC 8613 §8.1): encrypt the inner message and produce the
/// outer message (class-U options + the OSCORE option with the Partial IV +
/// Sender ID + optional ID Context, ciphertext payload). The outer code is 0.02
/// (POST) — or 0.05 (FETCH) when the request carries an Observe option
/// (§4.1.3.5). `version`/`type`/`mid`/`token` are copied from `inner` for the
/// transport layer.
///
/// # Errors
/// `Err` on AEAD failure.
pub fn protect_request_message(
    ctx: &SecurityContext,
    sender_id: &[u8],
    kid_context: Option<&[u8]>,
    partial_iv: &[u8],
    inner: &CoapMessage,
) -> Result<CoapMessage, OscoreError> {
    let plaintext = build_plaintext(inner);
    let ciphertext = protect_request(ctx, sender_id, partial_iv, &[], &plaintext)?;
    let mut options: Vec<CoapOption> = inner
        .options
        .iter()
        .filter(|o| is_class_u(o.number) && o.number != OSCORE_OPTION)
        .cloned()
        .collect();
    let osc = OscoreOption {
        partial_iv: partial_iv.to_vec(),
        kid: Some(sender_id.to_vec()),
        kid_context: kid_context.map(<[u8]>::to_vec),
    };
    options.push(CoapOption::new(
        OSCORE_OPTION,
        OptionValue::Opaque(osc.encode()),
    ));
    // §4.1.3.5: with an outer Observe option the outer code is 0.05 (FETCH).
    let outer_code = if options.iter().any(|o| o.number == 6) {
        CoapCode::new(0, 5)
    } else {
        CoapCode::new(0, 2)
    };
    Ok(CoapMessage {
        version: inner.version,
        message_type: inner.message_type,
        code: outer_code,
        message_id: inner.message_id,
        token: inner.token.clone(),
        options,
        payload: ciphertext,
    })
}

/// Protect a response (RFC 8613 §8.3): encrypt the inner response under the
/// responder's Sender Key. The AAD binds the response to the request's `kid` +
/// Partial IV. Without `own_piv` the request's nonce is reused (Appendix C.7 —
/// safe because the two directions use different keys); with `own_piv` the
/// nonce is built from the responder's own Sender ID + Partial IV (Appendix
/// C.8 — required for Observe notifications, §4.1.3.5.2). The outer code is
/// 2.04 (Changed) — or 2.05 (Content) when an Observe option is carried
/// (§4.1.3.5).
///
/// # Errors
/// `Err` on AEAD failure.
pub fn protect_response_message(
    ctx: &SecurityContext,
    responder_id: &[u8],
    request_kid: &[u8],
    request_piv: &[u8],
    own_piv: Option<&[u8]>,
    inner: &CoapMessage,
) -> Result<CoapMessage, OscoreError> {
    let plaintext = build_plaintext(inner);
    let aad = oscore_aad(ctx.alg, request_kid, request_piv, &[]);
    let nonce = match own_piv {
        Some(piv) => oscore_nonce(&ctx.common_iv, responder_id, piv),
        None => oscore_nonce(&ctx.common_iv, request_kid, request_piv),
    };
    let ciphertext = aes_ccm_encrypt(&ctx.sender_key, &nonce, &plaintext, &aad)?;
    let mut options: Vec<CoapOption> = inner
        .options
        .iter()
        .filter(|o| is_class_u(o.number) && o.number != OSCORE_OPTION)
        .cloned()
        .collect();
    // §8.3: a response carries no `kid`; the Partial IV only when it is our own.
    let osc = OscoreOption {
        partial_iv: own_piv.unwrap_or_default().to_vec(),
        kid: None,
        kid_context: None,
    };
    options.push(CoapOption::new(
        OSCORE_OPTION,
        OptionValue::Opaque(osc.encode()),
    ));
    // §4.1.3.5: with an outer Observe option the outer code is 2.05 (Content).
    let outer_code = if options.iter().any(|o| o.number == 6) {
        CoapCode::new(2, 5)
    } else {
        CoapCode::new(2, 4)
    };
    Ok(CoapMessage {
        version: inner.version,
        message_type: inner.message_type,
        code: outer_code,
        message_id: inner.message_id,
        token: inner.token.clone(),
        options,
        payload: ciphertext,
    })
}

/// Unprotect a response (RFC 8613 §8.4): decode the OSCORE option, rebuild the
/// nonce (the request's if the response has no Partial IV, else the responder's
/// own), verify + decrypt under the Recipient Key, and reconstruct the inner
/// response. `responder_id` is the peer's Sender ID (used only when the
/// response carries its own Partial IV).
///
/// # Errors
/// `Err` if the OSCORE option is missing/malformed or the tag fails.
pub fn unprotect_response_message(
    ctx: &SecurityContext,
    responder_id: &[u8],
    request_kid: &[u8],
    request_piv: &[u8],
    outer: &CoapMessage,
) -> Result<CoapMessage, OscoreError> {
    let osc_opt = outer
        .options
        .iter()
        .find(|o| o.number == OSCORE_OPTION)
        .ok_or(OscoreError)?;
    let osc = OscoreOption::decode(&osc_opt.value.to_wire_bytes())?;
    let nonce = if osc.partial_iv.is_empty() {
        oscore_nonce(&ctx.common_iv, request_kid, request_piv)
    } else {
        oscore_nonce(&ctx.common_iv, responder_id, &osc.partial_iv)
    };
    let aad = oscore_aad(ctx.alg, request_kid, request_piv, &[]);
    let plaintext = aes_ccm_decrypt(&ctx.recipient_key, &nonce, &outer.payload, &aad)?;
    if plaintext.is_empty() {
        return Err(OscoreError);
    }
    let code = CoapCode::new(plaintext[0] >> 5, plaintext[0] & 0x1F);
    let (class_e, payload) = decode_options(&plaintext[1..])?;
    let mut options: Vec<CoapOption> = outer
        .options
        .iter()
        .filter(|o| o.number != OSCORE_OPTION && is_class_u(o.number))
        .cloned()
        .collect();
    options.extend(class_e);
    options.sort_by_key(|o| o.number);
    Ok(CoapMessage {
        version: outer.version,
        message_type: outer.message_type,
        code,
        message_id: outer.message_id,
        token: outer.token.clone(),
        options,
        payload,
    })
}

/// Unprotect a request (RFC 8613 §8.2): decode the OSCORE option, AEAD-decrypt
/// the payload, and reconstruct the inner message (inner code + class-E options
/// from the plaintext + the carried-over class-U options).
///
/// # Errors
/// `Err` if the OSCORE option is missing/malformed or the tag fails (replay/tamper).
pub fn unprotect_request_message(
    ctx: &SecurityContext,
    outer: &CoapMessage,
) -> Result<CoapMessage, OscoreError> {
    let osc_opt = outer
        .options
        .iter()
        .find(|o| o.number == OSCORE_OPTION)
        .ok_or(OscoreError)?;
    let osc = OscoreOption::decode(&osc_opt.value.to_wire_bytes())?;
    let kid = osc.kid.clone().unwrap_or_default();

    let plaintext = unprotect_request(ctx, &kid, &osc.partial_iv, &[], &outer.payload)?;
    if plaintext.is_empty() {
        return Err(OscoreError);
    }
    let code = CoapCode::new(plaintext[0] >> 5, plaintext[0] & 0x1F);
    let (class_e, payload) = decode_options(&plaintext[1..])?;

    // outer class-U options (minus OSCORE) + inner class-E options.
    let mut options: Vec<CoapOption> = outer
        .options
        .iter()
        .filter(|o| o.number != OSCORE_OPTION && is_class_u(o.number))
        .cloned()
        .collect();
    options.extend(class_e);
    options.sort_by_key(|o| o.number);

    Ok(CoapMessage {
        version: outer.version,
        message_type: outer.message_type,
        code,
        message_id: outer.message_id,
        token: outer.token.clone(),
        options,
        payload,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::message::MessageType;
    use crate::option::numbers;
    use crate::oscore::AeadAlgorithm;

    fn ctx_pair() -> (SecurityContext, SecurityContext) {
        let ms = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10,
        ];
        let salt = [0x9e, 0x7c, 0xa9, 0x22, 0x23, 0x78, 0x63, 0x40];
        let client = SecurityContext::derive(
            &ms,
            &salt,
            &[],
            &[0x01],
            None,
            AeadAlgorithm::AesCcm16_64_128,
        );
        let server = SecurityContext::derive(
            &ms,
            &salt,
            &[0x01],
            &[],
            None,
            AeadAlgorithm::AesCcm16_64_128,
        );
        (client, server)
    }

    #[test]
    fn protect_unprotect_get_roundtrip() {
        let (client, server) = ctx_pair();
        let inner = CoapMessage {
            version: 1,
            message_type: MessageType::Confirmable,
            code: CoapCode::new(0, 1), // GET
            message_id: 0x1234,
            token: alloc::vec![0xAB, 0xCD],
            options: alloc::vec![
                CoapOption::uri_path("temperature"),
                CoapOption::new(numbers::URI_HOST, OptionValue::String("example.com".into())), // class U
            ],
            payload: Vec::new(),
        };
        let outer = protect_request_message(&client, &[], None, &[0x14], &inner).unwrap();
        // Outer is a POST carrying the OSCORE option; payload is the ciphertext.
        assert_eq!(outer.code, CoapCode::new(0, 2));
        assert!(outer.options.iter().any(|o| o.number == OSCORE_OPTION));
        assert!(outer.options.iter().any(|o| o.number == numbers::URI_HOST)); // class-U kept outer
        assert!(!outer.payload.is_empty());

        let back = server.clone();
        let recovered = unprotect_request_message(&back, &outer).unwrap();
        assert_eq!(recovered.code, CoapCode::new(0, 1)); // inner GET recovered
        assert_eq!(recovered.token, alloc::vec![0xAB, 0xCD]);
        // class-E Uri-Path recovered (as opaque bytes == "temperature").
        let up = recovered
            .options
            .iter()
            .find(|o| o.number == numbers::URI_PATH)
            .unwrap();
        assert_eq!(up.value.to_wire_bytes(), b"temperature");
        // class-U Uri-Host carried through.
        assert!(
            recovered
                .options
                .iter()
                .any(|o| o.number == numbers::URI_HOST)
        );
    }

    #[test]
    fn protect_unprotect_with_payload() {
        let (client, server) = ctx_pair();
        let inner = CoapMessage {
            version: 1,
            message_type: MessageType::NonConfirmable,
            code: CoapCode::new(0, 2), // POST
            message_id: 1,
            token: alloc::vec![0x01],
            options: alloc::vec![CoapOption::new(
                numbers::CONTENT_FORMAT,
                OptionValue::Uint(0)
            )],
            payload: b"hello world".to_vec(),
        };
        let outer = protect_request_message(&client, &[], None, &[0x07], &inner).unwrap();
        let recovered = unprotect_request_message(&server.clone(), &outer).unwrap();
        assert_eq!(recovered.payload, b"hello world");
        assert_eq!(recovered.code, CoapCode::new(0, 2));
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let (client, server) = ctx_pair();
        let inner = CoapMessage {
            version: 1,
            message_type: MessageType::Confirmable,
            code: CoapCode::new(0, 1),
            message_id: 1,
            token: Vec::new(),
            options: Vec::new(),
            payload: b"x".to_vec(),
        };
        let mut outer = protect_request_message(&client, &[], None, &[0x01], &inner).unwrap();
        let n = outer.payload.len();
        outer.payload[n - 1] ^= 0x01; // flip a tag byte
        assert!(unprotect_request_message(&server.clone(), &outer).is_err());
    }

    #[test]
    fn protect_unprotect_response_roundtrip_without_piv() {
        let (client, server) = ctx_pair();
        let inner = CoapMessage {
            version: 1,
            message_type: MessageType::Acknowledgement,
            code: CoapCode::new(2, 5), // 2.05 Content
            message_id: 0x1234,
            token: alloc::vec![0xAB],
            options: alloc::vec![CoapOption::new(
                numbers::CONTENT_FORMAT,
                OptionValue::Uint(0)
            )],
            payload: b"Hello World!".to_vec(),
        };
        // Server answers the request kid=[] (client Sender ID) piv=0x14.
        let outer = protect_response_message(&server, &[0x01], &[], &[0x14], None, &inner).unwrap();
        assert_eq!(outer.code, CoapCode::new(2, 4)); // outer 2.04 Changed (§4.2)
        let osc = outer
            .options
            .iter()
            .find(|o| o.number == OSCORE_OPTION)
            .unwrap();
        assert!(osc.value.to_wire_bytes().is_empty()); // no kid, no PIV (§8.3)
        let back = unprotect_response_message(&client, &[0x01], &[], &[0x14], &outer).unwrap();
        assert_eq!(back.code, CoapCode::new(2, 5));
        assert_eq!(back.payload, b"Hello World!");
    }

    #[test]
    fn protect_unprotect_response_roundtrip_with_own_piv() {
        let (client, server) = ctx_pair();
        let inner = CoapMessage {
            version: 1,
            message_type: MessageType::NonConfirmable,
            code: CoapCode::new(2, 5),
            message_id: 7,
            token: alloc::vec![0x01],
            options: Vec::new(),
            payload: b"notify".to_vec(),
        };
        let outer = protect_response_message(&server, &[0x01], &[], &[0x14], Some(&[0x03]), &inner)
            .unwrap();
        let back = unprotect_response_message(&client, &[0x01], &[], &[0x14], &outer).unwrap();
        assert_eq!(back.payload, b"notify");
        // Wrong request PIV in the AAD -> verification fails.
        assert!(unprotect_response_message(&client, &[0x01], &[], &[0x15], &outer).is_err());
    }

    #[test]
    fn observe_notification_gets_outer_2_05() {
        let (_client, server) = ctx_pair();
        let inner = CoapMessage {
            version: 1,
            message_type: MessageType::NonConfirmable,
            code: CoapCode::new(2, 5),
            message_id: 8,
            token: alloc::vec![0x01],
            options: alloc::vec![CoapOption::observe(3)],
            payload: b"n".to_vec(),
        };
        let outer = protect_response_message(&server, &[0x01], &[], &[0x14], Some(&[0x04]), &inner)
            .unwrap();
        assert_eq!(outer.code, CoapCode::new(2, 5)); // §4.1.3.5 outer Content
        assert!(outer.options.iter().any(|o| o.number == numbers::OBSERVE));
    }
}
