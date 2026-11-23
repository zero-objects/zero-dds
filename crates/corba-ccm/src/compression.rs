// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 1 §18 — the `Compression` module.
//!
//! `Compressor`, `CompressorFactory` and `CompressionManager` are local
//! interfaces (§18.1): [`Compressor`] and [`CompressorFactory`] are
//! traits a user implements for a compressor of their own;
//! [`CompressionManager`] is the registry the ORB hands out as the
//! initial reference `"CompressionManager"` (§18.4,
//! [`crate::orb_core::orb_init`]). The zlib compressor is provided by
//! default (§18.1), as is gzip; both run on `flate2`. A
//! [`CompressionManager::with_default_factories`] has these two; any
//! other [`CompressorId`] — the ids of §18.5 or a vendor id — is served
//! by a factory registered with [`CompressionManager::register_factory`].
//!
//! The data types, the exceptions and the two traits need only `alloc`;
//! [`CompressionManager`] (a `Mutex`) and the built-in compressors
//! (`flate2`) need the `std` feature.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use alloc::{collections::BTreeMap, sync::Weak};
#[cfg(feature = "std")]
use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "std")]
use std::io::{Read, Write};
#[cfg(feature = "std")]
use std::sync::Mutex;

use zerodds_corba_rt::{CompletionStatus, SystemException, SystemExceptionKind};

/// `Compression::CompressorId` (§18.5), `Compression::CompressionLevel`
/// (0 no compression, 1 low, 9 the highest, §18.2.5),
/// `Compression::CompressionRatio`, `Compression::CompressorIdLevel` and
/// `Compression::CompressorIdLevelList`: the types ZIOP carries on the wire
/// (CORBA 3.3 Part 2 §12), defined with it in `zerodds-corba-giop`.
pub use zerodds_corba_giop::ziop::{
    CompressionLevel, CompressionRatio, CompressorId, CompressorIdLevel, CompressorIdLevelList,
};
/// `COMPRESSORID_NONE` — no compression.
pub const COMPRESSORID_NONE: CompressorId = 0;
/// `COMPRESSORID_GZIP` — gzip (RFC 1952).
pub const COMPRESSORID_GZIP: CompressorId = 1;
/// `COMPRESSORID_PKZIP`.
pub const COMPRESSORID_PKZIP: CompressorId = 2;
/// `COMPRESSORID_BZIP2`.
pub const COMPRESSORID_BZIP2: CompressorId = 3;
/// `COMPRESSORID_ZLIB` — zlib (RFC 1950).
pub const COMPRESSORID_ZLIB: CompressorId = 4;
/// `COMPRESSORID_LZMA`.
pub const COMPRESSORID_LZMA: CompressorId = 5;
/// `COMPRESSORID_LZO`.
pub const COMPRESSORID_LZO: CompressorId = 6;
/// `COMPRESSORID_RZIP`.
pub const COMPRESSORID_RZIP: CompressorId = 7;
/// `COMPRESSORID_7X`.
pub const COMPRESSORID_7X: CompressorId = 8;
/// `COMPRESSORID_XAR`.
pub const COMPRESSORID_XAR: CompressorId = 9;

/// `Compression::Buffer` (`CORBA::OctetSeq`).
pub type Buffer = Vec<u8>;

/// The highest compression level (§18.2.5).
pub const MAX_COMPRESSION_LEVEL: CompressionLevel = 9;

/// Repository id of [`CompressionException`].
pub const COMPRESSION_EXCEPTION_ID: &str = "IDL:omg.org/Compression/CompressionException:1.0";
/// Repository id of [`FactoryAlreadyRegistered`].
pub const FACTORY_ALREADY_REGISTERED_ID: &str =
    "IDL:omg.org/Compression/FactoryAlreadyRegistered:1.0";
/// Repository id of [`UnknownCompressorId`].
pub const UNKNOWN_COMPRESSOR_ID_ID: &str = "IDL:omg.org/Compression/UnknownCompressorId:1.0";

/// `Compression::CompressionException` (§18.2.1): `compress` or
/// `decompress` failed. `reason` is the compressor's own code; the
/// built-in compressors use the zlib return codes ([`Z_DATA_ERROR`],
/// [`Z_BUF_ERROR`], [`Z_STREAM_ERROR`]), `description` the text of the
/// underlying error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionException {
    /// The compressor's reason code.
    pub reason: i32,
    /// The description of the underlying algorithm.
    pub description: String,
}

