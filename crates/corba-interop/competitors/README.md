# CORBA Test & Interop Suite

Complete suite for the entire CORBA crate family.

## Running

The cross-ORB cells are tests of `zerodds-corba-interop`, `#[ignore]`d in
the crate tier and run where the foreign ORBs are:

```sh
# omniORB 4.3 and JacORB 3.9 (CI: integration-bridges-vendor-corba)
bash crates/corba-interop/competitors/build_peers.sh omniorb /tmp/corba-peers
bash crates/corba-interop/competitors/build_peers.sh jacorb /tmp/corba-peers
ZERODDS_CORBA_PEERS=/tmp/corba-peers cargo nextest run -p zerodds-corba-interop \
    --test omniorb_cross_orb --test jacorb_cross_orb --run-ignored ignored-only

# TAO of the OpenDDS build (CI: vendor-live-corba-tao)
TAO_PREFIX=/opt/opendds bash crates/corba-interop/competitors/build_peers.sh tao /tmp/corba-peers
ZERODDS_CORBA_PEERS=/tmp/corba-peers cargo test -p zerodds-corba-interop \
    --test tao_cross_orb -- --ignored
```

`build_peers.sh` builds each ORB's programs from the IDL ZeroDDS generates
its side from (`interop.idl`, `cosnaming.idl`, `features.idl`) with that
ORB's own IDL compiler; a test whose program is missing fails.

## What is covered

**1. Unit / integration tests** — the crate tier of each CORBA crate.

**2. Cross-ORB** (`tests/omniorb_cross_orb.rs`, `tests/tao_cross_orb.rs`,
`tests/jacorb_cross_orb.rs`) — ZeroDDS ↔ **omniORB 4.3.3 / TAO / JacORB
3.9**, both directions: the Echo and Bench matrix (long/double/long
long/char/string/**wstring**/any/sequence/out/inout/object-ref/typed
exception), CosNaming (a foreign client drives the ZeroDDS
`NamingContext`; ZeroDDS binds and resolves in omniNames and in JacORB's
NameServer), SSLIOP with omniORB in both directions, the AMI client
against omniORB's Bench, JacORB's transactional client propagating its OTS
context, and every operation of `Features` and `Shapes` (boolean, octet,
short, unsigned long, sequence<string>, inout pairs, oneway, attributes,
struct, enum, union with a `default:` member) between the generated stubs
and skeletons and omniORB's. The TAO of the OpenDDS build has no naming
daemon, so the ZeroDDS naming client meets omniNames and JacORB's.

## Cross-ORB coverage per feature

| Feature | omniORB | TAO | JacORB |
|---|---|---|---|
| Echo (string, 4 kB) | both directions | both directions | both directions |
| Bench: long/double/long long/char/string/wstring/any/sequence<long>/out/inout/object reference/typed exception | both directions | both directions | both directions |
| CosNaming: foreign client → ZeroDDS `NamingContext` | yes | yes | yes |
| CosNaming: ZeroDDS client → foreign daemon | omniNames | – (no daemon in the OpenDDS build) | NameServer |
| SSLIOP | both directions | – | – |
| AMI client (callback, polling) | yes | – | – |
| OTS context from a transactional client | – | – | yes |
| Features and Shapes (generated stubs and skeletons) | both directions | – | – |

CosNaming uses the standardized `CosNaming::NamingContext` wire
(`Name = sequence<NameComponent{id,kind}>`, RepositoryId
`IDL:omg.org/CosNaming/NamingContext:1.0`); foreign clients generate their stubs
from `cosnaming.idl` (`#pragma prefix "omg.org"`) via omniidl/tao_idl or use
`org.omg.CosNaming` (JacORB).

wstring crosses with the UTF-16 BOM (§15.3.1.6) on the wire and the
TAG_CODE_SETS component in the IOR (§13.10.2.4); without the component
omniORB throws INV_OBJREF when sending a wstring, TAO and JacORB do not.
