# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed
- The status methods of the writers and readers
  (`publication_matched_status`, `subscription_matched_status`,
  `liveliness_lost_status`, `offered_deadline_missed_status`,
  `sample_lost_status`, `requested_deadline_missed_status`) return the
  record DDS 1.4 §2.2.4.1 defines: a cumulative `total_count`, the
  changes since the last read, the handle of the last endpoint that
  joined or left. They returned the current match count twice and 0 for
  every change and handle. A read resets the changes; on a disabled
  entity the methods raise.

### Removed
- `zerodds.idl` (the `@idl_struct` decorator and its own `_IdlKind`
  hierarchy) and `zerodds.cdr` (`CdrWriter`/`CdrReader`) — the second CDR
  codec beside `runtime/python` and `zerodds-idlc generate --lang
  python`'s output (C2(7): exactly one CDR codec per language). Every
  caller now goes through a generated `TopicDescriptor`
  (`zerodds_wire.topic_descriptor`), the same TypeSupport bridge C#'s
  `<Type>.TypeSupport.cs` and Java's `ZdTypeSupports.Support_<tag>`
  provide. No deprecated alias is kept for the removed names.
- `zerodds.topic.IdlTopic`/`IdlWriter`/`IdlReader` no longer accept a
  legacy `@idl_struct`-decorated `@dataclass`; `IdlTopic.__init__` now
  requires a `zerodds_wire.topic_descriptor.TopicDescriptor` and raises
  `TypeError` otherwise. `IdlWriter`/`IdlReader` are the former
  `DescriptorWriter`/`DescriptorReader` (unified, one path).
- `examples/03_idl_struct_cdr.py` → `examples/03_idl_topic.py`, generated
  from the committed `examples/03_idl_topic.idl` via `zerodds-idlc
  generate --lang python`, instead of the decorator.

### Fixed
- `zerodds_py.DescriptorWriter`/`DescriptorReader` (the native classes
  behind `create_descriptor_writer`/`create_descriptor_reader`, i.e. the
  ones `zerodds.topic.IdlWriter`/`IdlReader` now always wrap) were missing
  `publication_matched_status`/`liveliness_lost_status`/
  `offered_deadline_missed_status` (writer) and
  `subscription_matched_status`/`sample_lost_status`/
  `requested_deadline_missed_status` (reader) — present on
  `BytesWriter`/`BytesReader` and on the legacy `IdlWriter`/`IdlReader`'s
  own `BytesWriter`/`BytesReader`-backed path this change removes. Making
  every caller go through the descriptor path surfaced the gap
  immediately (`test_idl_writer_passthrough_status`); added, mirroring
  the `BytesWriter`/`BytesReader` implementations exactly
  (`crates/py/src/ffi.rs`).