/// zlib `Z_STREAM_ERROR`: the compressor could not run.
pub const Z_STREAM_ERROR: i32 = -2;
/// zlib `Z_DATA_ERROR`: the input is not valid compressed data.
pub const Z_DATA_ERROR: i32 = -3;
/// zlib `Z_BUF_ERROR`: the output does not fit; for `decompress`, the
/// decompressed data exceeds the compressor's limit.
pub const Z_BUF_ERROR: i32 = -5;

impl core::fmt::Display for CompressionException {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "CompressionException(reason {}): {}",
            self.reason, self.description
        )
    }
}

impl core::error::Error for CompressionException {}

/// `Compression::FactoryAlreadyRegistered` (§18.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactoryAlreadyRegistered;

/// `Compression::UnknownCompressorId` (§18.4.2 to §18.4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownCompressorId;

/// What [`CompressionManager::get_compressor`] raises: the user
/// exception of the IDL, or `BAD_PARAM` minor 44 for a level above 9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetCompressorError {
    /// `UnknownCompressorId`.
    UnknownCompressorId,
    /// A system exception.
    System(SystemException),
}

impl From<SystemException> for GetCompressorError {
    fn from(e: SystemException) -> Self {
        Self::System(e)
    }
}

/// `BAD_PARAM` minor 44 (§18.3.2, §18.4.4): a compression level above 9.
#[must_use]
pub fn bad_compression_level() -> SystemException {
    SystemException::omg(SystemExceptionKind::BadParam, 44, CompletionStatus::No)
}

/// The level check of §18.3.2 for an implementation of
/// [`CompressorFactory::get_compressor`].
///
/// # Errors
/// `BAD_PARAM` minor 44 for a level above 9.
pub fn check_level(level: CompressionLevel) -> Result<(), SystemException> {
    if level > MAX_COMPRESSION_LEVEL {
        Err(bad_compression_level())
    } else {
        Ok(())
    }
}

/// `Compression::Compressor` (§18.2).
pub trait Compressor: Send + Sync {
    /// §18.2.2 — compresses `source` into `target` (its previous content
    /// is replaced).
    ///
    /// # Errors
    /// [`CompressionException`] when compression fails.
    fn compress(&self, source: &[u8], target: &mut Buffer) -> Result<(), CompressionException>;

    /// §18.2.3 — decompresses `source` into `target` (its previous
    /// content is replaced).
    ///
    /// # Errors
    /// [`CompressionException`] when decompression fails.
    fn decompress(&self, source: &[u8], target: &mut Buffer) -> Result<(), CompressionException>;

    /// §18.2.4 — the factory that created this compressor; `None` once
    /// that factory is gone.
    fn compressor_factory(&self) -> Option<Arc<dyn CompressorFactory>>;

    /// §18.2.5 — the level this compressor applies.
    fn compression_level(&self) -> CompressionLevel;

    /// §18.2.6 — total octets `compress` wrote (its `target`s).
    fn compressed_bytes(&self) -> u64;

    /// §18.2.7 — total octets `compress` read (its `source`s).
    fn uncompressed_bytes(&self) -> u64;

    /// §18.2.8 — `compressed_bytes / uncompressed_bytes`; 0 before the
    /// first `compress`.
    fn compression_ratio(&self) -> CompressionRatio {
        ratio(self.compressed_bytes(), self.uncompressed_bytes())
    }
}

/// `compressed / uncompressed` as §18.2.8 defines it, 0 when nothing
/// was compressed yet.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn ratio(compressed: u64, uncompressed: u64) -> CompressionRatio {
    if uncompressed == 0 {
        0.0
    } else {
        (compressed as f64 / uncompressed as f64) as f32
    }
}

/// `Compression::CompressorFactory` (§18.3).
pub trait CompressorFactory: Send + Sync {
    /// §18.3.1 — the algorithm of every compressor of this factory.
    fn compressor_id(&self) -> CompressorId;

    /// §18.3.2 — the compressor for `level`; the same instance for the
    /// same level (the factory keeps it).
    ///
    /// # Errors
    /// `BAD_PARAM` minor 44 for a level above 9 ([`check_level`]).
    fn get_compressor(
        &self,
        compression_level: CompressionLevel,
    ) -> Result<Arc<dyn Compressor>, SystemException>;
}

/// `Compression::CompressorFactorySeq`.
pub type CompressorFactorySeq = Vec<Arc<dyn CompressorFactory>>;

/// `Compression::CompressionManager` (§18.4): the factories of an ORB,
/// one per [`CompressorId`].
#[cfg(feature = "std")]
#[derive(Default)]
pub struct CompressionManager {
    factories: Mutex<BTreeMap<CompressorId, Arc<dyn CompressorFactory>>>,
}

