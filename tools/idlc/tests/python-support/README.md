# Fresh Python topic descriptors

This fixture consumes current shared-CLI output, not the older decorated
Python classes. `support.idl` covers short and long keys, two keys with the
same eight-byte prefix, an empty aggregate, and an appendable optional value.

From the ZeroDDS root, using the already built matching CLI:

```sh
generated_support="$(mktemp -d)"
target/debug/zerodds-idlc generate --lang python --encoding xcdr2 \
  --out "$generated_support" tools/idlc/tests/python-support/support.idl
PYTHONPATH="$PWD/runtime/python" python3 \
  tools/idlc/tests/python-support/consumer.py "$generated_support/support.py"
PYTHONPATH="$PWD/runtime/python" python3 \
  tools/idlc/tests/python-support/test_descriptor.py
```

Repeat generation into another fresh directory with `--encoding xcdr1` for
that representation. The descriptor table stays in the generated module, so
the input filename need not be a valid Python import identifier. The consumer
loads that exact file and uses its actual generated classes.

The consumer checks independent LE/BE octets and key literals, maximum-size
MD5 selection, optional presence/absence, empty values and strict rejection.
The six boundary tests separately exercise the descriptor's argument,
metadata, callback, holder and decode-consumption contracts.

These are codec/descriptor checks. No native DDS endpoint or remote peer is
created, and no cross-vendor result is claimed. The native per-topic support
connection remains separate integration work. See the [CLI contract](../../README.md).

`ZD_TOPIC_DESCRIPTORS["scope::Type"]` exposes the generated `cls`, exact
`type_name`, `keyed`, `extensibility`, `representation`, `type_object`, and
`encode`/`decode`/`key_hash`/`key_holder`/`decode_key_holder` methods. Encode/decode operate on a body without
encapsulation. One generated descriptor supports its declared representation,
not both XCDR versions by changing a runtime argument. Requesting unavailable
TypeObject metadata raises an explicit error.

`key_holder(sample)` returns the actual PLAIN_CDR2 big-endian, MAXALIGN=4
key-only bytes. The hash and holder share the same generated writer result,
with hash finalization selected by the existing maximum-size planner.
The 16-byte hash's zero-padding or MD5 digest is not substituted for the holder;
the holder retains its own CDR alignment padding. Keyless types return
`None`, distinct from an actual empty holder or a legitimate zero key.
Older keyed descriptors lacking the holder callback raise `NotImplementedError`.

`decode_key_holder` uses the new shared, bounded inverse entry for struct roots
with direct scalar, string or enum keys, under the default ForcedFinal
PLAIN_CDR2-BE key dialect. Member order comes from the key plan; non-key values
come from existing generated default materializers, including explicit defaults
and absent optionals. Even appendable/mutable roots have no data DHEADER in
their holders. Actual CDR alignment remains authoritative.

Nested aggregates, aliases, collections, indirect/optional key members, root
unions and other holder dialects currently refuse inverse generation explicitly.
Their descriptor keeps its working writer/hash operations, while
`decode_key_holder` raises `NotImplementedError` with the generation reason.
Keyless descriptors also refuse reconstruction. Truncation, decoder discard,
trailing hash padding and wrong result classes never become successful reads.
The shared entry requires an explicit target initialization contract; targets
that have not implemented it refuse this new entry, with existing data codecs
and their names unchanged. Full native keyed-lifecycle integration remains open.

Descriptor representations use DDS IDs `0` (XCDR1) and `2` (XCDR2).
The native reader's internal encapsulation discriminator instead uses `0`
and `1`. A future binding must map internal `1` to descriptor `2` explicitly,
not pass the internal number directly to these methods.

Pass the descriptor, not its class, to `IdlTopic`: the descriptor creates a
typed native topic, a decorated class the untyped raw-bytes topic. The
[Python binding scope](../../../../crates/py/README.md#generated-python-and-native-topics)
shows the typed form; `crates/py/tests/generated_descriptor_live.py` runs it
across two processes.