- `python/tests/_generated/*.py` (12 fixtures) were produced by the
  removed `idl-python` tool; their source `.idl` was never committed.
  Reconstructed one `.idl` per fixture from the generated module's
  types/annotations/extensibility (committed next to the tests) and
  regenerated all 12 with `zerodds-idlc --lang python`. The regenerated
  modules expose `ZD_TOPIC_DESCRIPTORS["Type::Name"]`
  (`zerodds_wire.topic_descriptor.TopicDescriptor`, `runtime/python`),
  not a class with its own `.encode`/`.decode` — that shape belonged to
  the tool this fixture predates. `test_idl.py`'s 12 generated-module
  tests are rewritten against the regenerated modules accordingly (map
  fields are `list[tuple[K, V]]`, not `dict`; fixed arrays are
  constructed as `tuple` but come back out of `decode` as `list`; a
  union is a dataclass with a `_d` discriminator field and one Optional
  field per case, not the removed `idl_union` helper's `.make()`).
  `xcdr1.idl` (the one fixture exercising both XCDR1 and XCDR2) is
  regenerated twice — `xcdr1_xcdr1_gen.py` / `xcdr1_xcdr2_gen.py` — since
  a `TopicDescriptor` is locked to the one representation it was
  generated for, unlike the removed tool's classes, which took a
  `representation=` argument on `encode`/`decode`.
- `combo.idl`'s enum and union are named `ModeKind`/`ReadingUnion`, not
  `Mode`/`Reading`: the fixture's struct member names are literally
  `mode`/`reading`, and today's idlc refuses a type name and a member
  name that differ only in case in one scope (IDL0008, OMG IDL 4.2
  §7.2.3.1) — something the removed tool evidently allowed. The member
  names are unchanged.
- `keyed.idl`'s `id` field is `@key`: read off the fixture's embedded
  TYPE_OBJECT member-flags word (0x0021 against 0x0001 for the other
  members — the IS_KEY bit), since the `zerodds.idl` decorator this
  fixture predates has no `@key` concept of its own to have preserved
  one explicitly.
- `test_idl.py::_load_generated` did not register the module it builds
  in `sys.modules` before executing it. Every regenerated fixture uses
  `from __future__ import annotations`; `dataclasses.dataclass` on
  Python 3.10+ resolves a deferred string annotation via
  `sys.modules[cls.__module__]`, so every `@dataclass` in an unregistered
  module failed at class-body-execution time. The 12 pre-existing
  fixtures never used deferred annotations and never exercised this.
- The regenerated Python encoder did not itself enforce a declared bound
  (`string<N>`, `sequence<T,N>`, `map<K,V,N>`, `wstring<N>`): the emitted
  `zd_enc_*_bN` functions wrote whatever length the caller's value had.
  This was not particular to Python — the shared codec walk
  (`zerodds_lang_rules::emit::encode`) built every non-Rust column's
  bounded `string`/`wstring`/`sequence`/`map` from the same unbounded
  primitive, so the same gap existed in all nineteen other columns; the
  Rust codegen and `crates/wire-codec`'s reflective codec already
  checked the bound. Fixed at the walk: a bounded write now calls
  `Writer.write_string_bounded`/`write_wstring_bounded` (which measure
  the value and raise `OutOfRange` over the bound before writing
  anything, replacing two columns' — C# and Java — dead attempts that
  truncated at a character boundary instead) or, for a `sequence`/`map`,
  `Writer.check_sequence_bound`/`check_map_bound` ahead of the DHEADER.
  `bounds_gen.py` is regenerated; `test_generated_bounds_roundtrip_and_enforced`
  now asserts the encode-side `OutOfRange` directly, in addition to the
  decode-side `@try_construct` DISCARD it already measured (XTypes 1.3
  §7.2.2.4.4.4.9) against a wire value assembled by hand — the one case
  the encoder's own refusal cannot reach, a foreign peer's sample.

### Notes (found, not fixed here — outside a CDR-codec-consolidation pass)
- A fixed-count array (`T name[N]`) has no bound check on either side of
  the generated codec: too few elements makes `zd_enc_arr_*_N` index past
  the list end and raise `IndexError`; too many elements are silently
  dropped on encode with no error, no warning, and no decode-side check
  either (a fixed array has no count field on the wire to compare against
  a bound the way a bounded sequence/string/map does). The removed
  `zerodds.idl._IdlArray._write` raised a descriptive `ValueError` for
  both directions.
- A union with no `default:` case and an unmatched discriminator does not
  raise on encode: the shared codegen writes the discriminator and
  nothing after it (`crates/lang-rules/src/emit/encode.rs`: mirrors the
  suppressed-label overlay every other binding's codec uses). The removed
  `zerodds.idl._IdlUnion._write_body` raised `ValueError` for exactly this
  case. `test_union_without_default_writes_only_the_discriminator` pins
  the measured behaviour; this is by design, not a gap this pass leaves
  open.
- `map<K,V>` is `list[tuple[K,V]]` in generated code, not a `dict`: the
  generated encoder writes entries in the caller's given order and does
  not sort by key. The removed `zerodds.idl._IdlMap._write` explicitly
  sorted by key before writing every time, which no caller of the
  generated codec can now rely on implicitly — a caller that needs a
  deterministic wire order (e.g. cross-vendor byte comparison) has to
  sort the list itself before encoding.
- The fixed-array overcount above is open: the bound checks that now
  refuse an over-bound `string<N>`/`wstring<N>`/`sequence<T,N>`/
  `map<K,V,N>` in every generated encoder do not cover a fixed array's
  element count. `test_fixed_array_count_is_refused_on_encode` states the
  required behaviour and is marked `xfail(strict=True)` until it holds.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the **`zerodds-py`** crate as a layer-6 PSM/binding.

### Spec references
- OMG DDS 1.4 §2.2.2 + DDS-PSM-Cxx 1.0 §7.5: PSM API surface
- ZeroDDS vendor spec `zerodds-c-api-1.0` (C-FFI foundation)

### Public API
See `README.md` + `src/lib.rs` doc comments + the associated coverage doc.

### Implementation
PyO3 bindings for the ZeroDDS DCPS API

### Architecture
- Layer: 6 (PSMs / bindings)
- Dependencies (in): `zerodds-c-api` (foundation) + language-specific helper crates
- Dependents (out): user code

### Stability
All `pub` items are RC1-stable; breaking changes require a major bump.