#[cfg(feature = "std")]
impl core::fmt::Debug for CompressionManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let ids: Vec<CompressorId> = self
            .factories
            .lock()
            .map(|g| g.keys().copied().collect())
            .unwrap_or_default();
        f.debug_struct("CompressionManager")
            .field("compressor_ids", &ids)
            .finish()
    }
}

#[cfg(feature = "std")]
impl CompressionManager {
    /// A manager without factories.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with the built-in zlib (§18.1: "must be provided by
    /// default") and gzip factories.
    #[must_use]
    pub fn with_default_factories() -> Self {
        let m = Self::new();
        let zlib: Arc<dyn CompressorFactory> = FlateCompressorFactory::zlib();
        let gzip: Arc<dyn CompressorFactory> = FlateCompressorFactory::gzip();
        if let Ok(mut g) = m.factories.lock() {
            g.insert(COMPRESSORID_ZLIB, zlib);
            g.insert(COMPRESSORID_GZIP, gzip);
        }
        m
    }

    /// §18.4.1 — registers `compressor_factory` under its
    /// `compressor_id`.
    ///
    /// # Errors
    /// [`FactoryAlreadyRegistered`] when a factory has that id.
    pub fn register_factory(
        &self,
        compressor_factory: Arc<dyn CompressorFactory>,
    ) -> Result<(), FactoryAlreadyRegistered> {
        let mut g = self
            .factories
            .lock()
            .map_err(|_| FactoryAlreadyRegistered)?;
        let id = compressor_factory.compressor_id();
        if g.contains_key(&id) {
            return Err(FactoryAlreadyRegistered);
        }
        g.insert(id, compressor_factory);
        Ok(())
    }

    /// §18.4.2 — removes the factory with `compressor_id`.
    ///
    /// # Errors
    /// [`UnknownCompressorId`] when none has it.
    pub fn unregister_factory(
        &self,
        compressor_id: CompressorId,
    ) -> Result<(), UnknownCompressorId> {
        let mut g = self.factories.lock().map_err(|_| UnknownCompressorId)?;
        g.remove(&compressor_id)
            .map(|_| ())
            .ok_or(UnknownCompressorId)
    }

    /// §18.4.3 — the factory with `compressor_id`.
    ///
    /// # Errors
    /// [`UnknownCompressorId`] when none has it.
    pub fn get_factory(
        &self,
        compressor_id: CompressorId,
    ) -> Result<Arc<dyn CompressorFactory>, UnknownCompressorId> {
        let g = self.factories.lock().map_err(|_| UnknownCompressorId)?;
        g.get(&compressor_id).cloned().ok_or(UnknownCompressorId)
    }

    /// §18.4.4 — the compressor for `compression_level` of the factory
    /// with `compressor_id`; the same instance for the same id and
    /// level.
    ///
    /// # Errors
    /// `BAD_PARAM` minor 44 for a level above 9, else
    /// [`GetCompressorError::UnknownCompressorId`] when no factory has
    /// the id.
    pub fn get_compressor(
        &self,
        compressor_id: CompressorId,
        compression_level: CompressionLevel,
    ) -> Result<Arc<dyn Compressor>, GetCompressorError> {
        check_level(compression_level)?;
        let factory = self
            .get_factory(compressor_id)
            .map_err(|_| GetCompressorError::UnknownCompressorId)?;
        Ok(factory.get_compressor(compression_level)?)
    }

    /// §18.4.5 — every registered factory, by ascending id.
    #[must_use]
    pub fn get_factories(&self) -> CompressorFactorySeq {
        self.factories
            .lock()
            .map(|g| g.values().cloned().collect())
            .unwrap_or_default()
    }
}

/// The compressors of the manager are the ones ZIOP uses (CORBA 3.3
/// Part 2 §12): a message is compressed with the compressor of the
/// agreed `CompressorIdLevel` and decompressed with the compressor its
/// `CompressionData` names.
#[cfg(feature = "std")]
impl zerodds_corba_giop::ziop::ZiopCodec for CompressionManager {
    fn supports(&self, compressor: CompressorId) -> bool {
        self.get_factory(compressor).is_ok()
    }

    fn compress(
        &self,
        compressor: CompressorIdLevel,
        body: &[u8],
    ) -> Result<Vec<u8>, zerodds_corba_giop::ziop::ZiopError> {
        let c = self
            .get_compressor(compressor.compressor_id, compressor.compression_level)
            .map_err(|e| ziop_error(compressor.compressor_id, &e))?;
        let mut out = Vec::new();
        c.compress(body, &mut out).map_err(ziop_failed)?;
        Ok(out)
    }

    fn decompress(
        &self,
        compressor: CompressorId,
        data: &[u8],
        _original_length: u32,
    ) -> Result<Vec<u8>, zerodds_corba_giop::ziop::ZiopError> {
        // The level is a property of compressing; any compressor of the
        // factory reads the data.
        let c = self
            .get_compressor(compressor, 0)
            .map_err(|e| ziop_error(compressor, &e))?;
        let mut out = Vec::new();
        c.decompress(data, &mut out).map_err(ziop_failed)?;
        Ok(out)
    }
}

#[cfg(feature = "std")]
fn ziop_error(id: CompressorId, e: &GetCompressorError) -> zerodds_corba_giop::ziop::ZiopError {
    match e {
        GetCompressorError::UnknownCompressorId => {
            zerodds_corba_giop::ziop::ZiopError::UnknownCompressor(id)
        }
        GetCompressorError::System(s) => zerodds_corba_giop::ziop::ZiopError::Failed {
            reason: Z_STREAM_ERROR,
            description: alloc::format!("{s:?}"),
        },
    }
}

#[cfg(feature = "std")]
#[allow(clippy::needless_pass_by_value)]
fn ziop_failed(e: CompressionException) -> zerodds_corba_giop::ziop::ZiopError {
    zerodds_corba_giop::ziop::ZiopError::Failed {
        reason: e.reason,
        description: e.description,
    }
}

/// The decompressed size a built-in compressor accepts by default: the
/// largest GIOP message `corba-iiop` reads (16 MiB). A small compressed
/// input can expand to gigabytes; the limit stops `decompress` there
/// with [`Z_BUF_ERROR`] instead of allocating without bound.
#[cfg(feature = "std")]
pub const DEFAULT_MAX_DECOMPRESSED_BYTES: usize = 16 * 1024 * 1024;

/// The format of a built-in compressor.
#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlateFormat {
    /// zlib (RFC 1950), `COMPRESSORID_ZLIB`.
    Zlib,
    /// gzip (RFC 1952), `COMPRESSORID_GZIP`.
    Gzip,
}

#[cfg(feature = "std")]
impl FlateFormat {
    /// The [`CompressorId`] of the format.
    #[must_use]
    pub const fn compressor_id(self) -> CompressorId {
        match self {
            Self::Zlib => COMPRESSORID_ZLIB,
            Self::Gzip => COMPRESSORID_GZIP,
        }
    }
}

/// The built-in zlib and gzip factories (`flate2`). Level 0 writes the
/// format with stored (uncompressed) blocks, 1 to 9 the deflate levels.
#[cfg(feature = "std")]
pub struct FlateCompressorFactory {
    format: FlateFormat,
    max_decompressed: usize,
    me: Weak<FlateCompressorFactory>,
    compressors: Mutex<BTreeMap<CompressionLevel, Arc<FlateCompressor>>>,
}

#[cfg(feature = "std")]
impl core::fmt::Debug for FlateCompressorFactory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FlateCompressorFactory")
            .field("format", &self.format)
            .field("max_decompressed", &self.max_decompressed)
            .finish()
    }
}

#[cfg(feature = "std")]
impl FlateCompressorFactory {
    /// The zlib factory with [`DEFAULT_MAX_DECOMPRESSED_BYTES`].
    #[must_use]
    pub fn zlib() -> Arc<Self> {
        Self::new(FlateFormat::Zlib, DEFAULT_MAX_DECOMPRESSED_BYTES)
    }

    /// The gzip factory with [`DEFAULT_MAX_DECOMPRESSED_BYTES`].
    #[must_use]
    pub fn gzip() -> Arc<Self> {
        Self::new(FlateFormat::Gzip, DEFAULT_MAX_DECOMPRESSED_BYTES)
    }

    /// A factory for `format` whose compressors decompress at most
    /// `max_decompressed` octets.
    #[must_use]
    pub fn new(format: FlateFormat, max_decompressed: usize) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            format,
            max_decompressed,
            me: me.clone(),
            compressors: Mutex::new(BTreeMap::new()),
        })
    }

    /// The decompressed size the compressors of this factory accept.
    #[must_use]
    pub fn max_decompressed(&self) -> usize {
        self.max_decompressed
    }
}

#[cfg(feature = "std")]
impl CompressorFactory for FlateCompressorFactory {
    fn compressor_id(&self) -> CompressorId {
        self.format.compressor_id()
    }

    fn get_compressor(
        &self,
        compression_level: CompressionLevel,
    ) -> Result<Arc<dyn Compressor>, SystemException> {
        check_level(compression_level)?;
        let mut g = self.compressors.lock().map_err(|_| {
            SystemException::new(SystemExceptionKind::Internal, 0, CompletionStatus::No)
        })?;
        let c = g.entry(compression_level).or_insert_with(|| {
            Arc::new(FlateCompressor {
                format: self.format,
                level: compression_level,
                max_decompressed: self.max_decompressed,
                factory: self.me.clone(),
                compressed: AtomicU64::new(0),
                uncompressed: AtomicU64::new(0),
            })
        });
        Ok(Arc::clone(c) as Arc<dyn Compressor>)
    }
}

/// A compressor of a [`FlateCompressorFactory`].
#[cfg(feature = "std")]
pub struct FlateCompressor {
    format: FlateFormat,
    level: CompressionLevel,
    max_decompressed: usize,
    factory: Weak<FlateCompressorFactory>,
    compressed: AtomicU64,
    uncompressed: AtomicU64,
}

#[cfg(feature = "std")]
impl core::fmt::Debug for FlateCompressor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FlateCompressor")
            .field("format", &self.format)
            .field("level", &self.level)
            .finish()
    }
}

#[cfg(feature = "std")]
fn io_exception(reason: i32, e: &std::io::Error) -> CompressionException {
    CompressionException {
        reason,
        description: alloc::format!("{e}"),
    }
}

#[cfg(feature = "std")]
impl FlateCompressor {
    fn encode(&self, source: &[u8]) -> std::io::Result<Vec<u8>> {
        let level = flate2::Compression::new(u32::from(self.level));
        match self.format {
            FlateFormat::Zlib => {
                let mut e = flate2::write::ZlibEncoder::new(Vec::new(), level);
                e.write_all(source)?;
                e.finish()
            }
            FlateFormat::Gzip => {
                let mut e = flate2::write::GzEncoder::new(Vec::new(), level);
                e.write_all(source)?;
                e.finish()
            }
        }
    }

    /// Reads at most `max_decompressed + 1` octets so an oversized
    /// result is seen without being allocated in full.
    fn decode(&self, source: &[u8]) -> Result<Vec<u8>, CompressionException> {
        let limit = self.max_decompressed as u64 + 1;
        let mut out = Vec::new();
        let read = match self.format {
            FlateFormat::Zlib => flate2::read::ZlibDecoder::new(source)
                .take(limit)
                .read_to_end(&mut out),
            FlateFormat::Gzip => flate2::read::GzDecoder::new(source)
                .take(limit)
                .read_to_end(&mut out),
        };
        read.map_err(|e| io_exception(Z_DATA_ERROR, &e))?;
        if out.len() > self.max_decompressed {
            return Err(CompressionException {
                reason: Z_BUF_ERROR,
                description: alloc::format!(
                    "decompressed data exceeds {} octets",
                    self.max_decompressed
                ),
            });
        }
        Ok(out)
    }
}

#[cfg(feature = "std")]
impl Compressor for FlateCompressor {
    fn compress(&self, source: &[u8], target: &mut Buffer) -> Result<(), CompressionException> {
        let out = self
            .encode(source)
            .map_err(|e| io_exception(Z_STREAM_ERROR, &e))?;
        self.uncompressed
            .fetch_add(source.len() as u64, Ordering::Relaxed);
        self.compressed
            .fetch_add(out.len() as u64, Ordering::Relaxed);
        *target = out;
        Ok(())
    }

    fn decompress(&self, source: &[u8], target: &mut Buffer) -> Result<(), CompressionException> {
        *target = self.decode(source)?;
        Ok(())
    }

    fn compressor_factory(&self) -> Option<Arc<dyn CompressorFactory>> {
        self.factory
            .upgrade()
            .map(|f| f as Arc<dyn CompressorFactory>)
    }

    fn compression_level(&self) -> CompressionLevel {
        self.level
    }

    fn compressed_bytes(&self) -> u64 {
        self.compressed.load(Ordering::Relaxed)
    }

    fn uncompressed_bytes(&self) -> u64 {
        self.uncompressed.load(Ordering::Relaxed)
    }
}

#[cfg(all(test, feature = "std"))]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn corba_p1_18_5_compressor_ids_match_the_idl() {
        assert_eq!(
            [
                COMPRESSORID_NONE,
                COMPRESSORID_GZIP,
                COMPRESSORID_PKZIP,
                COMPRESSORID_BZIP2,
                COMPRESSORID_ZLIB,
                COMPRESSORID_LZMA,
                COMPRESSORID_LZO,
                COMPRESSORID_RZIP,
                COMPRESSORID_7X,
                COMPRESSORID_XAR,
            ],
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
        assert_eq!(
            COMPRESSION_EXCEPTION_ID,
            "IDL:omg.org/Compression/CompressionException:1.0"
        );
    }

    fn sample() -> Vec<u8> {
        b"the quick brown fox jumps over the lazy dog ".repeat(64)
    }

    #[test]
    fn corba_p1_18_1_zlib_is_provided_by_default() {
        let m = CompressionManager::with_default_factories();
        let ids: Vec<CompressorId> = m
            .get_factories()
            .iter()
            .map(|f| f.compressor_id())
            .collect();
        assert_eq!(ids, [COMPRESSORID_GZIP, COMPRESSORID_ZLIB]);
        let c = m.get_compressor(COMPRESSORID_ZLIB, 6).unwrap();
        let mut packed = Vec::new();
        c.compress(&sample(), &mut packed).unwrap();
        // RFC 1950: CMF 0x78 (deflate, 32 KiB window), FCHECK makes
        // CMF*256+FLG a multiple of 31.
        assert_eq!(packed[0], 0x78);
        assert_eq!((u16::from(packed[0]) << 8 | u16::from(packed[1])) % 31, 0);
        let mut back = vec![1, 2, 3];
        c.decompress(&packed, &mut back).unwrap();
        assert_eq!(back, sample());
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn corba_p1_18_1_streams_of_the_reference_zlib_decompress() {
        // Written by zlib 1.3.1, the C library (Python 3.13
        // `zlib.compress(d, 9)` and `gzip.compress(d, 9, mtime=0)` on the
        // Linux test host), for d = b"CORBA 3.3 Part 1 18 Compression " * 8.
        let zlib = unhex(
            "78da73f60f72725430d6335608482c2a51305430b45070cecf2d284a2d2ececccf53701ee6f200a88b4bf1",
        );
        let gzip = unhex(
            "1f8b08000000000002ff73f60f72725430d6335608482c2a51305430b45070cecf2d284a2d2ececccf\
             53701ee6f200c36ec99300010000",
        );
        let expected = b"CORBA 3.3 Part 1 18 Compression ".repeat(8);
        let m = CompressionManager::with_default_factories();
        for (id, stream) in [(COMPRESSORID_ZLIB, zlib), (COMPRESSORID_GZIP, gzip)] {
            let mut back = Vec::new();
            m.get_compressor(id, 9)
                .unwrap()
                .decompress(&stream, &mut back)
                .unwrap();
            assert_eq!(back, expected);
        }
    }

    #[test]
    fn corba_p1_18_2_every_level_of_the_built_in_compressors_round_trips() {
        for factory in [
            FlateCompressorFactory::zlib(),
            FlateCompressorFactory::gzip(),
        ] {
            for level in 0..=MAX_COMPRESSION_LEVEL {
                let c = factory.get_compressor(level).unwrap();
                assert_eq!(c.compression_level(), level);
                let mut packed = Vec::new();
                c.compress(&sample(), &mut packed).unwrap();
                if level == 0 {
                    // 0 = no compression: stored blocks, larger than the input.
                    assert!(packed.len() > sample().len());
                } else {
                    assert!(packed.len() < sample().len() / 4);
                }
                let mut back = Vec::new();
                c.decompress(&packed, &mut back).unwrap();
                assert_eq!(back, sample());
            }
        }
        // gzip: RFC 1952 magic.
        let c = FlateCompressorFactory::gzip().get_compressor(9).unwrap();
        let mut packed = Vec::new();
        c.compress(b"x", &mut packed).unwrap();
        assert_eq!(&packed[..2], &[0x1f, 0x8b]);
    }

    #[test]
    fn corba_p1_18_2_6_the_counters_and_the_ratio_follow_compress() {
        let c = FlateCompressorFactory::zlib().get_compressor(9).unwrap();
        assert_eq!(c.compression_ratio(), 0.0);
        let mut first = Vec::new();
        c.compress(&sample(), &mut first).unwrap();
        let mut second = Vec::new();
        c.compress(b"short", &mut second).unwrap();
        // decompress does not count.
        let mut back = Vec::new();
        c.decompress(&first, &mut back).unwrap();
        let compressed = (first.len() + second.len()) as u64;
        let uncompressed = (sample().len() + 5) as u64;
        assert_eq!(c.compressed_bytes(), compressed);
        assert_eq!(c.uncompressed_bytes(), uncompressed);
        let expected = compressed as f32 / uncompressed as f32;
        assert!((c.compression_ratio() - expected).abs() < 1e-6);
    }

    #[test]
    fn corba_p1_18_3_2_the_same_level_gives_the_same_compressor_and_its_factory() {
        let factory = FlateCompressorFactory::zlib();
        let a = factory.get_compressor(5).unwrap();
        let b = factory.get_compressor(5).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(!Arc::ptr_eq(&a, &factory.get_compressor(6).unwrap()));
        let back = a.compressor_factory().unwrap();
        assert_eq!(back.compressor_id(), COMPRESSORID_ZLIB);
        let m = CompressionManager::with_default_factories();
        let x = m.get_compressor(COMPRESSORID_GZIP, 3).unwrap();
        let y = m.get_compressor(COMPRESSORID_GZIP, 3).unwrap();
        assert!(Arc::ptr_eq(&x, &y));
    }

    #[test]
    fn corba_p1_18_3_2_a_level_above_9_is_bad_param_44() {
        let e = FlateCompressorFactory::zlib()
            .get_compressor(10)
            .map(|_| ())
            .unwrap_err();
        assert_eq!(
            (e.kind, e.minor, e.completed),
            (
                SystemExceptionKind::BadParam,
                0x4f4d_0000 | 44,
                CompletionStatus::No
            )
        );
        let m = CompressionManager::with_default_factories();
        match m.get_compressor(COMPRESSORID_ZLIB, 10) {
            Err(GetCompressorError::System(e)) => assert_eq!(e, bad_compression_level()),
            other => panic!("expected BAD_PARAM 44, got {:?}", other.map(|_| ())),
        }
    }

    #[test]
    fn corba_p1_18_2_3_corrupt_input_is_a_compression_exception() {
        let c = FlateCompressorFactory::zlib().get_compressor(6).unwrap();
        let mut back = Vec::new();
        let e = c
            .decompress(&[0x78, 0x9c, 0xff, 0xff, 0x00], &mut back)
            .unwrap_err();
        assert_eq!(e.reason, Z_DATA_ERROR);
        assert!(!e.description.is_empty());
    }

    #[test]
    fn corba_p1_18_2_3_decompression_stops_at_the_limit() {
        let factory = FlateCompressorFactory::new(FlateFormat::Zlib, 64 * 1024);
        let c = factory.get_compressor(9).unwrap();
        let mut packed = Vec::new();
        c.compress(&vec![0_u8; 64 * 1024 + 1], &mut packed).unwrap();
        assert!(packed.len() < 1024);
        let mut back = Vec::new();
        let e = c.decompress(&packed, &mut back).unwrap_err();
        assert_eq!(e.reason, Z_BUF_ERROR);
        // Exactly at the limit is accepted.
        c.compress(&vec![0_u8; 64 * 1024], &mut packed).unwrap();
        c.decompress(&packed, &mut back).unwrap();
        assert_eq!(back.len(), 64 * 1024);
    }

    /// A custom compressor (§18.1: "it must be possible to implement a
    /// new custom compressor"): run-length pairs (count, octet).
    struct RleFactory {
        me: Weak<RleFactory>,
    }

    struct Rle {
        factory: Weak<RleFactory>,
        compressed: AtomicU64,
        uncompressed: AtomicU64,
    }

    impl Compressor for Rle {
        fn compress(&self, source: &[u8], target: &mut Buffer) -> Result<(), CompressionException> {
            let mut out = Vec::new();
            for chunk in source.chunk_by(|a, b| a == b) {
                for part in chunk.chunks(255) {
                    out.push(u8::try_from(part.len()).unwrap());
                    out.push(part[0]);
                }
            }
            self.uncompressed
                .fetch_add(source.len() as u64, Ordering::Relaxed);
            self.compressed
                .fetch_add(out.len() as u64, Ordering::Relaxed);
            *target = out;
            Ok(())
        }
        fn decompress(
            &self,
            source: &[u8],
            target: &mut Buffer,
        ) -> Result<(), CompressionException> {
            if !source.len().is_multiple_of(2) {
                return Err(CompressionException {
                    reason: 1,
                    description: "odd length".into(),
                });
            }
            *target = source
                .chunks(2)
                .flat_map(|p| core::iter::repeat_n(p[1], usize::from(p[0])))
                .collect();
            Ok(())
        }
        fn compressor_factory(&self) -> Option<Arc<dyn CompressorFactory>> {
            self.factory
                .upgrade()
                .map(|f| f as Arc<dyn CompressorFactory>)
        }
        fn compression_level(&self) -> CompressionLevel {
            1
        }
        fn compressed_bytes(&self) -> u64 {
            self.compressed.load(Ordering::Relaxed)
        }
        fn uncompressed_bytes(&self) -> u64 {
            self.uncompressed.load(Ordering::Relaxed)
        }
    }

    impl CompressorFactory for RleFactory {
        fn compressor_id(&self) -> CompressorId {
            0x8001
        }
        fn get_compressor(
            &self,
            level: CompressionLevel,
        ) -> Result<Arc<dyn Compressor>, SystemException> {
            check_level(level)?;
            Ok(Arc::new(Rle {
                factory: self.me.clone(),
                compressed: AtomicU64::new(0),
                uncompressed: AtomicU64::new(0),
            }))
        }
    }

    #[test]
    fn corba_p1_18_4_register_get_and_unregister_a_custom_factory() {
        let m = CompressionManager::with_default_factories();
        assert_eq!(
            m.get_factory(0x8001).map(|_| ()).unwrap_err(),
            UnknownCompressorId
        );
        let rle: Arc<dyn CompressorFactory> = Arc::new_cyclic(|me| RleFactory { me: me.clone() });
        m.register_factory(Arc::clone(&rle)).unwrap();
        assert_eq!(
            m.register_factory(Arc::clone(&rle)),
            Err(FactoryAlreadyRegistered)
        );
        // A second factory for an id that is taken.
        assert_eq!(
            m.register_factory(FlateCompressorFactory::zlib()),
            Err(FactoryAlreadyRegistered)
        );
        assert_eq!(m.get_factories().len(), 3);
        assert!(Arc::ptr_eq(&m.get_factory(0x8001).unwrap(), &rle));
        let c = m.get_compressor(0x8001, 1).unwrap();
        let mut packed = Vec::new();
        c.compress(b"aaaaabbbc", &mut packed).unwrap();
        assert_eq!(packed, [5, b'a', 3, b'b', 1, b'c']);
        let mut back = Vec::new();
        c.decompress(&packed, &mut back).unwrap();
        assert_eq!(back, b"aaaaabbbc");
        assert_eq!(c.compressor_factory().unwrap().compressor_id(), 0x8001);
        m.unregister_factory(0x8001).unwrap();
        assert_eq!(m.unregister_factory(0x8001), Err(UnknownCompressorId));
        assert!(matches!(
            m.get_compressor(0x8001, 1).map(|_| ()),
            Err(GetCompressorError::UnknownCompressorId)
        ));
        // Ids without a factory: the reserved ones of §18.5 too.
        for id in [COMPRESSORID_NONE, COMPRESSORID_BZIP2, COMPRESSORID_LZMA] {
            assert!(matches!(
                m.get_compressor(id, 1).map(|_| ()),
                Err(GetCompressorError::UnknownCompressorId)
            ));
        }
    }

    #[test]
    fn corba_p2_12_1_the_compression_manager_is_the_ziop_codec() {
        use zerodds_corba_giop::ziop::{
            DEFAULT_COMPRESSOR, ZiopCodec, ZiopSend, compress_message, decompress_message,
        };
        use zerodds_corba_giop::{
            Message, Reply, ReplyStatusType, ServiceContextList, Version, encode_message,
        };
        let m = CompressionManager::with_default_factories();
        assert!(m.supports(COMPRESSORID_ZLIB));
        assert!(m.supports(COMPRESSORID_GZIP));
        assert!(!m.supports(COMPRESSORID_BZIP2));
        let frame = encode_message(
            Version::V1_2,
            zerodds_wire::Endianness::Little,
            false,
            &Message::Reply(Reply {
                request_id: 3,
                reply_status: ReplyStatusType::NoException,
                service_context: ServiceContextList::default(),
                body: b"ZIOP compresses GIOP bodies. ".repeat(100),
            }),
        )
        .unwrap();
        let send = ZiopSend {
            compressor: DEFAULT_COMPRESSOR,
            low_value: 0,
            min_ratio: Some(0.5),
        };
        let z = compress_message(&frame, &send, &m).unwrap().unwrap();
        assert_eq!(&z[..4], b"ZIOP");
        // The data is a zlib stream (RFC 1950: CMF 0x78).
        assert_eq!(z[24], 0x78);
        assert!(z.len() * 4 < frame.len());
        assert_eq!(decompress_message(&z, &m, 1 << 20).unwrap(), frame);
        // A compressor the manager does not have.
        let bzip2 = ZiopSend {
            compressor: CompressorIdLevel {
                compressor_id: COMPRESSORID_BZIP2,
                compression_level: 9,
            },
            ..send
        };
        assert!(compress_message(&frame, &bzip2, &m).is_err());
        let mut unknown = z.clone();
        unknown[12] = 3;
        assert!(decompress_message(&unknown, &m, 1 << 20).is_err());
    }
}
